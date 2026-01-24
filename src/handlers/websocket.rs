use serde::{Deserialize, Serialize};
use worker::*;

use crate::auth::AuthMiddleware;
use crate::models::{parse_token, verify_token, PendingClientInfo};
use crate::templates;

/// Row for token validation query
#[derive(Debug, Deserialize)]
struct TokenRow {
    user_id: String,
    token_hash: String,
}

/// Extract geo/IP info from Cloudflare headers
fn extract_geo_info(req: &Request) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let headers = req.headers();
    let ip_address = headers.get("CF-Connecting-IP").ok().flatten();
    let country = headers.get("CF-IPCountry").ok().flatten();
    let city = headers.get("CF-IPCity").ok().flatten();
    let region = headers.get("CF-Region").ok().flatten();
    (ip_address, country, city, region)
}

/// WebSocket upgrade handler - routes to appropriate Durable Object
pub async fn websocket_upgrade(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Note: Cloudflare handles WebSocket upgrade at edge - we just need to forward to DO
    // The DO will handle the actual WebSocket connection via WebSocketPair

    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Extract geo info from CF headers
    let (ip_address, country, city, region) = extract_geo_info(&req);

    // Determine connection type
    let is_browser = params.get("type").map(|t| t == "browser").unwrap_or(false);

    // For browser connections, require authentication via session cookie
    if is_browser {
        let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
            Ok(user) => user,
            Err(redirect) => return Ok(redirect),
        };

        // Forward to user's Durable Object
        let namespace = ctx.env.durable_object("USER_HUB")?;
        let id = namespace.id_from_name(&user.id)?;
        let stub = id.get_stub()?;

        // Forward with WebSocket upgrade headers for DO
        let headers = Headers::new();
        headers.set("Upgrade", "websocket")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Get);
        init.with_headers(headers);

        let do_req = Request::new_with_init("https://do/ws?type=browser", &init)?;
        stub.fetch_with_request(do_req).await
    } else {
        // claudecodeui connection - authenticate via token
        let full_token = params.get("token").ok_or("Missing token parameter")?;

        // Parse and validate token
        let (token_id, raw_token) = match parse_token(full_token) {
            Some(parts) => parts,
            None => return Response::error("Invalid token format", 401),
        };

        // Look up token in D1
        let db = ctx.env.d1("DB")?;
        let token_result = db
            .prepare(
                "SELECT user_id, token_hash FROM client_tokens WHERE id = ?1 AND revoked_at IS NULL",
            )
            .bind(&[token_id.clone().into()])?
            .first::<TokenRow>(None)
            .await?;

        let token_row = match token_result {
            Some(row) => row,
            None => return Response::error("Token not found or revoked", 401),
        };

        // Verify token hash
        if !verify_token(&raw_token, &token_row.token_hash) {
            return Response::error("Invalid token", 401);
        }

        // Update last_used timestamp (fire and forget)
        let _ = db
            .prepare("UPDATE client_tokens SET last_used = CURRENT_TIMESTAMP WHERE id = ?1")
            .bind(&[token_id.into()])?
            .run()
            .await;

        // Forward to user's Durable Object
        let namespace = ctx.env.durable_object("USER_HUB")?;
        let id = namespace.id_from_name(&token_row.user_id)?;
        let stub = id.get_stub()?;

        // Forward with WebSocket upgrade headers for DO
        let headers = Headers::new();
        headers.set("Upgrade", "websocket")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Get);
        init.with_headers(headers);

        // Include client_id and geo info in the DO request URL for hibernation-aware tagging
        let client_id = params.get("client_id").cloned().unwrap_or_default();
        let mut do_url = format!("https://do/ws?client_id={}", client_id);
        if let Some(ip) = &ip_address {
            do_url.push_str(&format!("&ip={}", urlencoding::encode(ip)));
        }
        if let Some(c) = &country {
            do_url.push_str(&format!("&country={}", urlencoding::encode(c)));
        }
        if let Some(ct) = &city {
            do_url.push_str(&format!("&city={}", urlencoding::encode(ct)));
        }
        if let Some(r) = &region {
            do_url.push_str(&format!("&region={}", urlencoding::encode(r)));
        }
        let do_req = Request::new_with_init(&do_url, &init)?;
        stub.fetch_with_request(do_req).await
    }
}

/// WebSocket handler for pending (unauthenticated) clients
pub async fn websocket_pending(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Extract claim patterns from query params
    let user = params.get("user").cloned().unwrap_or_default();
    let org = params.get("org").cloned().unwrap_or_default();
    let team = params.get("team").cloned().unwrap_or_default();

    // Require at least one claim pattern
    if user.is_empty() && org.is_empty() && team.is_empty() {
        return Response::error("At least one claim pattern required (user, org, or team)", 400);
    }

    // Extract geo info from CF headers
    let (ip_address, country, city, region) = extract_geo_info(&req);

    // Forward to global PendingHub Durable Object
    let namespace = ctx.env.durable_object("PENDING_HUB")?;
    // Use a fixed name for the global pending hub
    let id = namespace.id_from_name("global")?;
    let stub = id.get_stub()?;

    // Build DO URL with all params
    let headers = Headers::new();
    headers.set("Upgrade", "websocket")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    init.with_headers(headers);

    let mut do_url = format!("https://do/ws?user={}&org={}&team={}",
        urlencoding::encode(&user),
        urlencoding::encode(&org),
        urlencoding::encode(&team)
    );
    if let Some(ip) = &ip_address {
        do_url.push_str(&format!("&ip={}", urlencoding::encode(ip)));
    }
    if let Some(c) = &country {
        do_url.push_str(&format!("&country={}", urlencoding::encode(c)));
    }
    if let Some(ct) = &city {
        do_url.push_str(&format!("&city={}", urlencoding::encode(ct)));
    }
    if let Some(r) = &region {
        do_url.push_str(&format!("&region={}", urlencoding::encode(r)));
    }

    let do_req = Request::new_with_init(&do_url, &init)?;
    stub.fetch_with_request(do_req).await
}

/// Get pending clients that the current user can claim
pub async fn get_pending_clients(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Require authentication
    let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
        Ok(user) => user,
        Err(redirect) => return Ok(redirect),
    };

    // TODO: Get user's orgs and teams from GitHub API or cache
    // For now, just use the user's login
    let user_orgs: Vec<String> = Vec::new(); // Would be fetched from GitHub
    let user_teams: Vec<String> = Vec::new(); // Would be fetched from GitHub

    // Query the PendingHub for clients this user can claim
    let namespace = ctx.env.durable_object("PENDING_HUB")?;
    let id = namespace.id_from_name("global")?;
    let stub = id.get_stub()?;

    let do_url = format!(
        "https://do/pending?github_login={}&orgs={}&teams={}",
        urlencoding::encode(&user.github_login),
        urlencoding::encode(&user_orgs.join(",")),
        urlencoding::encode(&user_teams.join(","))
    );

    let do_req = Request::new_with_init(&do_url, &RequestInit::new())?;
    let mut do_response = stub.fetch_with_request(do_req).await?;

    // Parse the JSON response from DO
    let pending_clients: Vec<PendingClientInfo> = do_response.json().await?;

    // Render as HTML for HTMX
    let html = templates::render_pending_list(&pending_clients);
    Response::from_html(html)
}

/// Claim a pending client (authorize it and issue a token)
pub async fn claim_pending_client(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Require authentication
    let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
        Ok(user) => user,
        Err(redirect) => return Ok(redirect),
    };

    let pending_id = ctx.param("id").ok_or("Missing pending_id")?;

    // Parse the claim request body
    #[derive(Deserialize)]
    struct ClaimRequest {
        name: String,
    }

    let body: ClaimRequest = req.json().await?;

    // Forward to PendingHub to claim the client
    let namespace = ctx.env.durable_object("PENDING_HUB")?;
    let id = namespace.id_from_name("global")?;
    let stub = id.get_stub()?;

    // Build claim request for DO
    #[derive(Serialize)]
    struct DoClaimRequest {
        user_id: String,
        name: String,
    }

    let do_body = serde_json::to_string(&DoClaimRequest {
        user_id: user.id,
        name: body.name,
    })?;

    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(do_body.into()));

    let do_url = format!("https://do/claim/{}", pending_id);
    let do_req = Request::new_with_init(&do_url, &init)?;
    stub.fetch_with_request(do_req).await
}
