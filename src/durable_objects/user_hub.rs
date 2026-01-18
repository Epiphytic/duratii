use serde::{Deserialize, Serialize};
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

/// Per-user Durable Object that manages connected claudecodeui instances
#[durable_object]
pub struct UserHub {
    state: State,
    env: Env,
    /// Connected claudecodeui clients
    clients: HashMap<String, ClientConnection>,
    /// Connected browser sessions (for real-time updates)
    browsers: Vec<WebSocket>,
}

struct ClientConnection {
    websocket: WebSocket,
    client: Client,
}

#[durable_object]
impl DurableObject for UserHub {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            clients: HashMap::new(),
            browsers: Vec::new(),
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let path = url.path();

        // Route based on path
        if path == "/ws" {
            self.handle_websocket(req).await
        } else if path == "/clients" {
            self.get_clients_json().await
        } else {
            Response::error("Not found", 404)
        }
    }

    async fn websocket_message(
        &mut self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        match message {
            WebSocketIncomingMessage::String(text) => {
                self.handle_message(&ws, &text).await?;
            }
            WebSocketIncomingMessage::Binary(_) => {
                // Binary messages not supported
                let _ = ws.send_with_str(r#"{"type":"error","message":"Binary messages not supported"}"#);
            }
        }
        Ok(())
    }

    async fn websocket_close(
        &mut self,
        ws: WebSocket,
        _code: usize,
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        // Remove from browsers list
        self.browsers.retain(|b| b != &ws);

        // Remove from clients and broadcast disconnection
        let disconnected_id = self
            .clients
            .iter()
            .find(|(_, conn)| conn.websocket == ws)
            .map(|(id, _)| id.clone());

        if let Some(client_id) = disconnected_id {
            self.clients.remove(&client_id);

            // Persist to SQLite
            self.remove_client_from_storage(&client_id).await?;

            // Broadcast disconnection to browsers
            if let Ok(msg) = serde_json::to_string(&WsMessage::ClientDisconnected { client_id }) {
                self.broadcast_to_browsers(&msg);
            }
        }

        Ok(())
    }

    async fn websocket_error(&mut self, ws: WebSocket, error: Error) -> Result<()> {
        console_log!("WebSocket error: {:?}", error);
        // Treat errors as disconnections
        self.websocket_close(ws, 1006, "Error".to_string(), false)
            .await
    }
}

impl UserHub {
    async fn handle_websocket(&mut self, _req: Request) -> Result<Response> {
        let pair = WebSocketPair::new()?;
        let server = pair.server;
        let client = pair.client;

        // Accept the connection
        server.accept()?;

        // All connections start as potential claudecodeui clients
        // Browser connections will identify themselves via message

        Response::from_websocket(client)
    }

    async fn handle_message(&mut self, ws: &WebSocket, text: &str) -> Result<()> {
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

                // Persist to SQLite first
                self.save_client_to_storage(&client).await?;

                // Broadcast update to browsers
                if let Ok(json) = serde_json::to_string(&WsMessage::ClientUpdate {
                    client: client.clone(),
                }) {
                    self.broadcast_to_browsers(&json);
                }

                // Store connection
                self.clients.insert(
                    client_id,
                    ClientConnection {
                        websocket: ws.clone(),
                        client,
                    },
                );
            }

            WsMessage::StatusUpdate { client_id, status } => {
                // Get client, update, and extract data before persisting
                if let Some(conn) = self.clients.get_mut(&client_id) {
                    conn.client.update_status(status);
                    conn.client.update_last_seen();
                    let client_clone = conn.client.clone();

                    // Persist update
                    self.save_client_to_storage(&client_clone).await?;

                    // Broadcast to browsers
                    if let Ok(json) =
                        serde_json::to_string(&WsMessage::ClientUpdate { client: client_clone })
                    {
                        self.broadcast_to_browsers(&json);
                    }
                }
            }

            WsMessage::Ping { client_id } => {
                if let Some(conn) = self.clients.get_mut(&client_id) {
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
                if !self.browsers.contains(ws) {
                    self.browsers.push(ws.clone());
                }

                let clients: Vec<Client> =
                    self.clients.values().map(|c| c.client.clone()).collect();
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

    async fn get_clients_json(&self) -> Result<Response> {
        let clients: Vec<Client> = self.clients.values().map(|c| c.client.clone()).collect();
        Response::from_json(&clients)
    }

    fn broadcast_to_browsers(&self, message: &str) {
        for browser in &self.browsers {
            let _ = browser.send_with_str(message);
        }
    }

    async fn save_client_to_storage(&self, client: &Client) -> Result<()> {
        self.state
            .storage()
            .put(&format!("client:{}", client.id), client)
            .await?;
        Ok(())
    }

    async fn remove_client_from_storage(&self, client_id: &str) -> Result<()> {
        self.state
            .storage()
            .delete(&format!("client:{}", client_id))
            .await?;
        Ok(())
    }
}
