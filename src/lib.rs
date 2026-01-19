use worker::*;

mod auth;
mod durable_objects;
mod handlers;
mod models;
mod templates;

pub use durable_objects::UserHub;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    Router::new()
        // Public routes
        .get("/", handlers::home)
        .get("/health", handlers::health)
        // Auth routes
        .get_async("/auth/github", auth::start_oauth)
        .get_async("/auth/github/callback", auth::handle_callback)
        .get_async("/auth/logout", auth::logout)
        // Protected routes (dashboard)
        .get_async("/dashboard", handlers::dashboard)
        .get_async("/clients", handlers::get_clients)
        .get_async("/clients/:id", handlers::get_client)
        .get_async("/clients/:id/details", handlers::get_client_details)
        .post_async("/clients/:id/disconnect", handlers::disconnect_client)
        // Token management API
        .get_async("/api/tokens", handlers::list_tokens)
        .post_async("/api/tokens", handlers::create_token)
        .post_async("/api/tokens/:id/revoke", handlers::revoke_token)
        .delete_async("/api/tokens/:id", handlers::delete_token)
        // WebSocket upgrade for claudecodeui connections
        .get_async("/ws/connect", handlers::websocket_upgrade)
        // Static assets
        .get_async("/static/*path", handlers::serve_static)
        .run(req, env)
        .await
}
