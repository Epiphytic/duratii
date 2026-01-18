use worker::*;

use crate::auth::AuthMiddleware;

/// WebSocket upgrade handler - routes to appropriate Durable Object
pub async fn websocket_upgrade(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Determine connection type
    let is_browser = params.get("type").map(|t| t == "browser").unwrap_or(false);

    // For browser connections, require authentication
    if is_browser {
        let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
            Ok(user) => user,
            Err(redirect) => return Ok(redirect),
        };

        // Forward to user's Durable Object
        let namespace = ctx.env.durable_object("USER_HUB")?;
        let id = namespace.id_from_name(&user.id)?;
        let stub = id.get_stub()?;

        let do_req = Request::new("https://do/ws?type=browser", Method::Get)?;
        stub.fetch_with_request(do_req).await
    } else {
        // claudecodeui connection - authenticate via token in first message
        let user_token = params.get("token").ok_or("Missing token parameter")?;

        // Validate token and get user ID
        let user_id = validate_client_token(&ctx.env, user_token).await?;

        // Forward to user's Durable Object
        let namespace = ctx.env.durable_object("USER_HUB")?;
        let id = namespace.id_from_name(&user_id)?;
        let stub = id.get_stub()?;

        let do_req = Request::new("https://do/ws", Method::Get)?;
        stub.fetch_with_request(do_req).await
    }
}

/// Validate a client connection token
/// In production, this would verify against D1 or a signed JWT
async fn validate_client_token(env: &Env, token: &str) -> Result<String> {
    let db = env.d1("DB")?;

    // Check if this is a valid session token
    let result = db
        .prepare(
            "SELECT user_id FROM sessions
             WHERE id = ?1 AND expires_at > datetime('now')",
        )
        .bind(&[token.into()])?
        .first::<UserIdRow>(None)
        .await?;

    match result {
        Some(row) => Ok(row.user_id),
        None => Err("Invalid or expired token".into()),
    }
}

#[derive(serde::Deserialize)]
struct UserIdRow {
    user_id: String,
}
