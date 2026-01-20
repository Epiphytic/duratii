use worker::*;

use crate::auth::AuthMiddleware;
use crate::models::Client;
use crate::templates;

/// Get all clients for the current user (returns HTMX partial)
pub async fn get_clients(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Check authentication
    let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
        Ok(user) => user,
        Err(redirect) => return Ok(redirect),
    };

    // Get the user's Durable Object
    let namespace = ctx.env.durable_object("USER_HUB")?;
    let id = namespace.id_from_name(&user.id)?;
    let stub = id.get_stub()?;

    // Fetch clients from DO
    let do_req = Request::new("https://do/clients", Method::Get)?;
    let mut response = stub.fetch_with_request(do_req).await?;

    let clients: Vec<Client> = response.json().await.unwrap_or_default();

    // Check if this is an HTMX request
    let is_htmx = req.headers().get("HX-Request")?.is_some();

    if is_htmx {
        // Return just the client list partial
        Response::from_html(templates::render_client_list(&clients))
    } else {
        // Return full page with client list
        Response::from_html(templates::render_clients_page(&user, &clients))
    }
}

/// Get a single client by ID (returns HTMX partial - collapsed card)
pub async fn get_client(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Check authentication
    let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
        Ok(user) => user,
        Err(redirect) => return Ok(redirect),
    };

    let client_id = ctx.param("id").ok_or("Missing client ID")?;

    // Get the user's Durable Object
    let namespace = ctx.env.durable_object("USER_HUB")?;
    let id = namespace.id_from_name(&user.id)?;
    let stub = id.get_stub()?;

    // Fetch clients from DO
    let do_req = Request::new("https://do/clients", Method::Get)?;
    let mut response = stub.fetch_with_request(do_req).await?;

    let clients: Vec<Client> = response.json().await.unwrap_or_default();
    let client = clients.into_iter().find(|c| &c.id == client_id);

    match client {
        Some(c) => Response::from_html(templates::render_client_card(&c)),
        None => Response::error("Client not found", 404),
    }
}

/// Get expanded client details (returns HTMX partial - expanded card)
pub async fn get_client_details(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Check authentication
    let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
        Ok(user) => user,
        Err(redirect) => return Ok(redirect),
    };

    let client_id = ctx.param("id").ok_or("Missing client ID")?;

    // Get the user's Durable Object
    let namespace = ctx.env.durable_object("USER_HUB")?;
    let id = namespace.id_from_name(&user.id)?;
    let stub = id.get_stub()?;

    // Fetch clients from DO
    let do_req = Request::new("https://do/clients", Method::Get)?;
    let mut response = stub.fetch_with_request(do_req).await?;

    let clients: Vec<Client> = response.json().await.unwrap_or_default();
    let client = clients.into_iter().find(|c| &c.id == client_id);

    match client {
        Some(c) => Response::from_html(templates::render_client_details(&c)),
        None => Response::error("Client not found", 404),
    }
}

/// Get WebSocket connection info for a client (for direct WebSocket connection)
/// Returns the callback URL converted to WebSocket URL so frontend can connect directly
pub async fn get_client_ws_info(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Check authentication
    let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
        Ok(user) => user,
        Err(_) => return Response::error("Unauthorized", 401),
    };

    let client_id = ctx.param("id").ok_or("Missing client ID")?;

    // Get the user's Durable Object
    let namespace = ctx.env.durable_object("USER_HUB")?;
    let id = namespace.id_from_name(&user.id)?;
    let stub = id.get_stub()?;

    // Fetch clients from DO
    let do_req = Request::new("https://do/clients", Method::Get)?;
    let mut response = stub.fetch_with_request(do_req).await?;

    let clients: Vec<Client> = response.json().await.unwrap_or_default();
    let client = clients.into_iter().find(|c| &c.id == client_id);

    match client {
        Some(c) => {
            // Get the callback URL and convert to WebSocket URL
            if let Some(callback_url) = &c.metadata.callback_url {
                // Convert http(s):// to ws(s)://
                let ws_base = callback_url
                    .replace("https://", "wss://")
                    .replace("http://", "ws://");

                // Check if it's a localhost URL (not publicly accessible)
                let is_local = callback_url.contains("localhost") || callback_url.contains("127.0.0.1");

                let response_json = serde_json::json!({
                    "client_id": c.id,
                    "ws_base": ws_base,
                    "callback_url": callback_url,
                    "is_local": is_local,
                    "status": c.metadata.status.to_string(),
                });

                Response::from_json(&response_json)
            } else {
                Response::from_json(&serde_json::json!({
                    "error": "Client does not have a callback URL configured",
                    "client_id": c.id,
                }))
            }
        }
        None => Response::error("Client not found", 404),
    }
}

/// Disconnect a client (sends disconnect command via WebSocket)
pub async fn disconnect_client(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Check authentication
    let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
        Ok(user) => user,
        Err(redirect) => return Ok(redirect),
    };

    let client_id = ctx.param("id").ok_or("Missing client ID")?;

    // Get the user's Durable Object
    let namespace = ctx.env.durable_object("USER_HUB")?;
    let id = namespace.id_from_name(&user.id)?;
    let stub = id.get_stub()?;

    // Send disconnect request to DO
    let do_req = Request::new(
        &format!("https://do/clients/{}/disconnect", client_id),
        Method::Post,
    )?;
    let response = stub.fetch_with_request(do_req).await?;

    if response.status_code() == 200 {
        // Return updated client list for HTMX swap
        let clients_req = Request::new("https://do/clients", Method::Get)?;
        let mut clients_response = stub.fetch_with_request(clients_req).await?;
        let clients: Vec<Client> = clients_response.json().await.unwrap_or_default();

        Response::from_html(templates::render_client_list(&clients))
    } else {
        Response::error("Failed to disconnect client", 500)
    }
}
