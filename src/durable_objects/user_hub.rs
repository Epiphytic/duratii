use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use worker::{SqlStorageValue, *};

use crate::models::{Client, ClientMetadata, ClientStatus};

/// Row structure for deserializing SQLite client rows
#[derive(Debug, Deserialize)]
struct ClientRow {
    client_id: String,
    user_id: String,
    hostname: String,
    project: String,
    status: String,
    last_activity: Option<String>,
    connected_at: String,
    last_seen: String,
}

/// Message types for WebSocket communication
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    /// Client registration from claudecodeui
    Register {
        client_id: String,
        user_token: String,
        metadata: ClientMetadata,
    },
    /// Registration response
    Registered {
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// Status update from client
    StatusUpdate {
        client_id: String,
        status: ClientStatus,
    },
    /// Heartbeat/ping
    Ping { client_id: String },
    /// Pong response
    Pong { client_id: String },
    /// Client list request (from browser)
    GetClients,
    /// Client list response
    ClientList { clients: Vec<Client> },
    /// Single client update broadcast
    ClientUpdate { client: Client },
    /// Client disconnected
    ClientDisconnected { client_id: String },
    /// Error message
    Error { message: String },
}

struct ClientConnection {
    websocket: WebSocket,
    client: Client,
}

/// Per-user Durable Object that manages connected claudecodeui instances
#[durable_object]
pub struct UserHub {
    state: State,
    #[allow(dead_code)]
    env: Env,
    /// Connected claudecodeui clients (using RefCell for interior mutability)
    clients: RefCell<HashMap<String, ClientConnection>>,
    /// Connected browser sessions (for real-time updates)
    browsers: RefCell<Vec<WebSocket>>,
    /// Whether SQLite storage has been initialized
    initialized: RefCell<bool>,
}

impl DurableObject for UserHub {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            clients: RefCell::new(HashMap::new()),
            browsers: RefCell::new(Vec::new()),
            initialized: RefCell::new(false),
        }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let path = url.path();

        // Route based on path
        if path == "/ws" {
            self.handle_websocket(req).await
        } else if path == "/clients" {
            self.get_clients_json()
        } else if path.starts_with("/clients/") && path.ends_with("/disconnect") {
            // Extract client_id from /clients/{id}/disconnect
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() >= 3 {
                let client_id = parts[2];
                self.disconnect_client(client_id)
            } else {
                Response::error("Invalid path", 400)
            }
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
                self.handle_message(&ws, &text)?;
            }
            WebSocketIncomingMessage::Binary(_) => {
                // Binary messages not supported
                let error = WsMessage::Error {
                    message: "Binary messages not supported".to_string(),
                };
                if let Ok(json) = serde_json::to_string(&error) {
                    let _ = ws.send_with_str(&json);
                }
            }
        }
        Ok(())
    }

    /// Handle WebSocket close events (hibernation API)
    async fn websocket_close(
        &self,
        ws: WebSocket,
        _code: usize,
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        self.handle_close(&ws);
        Ok(())
    }

    /// Handle WebSocket errors (hibernation API)
    async fn websocket_error(&self, ws: WebSocket, _error: Error) -> Result<()> {
        // Treat errors as disconnections
        self.handle_close(&ws);
        Ok(())
    }
}

impl UserHub {
    /// Initialize SQLite schema if not already done
    fn ensure_initialized(&self) -> Result<()> {
        if *self.initialized.borrow() {
            return Ok(());
        }

        let sql = self.state.storage().sql();
        sql.exec(
            "CREATE TABLE IF NOT EXISTS clients (
                client_id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                hostname TEXT NOT NULL,
                project TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'idle',
                last_activity TEXT,
                connected_at TEXT NOT NULL,
                last_seen TEXT NOT NULL
            )",
            None,
        )?;

        *self.initialized.borrow_mut() = true;
        Ok(())
    }

    /// Save client to SQLite
    fn save_client(&self, client: &Client) -> Result<()> {
        self.ensure_initialized()?;
        let sql = self.state.storage().sql();

        sql.exec(
            "INSERT OR REPLACE INTO clients (client_id, user_id, hostname, project, status, last_activity, connected_at, last_seen)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            Some(vec![
                SqlStorageValue::String(client.id.clone()),
                SqlStorageValue::String(client.user_id.clone()),
                SqlStorageValue::String(client.metadata.hostname.clone()),
                SqlStorageValue::String(client.metadata.project.clone()),
                SqlStorageValue::String(client.metadata.status.to_string()),
                client.metadata.last_activity.clone().map(SqlStorageValue::String).unwrap_or(SqlStorageValue::Null),
                SqlStorageValue::String(client.connected_at.clone()),
                SqlStorageValue::String(client.last_seen.clone()),
            ]),
        )?;

        Ok(())
    }

    /// Delete client from SQLite
    fn delete_client(&self, client_id: &str) -> Result<()> {
        self.ensure_initialized()?;
        let sql = self.state.storage().sql();

        sql.exec(
            "DELETE FROM clients WHERE client_id = ?",
            Some(vec![SqlStorageValue::String(client_id.to_string())]),
        )?;

        Ok(())
    }

    /// Load all clients from SQLite (for recovery after hibernation)
    fn load_clients_from_sqlite(&self) -> Result<Vec<Client>> {
        self.ensure_initialized()?;
        let sql = self.state.storage().sql();

        let cursor = sql.exec(
            "SELECT client_id, user_id, hostname, project, status, last_activity, connected_at, last_seen FROM clients",
            None,
        )?;

        // Use typed deserialization
        let rows: Vec<ClientRow> = cursor.to_array()?;
        let mut clients = Vec::new();

        for row_value in rows {
            let status = match row_value.status.as_str() {
                "active" => ClientStatus::Active,
                "busy" => ClientStatus::Busy,
                "disconnected" => ClientStatus::Disconnected,
                _ => ClientStatus::Idle,
            };

            clients.push(Client {
                id: row_value.client_id,
                user_id: row_value.user_id,
                metadata: ClientMetadata {
                    hostname: row_value.hostname,
                    project: row_value.project,
                    status,
                    last_activity: row_value.last_activity,
                },
                connected_at: row_value.connected_at,
                last_seen: row_value.last_seen,
            });
        }

        Ok(clients)
    }

    /// Restore in-memory state from SQLite and WebSockets after hibernation
    fn restore_state(&self) -> Result<()> {
        // Get active WebSockets from hibernation API
        let websockets = self.state.get_websockets();

        // Load client data from SQLite
        let stored_clients = self.load_clients_from_sqlite()?;
        let client_map: HashMap<String, Client> = stored_clients
            .into_iter()
            .map(|c| (c.id.clone(), c))
            .collect();

        // Match WebSockets with their client data using tags
        let mut clients = self.clients.borrow_mut();
        let mut browsers = self.browsers.borrow_mut();

        for ws in websockets {
            let tags = self.state.get_tags(&ws);
            if tags.iter().any(|t| t == "browser") {
                browsers.push(ws);
            } else if let Some(client_id) = tags.first() {
                if let Some(client) = client_map.get(client_id) {
                    clients.insert(
                        client_id.clone(),
                        ClientConnection {
                            websocket: ws,
                            client: client.clone(),
                        },
                    );
                }
            }
        }

        Ok(())
    }

    async fn handle_websocket(&self, req: Request) -> Result<Response> {
        // Get the upgrade header
        let upgrade = req.headers().get("Upgrade")?;
        if upgrade.as_deref() != Some("websocket") {
            return Response::error("Expected websocket", 426);
        }

        // Check if this is a browser connection (via query param)
        let url = req.url()?;
        let is_browser = url.query_pairs().any(|(k, v)| k == "type" && v == "browser");

        let pair = WebSocketPair::new()?;
        let server = pair.server;
        let client = pair.client;

        // Accept the connection
        server.accept()?;

        // Set up event handlers using the hibernation API with tags
        if is_browser {
            self.state.accept_websocket_with_tags(&server, &["browser"]);
        } else {
            // Client connections get tagged after registration
            self.state.accept_web_socket(&server);
        }

        Response::from_websocket(client)
    }

    /// Ensure state is restored after hibernation
    fn ensure_state_restored(&self) -> Result<()> {
        // Only restore if clients map is empty but we have websockets
        if self.clients.borrow().is_empty() {
            let websockets = self.state.get_websockets();
            if !websockets.is_empty() {
                self.restore_state()?;
            }
        }
        Ok(())
    }

    fn handle_message(&self, ws: &WebSocket, text: &str) -> Result<()> {
        // Restore state if waking from hibernation
        let _ = self.ensure_state_restored();

        let msg: WsMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                let error = WsMessage::Error {
                    message: format!("Invalid message format: {}", e),
                };
                if let Ok(json) = serde_json::to_string(&error) {
                    let _ = ws.send_with_str(&json);
                }
                return Ok(());
            }
        };

        match msg {
            WsMessage::Register {
                client_id,
                user_token: _,
                metadata,
            } => {
                // Create client
                let user_id = self.state.id().to_string();
                let client = Client::new(client_id.clone(), user_id, metadata);

                // Save to SQLite for persistence
                let _ = self.save_client(&client);

                // Tag the WebSocket with the client ID for hibernation recovery
                // Note: We need to re-accept with tags, but that's not possible after accept
                // So we track the mapping in SQLite instead

                // Send registration success response
                let registered = WsMessage::Registered {
                    success: true,
                    message: None,
                };
                if let Ok(json) = serde_json::to_string(&registered) {
                    let _ = ws.send_with_str(&json);
                }

                // Broadcast update to browsers
                if let Ok(json) = serde_json::to_string(&WsMessage::ClientUpdate {
                    client: client.clone(),
                }) {
                    self.broadcast_to_browsers(&json);
                }

                // Store connection in memory
                self.clients.borrow_mut().insert(
                    client_id,
                    ClientConnection {
                        websocket: ws.clone(),
                        client,
                    },
                );
            }

            WsMessage::StatusUpdate { client_id, status } => {
                // Get client, update, and extract data
                let mut clients = self.clients.borrow_mut();
                if let Some(conn) = clients.get_mut(&client_id) {
                    conn.client.update_status(status);
                    conn.client.update_last_seen();
                    let client_clone = conn.client.clone();

                    // Update SQLite
                    let _ = self.save_client(&client_clone);

                    // Broadcast to browsers
                    if let Ok(json) =
                        serde_json::to_string(&WsMessage::ClientUpdate { client: client_clone })
                    {
                        drop(clients); // Release borrow before broadcasting
                        self.broadcast_to_browsers(&json);
                    }
                }
            }

            WsMessage::Ping { client_id } => {
                let client_to_save = {
                    let mut clients = self.clients.borrow_mut();
                    if let Some(conn) = clients.get_mut(&client_id) {
                        conn.client.update_last_seen();
                        Some(conn.client.clone())
                    } else {
                        None
                    }
                };

                // Update last_seen in SQLite periodically (on pings)
                if let Some(client) = client_to_save {
                    let _ = self.save_client(&client);
                }

                let pong = WsMessage::Pong { client_id };
                if let Ok(json) = serde_json::to_string(&pong) {
                    let _ = ws.send_with_str(&json);
                }
            }

            WsMessage::GetClients => {
                // This is a browser requesting the client list
                // Add it to browsers if not already there
                {
                    let mut browsers = self.browsers.borrow_mut();
                    if !browsers.contains(ws) {
                        browsers.push(ws.clone());
                    }
                }

                let clients: Vec<Client> = self
                    .clients
                    .borrow()
                    .values()
                    .map(|c| c.client.clone())
                    .collect();
                let response = WsMessage::ClientList { clients };
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = ws.send_with_str(&json);
                }
            }

            _ => {
                // Other message types not handled here
            }
        }

        Ok(())
    }

    fn handle_close(&self, ws: &WebSocket) {
        // Remove from browsers list
        self.browsers.borrow_mut().retain(|b| b != ws);

        // Remove from clients and broadcast disconnection
        let disconnected_id = {
            let clients = self.clients.borrow();
            clients
                .iter()
                .find(|(_, conn)| &conn.websocket == ws)
                .map(|(id, _)| id.clone())
        };

        if let Some(client_id) = disconnected_id {
            self.clients.borrow_mut().remove(&client_id);

            // Remove from SQLite
            let _ = self.delete_client(&client_id);

            // Broadcast disconnection to browsers
            if let Ok(msg) =
                serde_json::to_string(&WsMessage::ClientDisconnected { client_id })
            {
                self.broadcast_to_browsers(&msg);
            }
        }
    }

    fn get_clients_json(&self) -> Result<Response> {
        let clients: Vec<Client> = self
            .clients
            .borrow()
            .values()
            .map(|c| c.client.clone())
            .collect();
        Response::from_json(&clients)
    }

    fn broadcast_to_browsers(&self, message: &str) {
        for browser in self.browsers.borrow().iter() {
            let _ = browser.send_with_str(message);
        }
    }

    /// Disconnect a specific client by ID
    fn disconnect_client(&self, client_id: &str) -> Result<Response> {
        // Restore state if needed
        let _ = self.ensure_state_restored();

        // Find and remove the client
        let connection = self.clients.borrow_mut().remove(client_id);

        if let Some(conn) = connection {
            // Send disconnect command to the client
            let disconnect_msg = WsMessage::Error {
                message: "Disconnected by user".to_string(),
            };
            if let Ok(json) = serde_json::to_string(&disconnect_msg) {
                let _ = conn.websocket.send_with_str(&json);
            }
            // Close the WebSocket
            let _ = conn.websocket.close(Some(1000), Some("Disconnected by user"));

            // Delete from SQLite
            let _ = self.delete_client(client_id);

            // Broadcast disconnection to browsers
            if let Ok(msg) =
                serde_json::to_string(&WsMessage::ClientDisconnected {
                    client_id: client_id.to_string(),
                })
            {
                self.broadcast_to_browsers(&msg);
            }

            Response::ok("Client disconnected")
        } else {
            Response::error("Client not found", 404)
        }
    }
}
