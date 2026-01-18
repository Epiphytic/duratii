mod clients;
mod dashboard;
mod websocket;

pub use clients::{get_client, get_clients};
pub use dashboard::dashboard;
pub use websocket::websocket_upgrade;

use worker::*;

use crate::templates;

/// Home page - login screen
pub fn home(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    Response::from_html(templates::render_home())
}

/// Health check endpoint
pub fn health(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    Response::ok("OK")
}

/// Serve static assets from R2
pub async fn serve_static(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let path = ctx.param("path").unwrap_or(&"".to_string()).clone();

    let bucket = ctx.env.bucket("ASSETS")?;
    let object = bucket.get(&path).execute().await?;

    match object {
        Some(obj) => {
            let body = obj.body().ok_or("No body")?;
            let bytes = body.bytes().await?;

            let content_type = guess_content_type(&path);
            let mut headers = Headers::new();
            headers.set("Content-Type", content_type)?;
            headers.set("Cache-Control", "public, max-age=31536000")?;

            Ok(Response::from_bytes(bytes)?.with_headers(headers))
        }
        None => Response::error("Not found", 404),
    }
}

fn guess_content_type(path: &str) -> &'static str {
    if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}
