use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use worker::*;

use crate::models::hash_token;

/// Pending client waiting for authorization
#[derive(Clone)]
struct PendingClient {
    websocket: WebSocket,
    pending_id: String,
    hostname: String,
    project: String,
    platform: String,
    ip_address: Option<String>,
    country: Option<String>,
    city: Option<String>,
    region: Option<String>,
    connected_at: i64, // Unix timestamp for expiration check
    // Claim patterns - who can authorize this client
    allowed_users: Vec<String>,
    allowed_orgs: Vec<String>,
    allowed_teams: Vec<String>, // Format: "org/team-slug"
}

/// Info about a pending client (for API responses)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingClientInfo {
    pub pending_id: String,
    pub hostname: String,
    pub project: String,
    pub platform: String,
    pub ip_address: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub connected_at: i64,
    pub allowed_users: Vec<String>,
    pub allowed_orgs: Vec<String>,
    pub allowed_teams: Vec<String>,
}

impl From<&PendingClient> for PendingClientInfo {
    fn from(client: &PendingClient) -> Self {
        Self {
            pending_id: client.pending_id.clone(),
            hostname: client.hostname.clone(),
            project: client.project.clone(),
            platform: client.platform.clone(),
            ip_address: client.ip_address.clone(),
            country: client.country.clone(),
            city: client.city.clone(),
            region: client.region.clone(),
            connected_at: client.connected_at,
            allowed_users: client.allowed_users.clone(),
            allowed_orgs: client.allowed_orgs.clone(),
            allowed_teams: client.allowed_teams.clone(),
        }
    }
}

/// Message types for pending client WebSocket communication
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PendingWsMessage {
    /// Pending client registration (no token)
    PendingRegister {
        pending_id: String,
        hostname: String,
        project: String,
        platform: String,
    },
    /// Registration acknowledgment
    PendingRegistered { success: bool, message: Option<String> },
    /// Ping/pong for keepalive
    Ping { pending_id: String },
    Pong { pending_id: String },
    /// Token granted (to claudecodeui) - authorization successful
    TokenGranted { token: String, client_id: String },
    /// Authorization denied
    AuthorizationDenied { reason: String },
    /// Authorization timeout (10 minutes expired)
    AuthorizationTimeout { message: String },
    /// Error message
    Error { message: String },
}

/// Global Durable Object for pending (unauthenticated) clients
#[durable_object]
pub struct PendingHub {
    state: State,
    env: Env,
    /// Pending clients waiting for authorization
    clients: RefCell<HashMap<String, PendingClient>>,
    /// Whether we have an alarm set for cleanup
    alarm_set: RefCell<bool>,
}

const PENDING_TIMEOUT_MS: i64 = 10 * 60 * 1000; // 10 minutes

impl DurableObject for PendingHub {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            clients: RefCell::new(HashMap::new()),
            alarm_set: RefCell::new(false),
        }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let path = url.path();

        if path == "/ws" {
            self.handle_websocket(req).await
        } else if path == "/pending" {
            // Get all pending clients (for dashboard query)
            self.get_pending_clients_json(&req)
        } else if path.starts_with("/claim/") {
            // Claim a pending client
            let pending_id = path.strip_prefix("/claim/").unwrap_or("");
            self.handle_claim(req, pending_id).await
        } else {
            Response::error("Not found", 404)
        }
    }

    /// Handle incoming WebSocket messages (hibernation API)
    async fn websocket_message(
        &self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        match message {
            WebSocketIncomingMessage::String(text) => {
                self.handle_message(&ws, &text).await?;
            }
            WebSocketIncomingMessage::Binary(_) => {
                let error = PendingWsMessage::Error {
                    message: "Binary messages not supported".to_string(),
                };
                if let Ok(json) = serde_json::to_string(&error) {
                    let _ = ws.send_with_str(&json);
                }
            }
        }
        Ok(())
    }

    /// Handle WebSocket close events
    async fn websocket_close(
        &self,
        ws: WebSocket,
        _code: usize,
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        self.handle_close(&ws).await;
        Ok(())
    }

    /// Handle WebSocket errors
    async fn websocket_error(&self, ws: WebSocket, _error: Error) -> Result<()> {
        self.handle_close(&ws).await;
        Ok(())
    }

    /// Handle alarm for cleanup of expired pending clients
    async fn alarm(&self) -> Result<Response> {
        self.cleanup_expired().await;
        *self.alarm_set.borrow_mut() = false;

        // Check if we still have pending clients and need to reschedule
        if !self.clients.borrow().is_empty() {
            self.schedule_cleanup_alarm().await?;
        }

        Response::ok("OK")
    }
}

impl PendingHub {
    async fn handle_websocket(&self, req: Request) -> Result<Response> {
        let upgrade = req.headers().get("Upgrade")?;
        if upgrade.as_deref() != Some("websocket") {
            return Response::error("Expected websocket", 426);
        }

        // Parse query parameters for claim patterns and geo info
        let url = req.url()?;
        let params: HashMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        // Extract allowed users/orgs/teams from query params
        let allowed_users: Vec<String> = params
            .get("user")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        let allowed_orgs: Vec<String> = params
            .get("org")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        let allowed_teams: Vec<String> = params
            .get("team")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();

        // Require at least one claim pattern
        if allowed_users.is_empty() && allowed_orgs.is_empty() && allowed_teams.is_empty() {
            return Response::error("At least one claim pattern required (user, org, or team)", 400);
        }

        // Extract geo info from query params
        let ip_address = params.get("ip").cloned();
        let country = params.get("country").cloned();
        let city = params.get("city").cloned();
        let region = params.get("region").cloned();

        // Generate a unique pending_id
        let pending_id = generate_pending_id();

        let pair = WebSocketPair::new()?;
        let server = pair.server;
        let client = pair.client;

        // Accept WebSocket with tag for identification
        self.state.accept_websocket_with_tags(&server, &[&pending_id]);

        // Store pending client info for when we receive the PendingRegister message
        // We need to store the claim patterns and geo info
        let now = current_timestamp_ms();
        let pending_client = PendingClient {
            websocket: server,
            pending_id: pending_id.clone(),
            hostname: String::new(), // Will be filled in by PendingRegister
            project: String::new(),
            platform: String::new(),
            ip_address,
            country,
            city,
            region,
            connected_at: now,
            allowed_users,
            allowed_orgs,
            allowed_teams,
        };

        self.clients.borrow_mut().insert(pending_id, pending_client);

        // Schedule cleanup alarm if not already set
        self.schedule_cleanup_alarm().await?;

        Response::from_websocket(client)
    }

    async fn handle_message(&self, ws: &WebSocket, text: &str) -> Result<()> {
        let msg: PendingWsMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                let error = PendingWsMessage::Error {
                    message: format!("Invalid message format: {}", e),
                };
                if let Ok(json) = serde_json::to_string(&error) {
                    let _ = ws.send_with_str(&json);
                }
                return Ok(());
            }
        };

        match msg {
            PendingWsMessage::PendingRegister {
                pending_id,
                hostname,
                project,
                platform,
            } => {
                // Update the pending client with the registration info
                let mut clients = self.clients.borrow_mut();
                if let Some(client) = clients.get_mut(&pending_id) {
                    client.hostname = hostname;
                    client.project = project;
                    client.platform = platform;

                    let response = PendingWsMessage::PendingRegistered {
                        success: true,
                        message: Some("Waiting for authorization. A user with access will see you in their dashboard.".to_string()),
                    };
                    if let Ok(json) = serde_json::to_string(&response) {
                        let _ = ws.send_with_str(&json);
                    }
                } else {
                    let response = PendingWsMessage::PendingRegistered {
                        success: false,
                        message: Some("Pending session not found. Please reconnect.".to_string()),
                    };
                    if let Ok(json) = serde_json::to_string(&response) {
                        let _ = ws.send_with_str(&json);
                    }
                }
            }

            PendingWsMessage::Ping { pending_id } => {
                let pong = PendingWsMessage::Pong { pending_id };
                if let Ok(json) = serde_json::to_string(&pong) {
                    let _ = ws.send_with_str(&json);
                }
            }

            _ => {
                // Other message types are outbound only
            }
        }

        Ok(())
    }

    async fn handle_close(&self, ws: &WebSocket) {
        // Find and remove the client associated with this WebSocket
        let mut clients = self.clients.borrow_mut();
        let pending_id = clients
            .iter()
            .find(|(_, c)| &c.websocket == ws)
            .map(|(id, _)| id.clone());

        if let Some(id) = pending_id {
            clients.remove(&id);
        }
    }

    fn get_pending_clients_json(&self, req: &Request) -> Result<Response> {
        // Parse query params to filter by user identity
        let url = req.url()?;
        let params: HashMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let github_login = params.get("github_login");
        let user_orgs: Vec<String> = params
            .get("orgs")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        let user_teams: Vec<String> = params
            .get("teams")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();

        // Filter pending clients that this user can claim
        let clients: Vec<PendingClientInfo> = self
            .clients
            .borrow()
            .values()
            .filter(|c| {
                // Check if user matches any of the claim patterns
                let user_match = github_login
                    .map(|login| c.allowed_users.iter().any(|u| u.eq_ignore_ascii_case(login)))
                    .unwrap_or(false);

                let org_match = c.allowed_orgs.iter().any(|org| {
                    user_orgs.iter().any(|user_org| user_org.eq_ignore_ascii_case(org))
                });

                let team_match = c.allowed_teams.iter().any(|team| {
                    user_teams.iter().any(|user_team| user_team.eq_ignore_ascii_case(team))
                });

                user_match || org_match || team_match
            })
            .map(PendingClientInfo::from)
            .collect();

        Response::from_json(&clients)
    }

    async fn handle_claim(&self, mut req: Request, pending_id: &str) -> Result<Response> {
        // Parse the claim request body
        #[derive(Deserialize)]
        struct ClaimRequest {
            user_id: String,
            name: String,
        }

        let body: ClaimRequest = req.json().await?;

        // Find the pending client
        let pending_client = self.clients.borrow_mut().remove(pending_id);

        let client = match pending_client {
            Some(c) => c,
            None => return Response::error("Pending client not found", 404),
        };

        // Generate a token for this client (format: ao_<id>_<raw_token>)
        let (token_id, raw_token, token_hash) = generate_token();
        let full_token = format!("ao_{}_{}", token_id, raw_token);

        // Store token in D1
        let db = self.env.d1("DB")?;
        db.prepare(
            "INSERT INTO client_tokens (id, user_id, name, token_hash, created_at) VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)",
        )
        .bind(&[
            token_id.clone().into(),
            body.user_id.clone().into(),
            body.name.into(),
            token_hash.into(),
        ])?
        .run()
        .await?;

        // Generate a client_id for the new authorized client
        let client_id = generate_client_id();

        // Send the token to the pending client over WebSocket
        let token_granted = PendingWsMessage::TokenGranted {
            token: full_token.clone(),
            client_id: client_id.clone(),
        };
        if let Ok(json) = serde_json::to_string(&token_granted) {
            let _ = client.websocket.send_with_str(&json);
        }

        // Return success response with the token info (for dashboard notification)
        #[derive(Serialize)]
        struct ClaimResponse {
            success: bool,
            client_id: String,
            token_id: String,
        }

        Response::from_json(&ClaimResponse {
            success: true,
            client_id,
            token_id,
        })
    }

    async fn cleanup_expired(&self) {
        let now = current_timestamp_ms();
        let expired_threshold = now - PENDING_TIMEOUT_MS;

        let expired_ids: Vec<String> = self
            .clients
            .borrow()
            .iter()
            .filter(|(_, c)| c.connected_at < expired_threshold)
            .map(|(id, _)| id.clone())
            .collect();

        let mut clients = self.clients.borrow_mut();
        for id in expired_ids {
            if let Some(client) = clients.remove(&id) {
                // Send timeout message before closing
                let timeout_msg = PendingWsMessage::AuthorizationTimeout {
                    message: "Authorization timed out after 10 minutes. Please try again.".to_string(),
                };
                if let Ok(json) = serde_json::to_string(&timeout_msg) {
                    let _ = client.websocket.send_with_str(&json);
                }
                let _ = client.websocket.close(Some(4000), Some("Authorization timeout"));
            }
        }
    }

    async fn schedule_cleanup_alarm(&self) -> Result<()> {
        if *self.alarm_set.borrow() {
            return Ok(());
        }

        // Schedule alarm for 10 minutes from now
        let alarm_time = current_timestamp_ms() + PENDING_TIMEOUT_MS;
        self.state
            .storage()
            .set_alarm(std::time::Duration::from_millis(alarm_time as u64))
            .await?;
        *self.alarm_set.borrow_mut() = true;

        Ok(())
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> i64 {
    js_sys::Date::now() as i64
}

/// Generate a unique pending ID
fn generate_pending_id() -> String {
    let mut bytes = [0u8; 12];
    getrandom::getrandom(&mut bytes).unwrap_or_default();
    format!("pending_{}", bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>())
}

/// Generate a unique client ID
fn generate_client_id() -> String {
    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes).unwrap_or_default();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Generate a token (id, raw_value, hash) using the same format as models/token.rs
fn generate_token() -> (String, String, String) {
    let mut id_bytes = [0u8; 8];
    let mut token_bytes = [0u8; 32];
    getrandom::getrandom(&mut id_bytes).unwrap_or_default();
    getrandom::getrandom(&mut token_bytes).unwrap_or_default();

    let id: String = id_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let raw_token: String = token_bytes.iter().map(|b| format!("{:02x}", b)).collect();

    // Use the same hash function as the rest of the codebase
    let token_hash = hash_token(&raw_token);

    (id, raw_token, token_hash)
}

mod js_sys {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        pub type Date;

        #[wasm_bindgen(static_method_of = Date)]
        pub fn now() -> f64;
    }
}
