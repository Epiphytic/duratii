use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use worker::*;

use crate::models::{Client, ClientMetadata, ClientStatus};

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
}

impl DurableObject for UserHub {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            clients: RefCell::new(HashMap::new()),
            browsers: RefCell::new(Vec::new()),
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
        } else {
            Response::error("Not found", 404)
        }
    }
}

impl UserHub {
    async fn handle_websocket(&self, req: Request) -> Result<Response> {
        // Get the upgrade header
        let upgrade = req.headers().get("Upgrade")?;
        if upgrade.as_deref() != Some("websocket") {
            return Response::error("Expected websocket", 426);
        }

        let pair = WebSocketPair::new()?;
        let server = pair.server;
        let client = pair.client;

        // Accept the connection
        server.accept()?;

        // Set up event handlers using the hibernation API
        self.state.accept_web_socket(&server);

        Response::from_websocket(client)
    }

    fn handle_message(&self, ws: &WebSocket, text: &str) -> Result<()> {
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

                // Broadcast update to browsers
                if let Ok(json) = serde_json::to_string(&WsMessage::ClientUpdate {
                    client: client.clone(),
                }) {
                    self.broadcast_to_browsers(&json);
                }

                // Store connection
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
                if let Some(conn) = self.clients.borrow_mut().get_mut(&client_id) {
                    conn.client.update_last_seen();
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
}
