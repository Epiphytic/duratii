mod middleware;

pub use middleware::AuthMiddleware;

use serde::{Deserialize, Serialize};
use worker::*;

const GITHUB_AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_URL: &str = "https://api.github.com/user";
const GITHUB_ORGS_URL: &str = "https://api.github.com/user/orgs";

#[derive(Debug, Serialize, Deserialize)]
struct GitHubUser {
    id: i64,
    login: String,
    email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GitHubOrg {
    id: i64,
    login: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    scope: String,
}

/// Start GitHub OAuth flow
pub async fn start_oauth(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let client_id = ctx.env.var("GITHUB_CLIENT_ID")?.to_string();
    let redirect_uri = get_redirect_uri(&req)?;

    // Generate state for CSRF protection
    let state = generate_state();

    // Store state in cookie for validation
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&scope=read:org%20read:user%20user:email&state={}",
        GITHUB_AUTHORIZE_URL,
        client_id,
        url_encode(&redirect_uri),
        state
    );

    let mut headers = Headers::new();
    headers.set("Location", &auth_url)?;
    headers.set(
        "Set-Cookie",
        &format!(
            "oauth_state={}; HttpOnly; Secure; SameSite=Lax; Max-Age=600",
            state
        ),
    )?;

    Response::empty()
        .map(|r| r.with_status(302))
        .map(|r| r.with_headers(headers))
}

/// Handle GitHub OAuth callback
pub async fn handle_callback(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Verify state matches
    let state = params.get("state").ok_or("Missing state parameter")?;
    let cookie_state = get_cookie(&req, "oauth_state")?;

    if state != &cookie_state {
        return Response::error("Invalid state parameter", 400);
    }

    // Exchange code for token
    let code = params.get("code").ok_or("Missing code parameter")?;
    let client_id = ctx.env.var("GITHUB_CLIENT_ID")?.to_string();
    let client_secret = ctx.env.secret("GITHUB_CLIENT_SECRET")?.to_string();
    let redirect_uri = get_redirect_uri(&req)?;

    let token = exchange_code_for_token(&client_id, &client_secret, code, &redirect_uri).await?;

    // Get user info
    let github_user = get_github_user(&token).await?;

    // Verify org/user/team restrictions
    let allowed_orgs: Vec<String> = ctx
        .env
        .var("ALLOWED_ORGS")?
        .to_string()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect();

    let allowed_users: Vec<String> = ctx
        .env
        .var("ALLOWED_USERS")?
        .to_string()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect();

    // Check if user is allowed
    let is_allowed = if allowed_users.is_empty() && allowed_orgs.is_empty() {
        true // No restrictions configured
    } else {
        let user_allowed = allowed_users.contains(&github_user.login);
        let org_allowed = if !allowed_orgs.is_empty() {
            check_org_membership(&token, &allowed_orgs).await?
        } else {
            false
        };
        user_allowed || org_allowed
    };

    if !is_allowed {
        return Response::error("Access denied: not authorized", 403);
    }

    // Create user and session
    let user = crate::models::User::new(github_user.id, github_user.login, github_user.email);
    let session = crate::models::Session::new(user.id.clone(), 24 * 7); // 1 week

    // Store in D1 database
    // Note: D1 binding will be configured when deploying
    if let Ok(db) = ctx.env.d1("DB") {
        // Create or update user
        let _ = db
            .prepare(
                "INSERT INTO users (id, github_id, github_login, email, created_at, last_login)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(github_id) DO UPDATE SET
                     last_login = ?6,
                     github_login = ?3,
                     email = ?4",
            )
            .bind(&[
                user.id.clone().into(),
                user.github_id.into(),
                user.github_login.clone().into(),
                user.email.clone().unwrap_or_default().into(),
                user.created_at.clone().into(),
                user.last_login.clone().unwrap_or_default().into(),
            ])
            .map(|q| q.run());

        // Create session
        let _ = db
            .prepare(
                "INSERT INTO sessions (id, user_id, expires_at, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&[
                session.id.clone().into(),
                session.user_id.clone().into(),
                session.expires_at.clone().into(),
                session.created_at.clone().into(),
            ])
            .map(|q| q.run());
    }

    // Redirect to dashboard with session cookie
    let mut headers = Headers::new();
    headers.set("Location", "/dashboard")?;
    headers.set(
        "Set-Cookie",
        &format!(
            "session={}; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
            session.id,
            7 * 24 * 60 * 60 // 1 week in seconds
        ),
    )?;

    Response::empty()
        .map(|r| r.with_status(302))
        .map(|r| r.with_headers(headers))
}

/// Logout and clear session
pub async fn logout(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    // Clear session cookie
    let mut headers = Headers::new();
    headers.set("Location", "/")?;
    headers.set(
        "Set-Cookie",
        "session=; HttpOnly; Secure; SameSite=Lax; Max-Age=0",
    )?;

    Response::empty()
        .map(|r| r.with_status(302))
        .map(|r| r.with_headers(headers))
}

async fn exchange_code_for_token(
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<String> {
    let body = format!(
        "client_id={}&client_secret={}&code={}&redirect_uri={}",
        client_id, client_secret, code, redirect_uri
    );

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_body(Some(wasm_bindgen::JsValue::from_str(&body)));

    let mut headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Content-Type", "application/x-www-form-urlencoded")?;
    init.with_headers(headers);

    let request = Request::new_with_init(GITHUB_TOKEN_URL, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    let token_response: TokenResponse = response.json().await?;

    Ok(token_response.access_token)
}

async fn get_github_user(token: &str) -> Result<GitHubUser> {
    let mut headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", token))?;
    headers.set("User-Agent", "AI-Orchestrator")?;
    headers.set("Accept", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    init.with_headers(headers);

    let request = Request::new_with_init(GITHUB_USER_URL, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    response.json().await
}

async fn check_org_membership(token: &str, allowed_orgs: &[String]) -> Result<bool> {
    let mut headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", token))?;
    headers.set("User-Agent", "AI-Orchestrator")?;
    headers.set("Accept", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get);
    init.with_headers(headers);

    let request = Request::new_with_init(GITHUB_ORGS_URL, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    let orgs: Vec<GitHubOrg> = response.json().await?;

    Ok(orgs.iter().any(|org| allowed_orgs.contains(&org.login)))
}

fn get_redirect_uri(req: &Request) -> Result<String> {
    let url = req.url()?;
    let scheme = url.scheme();
    let host = url.host_str().ok_or("Missing host")?;
    let port = url.port().map(|p| format!(":{}", p)).unwrap_or_default();
    Ok(format!(
        "{}://{}{}/auth/github/callback",
        scheme, host, port
    ))
}

fn generate_state() -> String {
    use getrandom::getrandom;
    let mut bytes = [0u8; 16];
    getrandom(&mut bytes).expect("Failed to generate random bytes");
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn get_cookie(req: &Request, name: &str) -> Result<String> {
    let cookie_header = req.headers().get("Cookie")?.unwrap_or_default();
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{}=", name)) {
            return Ok(value.to_string());
        }
    }
    Err("Cookie not found".into())
}

fn url_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}
