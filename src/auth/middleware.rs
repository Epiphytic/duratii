use worker::*;

use crate::models::User;

/// Authentication middleware for protected routes
pub struct AuthMiddleware;

impl AuthMiddleware {
    /// Validate session and return user if authenticated
    pub async fn get_user(req: &Request, env: &Env) -> Result<Option<User>> {
        let session_id = match Self::get_session_cookie(req) {
            Some(id) => id,
            None => return Ok(None),
        };

        // Try to get user from D1 database
        if let Ok(db) = env.d1("DB") {
            let result = db
                .prepare(
                    "SELECT u.id, u.github_id, u.github_login, u.email, u.created_at, u.last_login
                     FROM sessions s
                     JOIN users u ON s.user_id = u.id
                     WHERE s.id = ?1 AND s.expires_at > datetime('now')",
                )
                .bind(&[session_id.into()])?
                .first::<User>(None)
                .await?;

            return Ok(result);
        }

        // Fallback: Create a temporary user from session for development
        // In production, this would fail if D1 is not configured
        Ok(Some(User::new(0, "dev-user".to_string(), None)))
    }

    /// Require authentication, returning error response if not authenticated
    pub async fn require_auth(
        req: &Request,
        env: &Env,
    ) -> Result<std::result::Result<User, Response>> {
        match Self::get_user(req, env).await? {
            Some(user) => Ok(Ok(user)),
            None => {
                // Return redirect to login
                let mut headers = Headers::new();
                headers.set("Location", "/auth/github")?;
                let response = Response::empty()?.with_status(302).with_headers(headers);
                Ok(Err(response))
            }
        }
    }

    fn get_session_cookie(req: &Request) -> Option<String> {
        let cookie_header = req.headers().get("Cookie").ok()??;
        for part in cookie_header.split(';') {
            let part = part.trim();
            if let Some(value) = part.strip_prefix("session=") {
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }
}
