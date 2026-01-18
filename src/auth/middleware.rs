use worker::*;

use super::{base64_decode, SessionData};
use crate::models::User;

/// Authentication middleware for protected routes
pub struct AuthMiddleware;

impl AuthMiddleware {
    /// Validate session and return user if authenticated
    pub async fn get_user(req: &Request, _env: &Env) -> Result<Option<User>> {
        let session_cookie = match Self::get_session_cookie(req) {
            Some(cookie) => cookie,
            None => return Ok(None),
        };

        // Decode and parse session data from cookie
        let session_json = match base64_decode(&session_cookie) {
            Ok(json) => json,
            Err(_) => return Ok(None),
        };

        let session_data: SessionData = match serde_json::from_str(&session_json) {
            Ok(data) => data,
            Err(_) => return Ok(None),
        };

        // Check expiration (simple string comparison works for ISO 8601)
        let now = current_timestamp();
        if session_data.expires_at < now {
            return Ok(None);
        }

        // Create user from session data
        Ok(Some(User::new(
            session_data.github_id,
            session_data.github_login,
            None,
        )))
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
                let headers = Headers::new();
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

fn current_timestamp() -> String {
    let now = js_sys::Date::now();
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(now));
    date.to_iso_string().as_string().unwrap_or_default()
}

mod js_sys {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        pub type Date;

        #[wasm_bindgen(constructor)]
        pub fn new(value: &JsValue) -> Date;

        #[wasm_bindgen(static_method_of = Date)]
        pub fn now() -> f64;

        #[wasm_bindgen(method, js_name = toISOString)]
        pub fn to_iso_string(this: &Date) -> JsString;
    }

    #[wasm_bindgen]
    extern "C" {
        pub type JsString;

        #[wasm_bindgen(method, js_name = toString)]
        pub fn as_string(this: &JsString) -> Option<String>;
    }
}
