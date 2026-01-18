# AI Orchestrator UI

A lightweight, cost-optimized orchestrator for managing multiple Claude Code instances from a unified web interface.

## Project Overview

This application provides a clean HTMX-based UI that allows authenticated users to connect to and manage all their Claude Code instances running anywhere. It aggregates instances of [claudecodeui](https://github.com/siteboon/claudecodeui) which are modified to connect back to this orchestrator.

### Core Requirements

| Requirement        | Implementation                                        |
| ------------------ | ----------------------------------------------------- |
| UI Framework       | HTMX for hypermedia-driven interactions               |
| Backend            | Rust compiled to WebAssembly for Cloudflare Workers   |
| Client Aggregation | Modified claudecodeui instances connect via WebSocket |
| Authentication     | GitHub App with org/user/team restrictions            |
| Platform           | Cloudflare Workers, D1, R2                            |
| Cost Optimization  | Durable Objects, minimal CPU/memory, sleep when idle  |

## Architecture

```
┌─────────────────┐     ┌──────────────────────────────────────────────┐
│  claudecodeui   │────▶│           Cloudflare Edge                    │
│   instance 1    │     │  ┌─────────────────────────────────────────┐ │
└─────────────────┘     │  │         Rust/WASM Worker                │ │
                        │  │  ┌─────────────┐  ┌──────────────────┐  │ │
┌─────────────────┐     │  │  │ HTMX Routes │  │ GitHub OAuth     │  │ │
│  claudecodeui   │────▶│  │  └─────────────┘  └──────────────────┘  │ │
│   instance 2    │     │  └─────────────────────────────────────────┘ │
└─────────────────┘     │                       │                      │
                        │  ┌────────────────────▼────────────────────┐ │
┌─────────────────┐     │  │        Durable Object (per user)       │ │
│  claudecodeui   │────▶│  │  ┌───────────┐  ┌───────────────────┐  │ │
│   instance N    │     │  │  │ WebSocket │  │ Client Registry   │  │ │
└─────────────────┘     │  │  │   Hub     │  │ (SQLite storage)  │  │ │
                        │  │  └───────────┘  └───────────────────┘  │ │
     ┌──────────┐       │  └─────────────────────────────────────────┘ │
     │ Browser  │◀─────▶│                       │                      │
     │ (HTMX)   │       │  ┌────────────────────▼────────────────────┐ │
     └──────────┘       │  │  D1 (users, sessions)  R2 (static assets)│ │
                        │  └─────────────────────────────────────────┘ │
                        └──────────────────────────────────────────────┘
```

## Technology Stack

### Backend (Rust on Cloudflare Workers)

- **worker-rs**: Rust bindings for Cloudflare Workers
- **worker-macros**: Procedural macros for route handling
- **serde**: JSON serialization
- **wasm-bindgen**: WebAssembly interop

### Frontend

- **HTMX**: Hypermedia-driven UI updates
- **Minimal CSS**: Lightweight styling (no heavy frameworks)
- **No build step**: Direct HTML templates

### Cloudflare Services

| Service         | Purpose                                                 |
| --------------- | ------------------------------------------------------- |
| Workers         | HTTP routing, HTMX responses, OAuth flow                |
| Durable Objects | Per-user WebSocket hub, client registry, real-time sync |
| D1              | User accounts, sessions, GitHub team/org mappings       |
| R2              | Static assets (CSS, JS), optional log storage           |
| KV              | Session tokens, rate limiting (optional)                |

## Development Guidelines

### Rust/WASM Best Practices

```rust
// Use worker-rs for Cloudflare Workers
use worker::*;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get("/", |_, _| Response::ok("Hello"))
        .get_async("/clients", handle_clients)
        .run(req, env)
        .await
}
```

### HTMX Response Patterns

Return HTML fragments, not JSON:

```rust
// Good: Return HTML fragment for HTMX swap
async fn get_client_list(env: &Env, user_id: &str) -> Result<Response> {
    let clients = fetch_clients(env, user_id).await?;
    let html = render_client_list(&clients);
    Response::from_html(html)
}

// HTMX attributes in templates
// hx-get="/clients" hx-trigger="every 5s" hx-swap="innerHTML"
```

### Durable Object Pattern (Per-User Hub)

```rust
// One DO per authenticated user - handles all their connected clients
#[durable_object]
pub struct UserHub {
    state: State,
    env: Env,
    clients: HashMap<String, ClientConnection>,
}

impl UserHub {
    // WebSocket connections from claudecodeui instances
    async fn websocket_message(&mut self, ws: WebSocket, msg: String) {
        // Route messages, update client state, broadcast to browser
    }
}
```

### Cost Optimization Rules

1. **Sleep when idle**: Durable Objects hibernate after 10 seconds of inactivity
2. **Batch D1 queries**: Combine reads/writes where possible
3. **Minimize CPU time**: Keep request handlers under 50ms
4. **Use SQLite in DOs**: Prefer DO's built-in SQLite over D1 for hot data
5. **Lazy load clients**: Only fetch client details when user expands a card
6. **Cache static assets**: R2 with long cache headers for CSS/JS

### GitHub App Authentication

```rust
// OAuth flow endpoints
Router::new()
    .get("/auth/github", start_oauth)           // Redirect to GitHub
    .get("/auth/github/callback", handle_callback)  // Exchange code for token
    .get("/auth/logout", logout)
```

Required GitHub App permissions:

- `read:org` - Check organization membership
- `read:user` - Get user profile
- `user:email` - Get email for account linking

Team/org restriction logic:

```rust
async fn authorize_user(token: &str, allowed_orgs: &[String]) -> Result<bool> {
    let orgs = github_api::get_user_orgs(token).await?;
    Ok(orgs.iter().any(|o| allowed_orgs.contains(&o.login)))
}
```

### Client Connection Protocol

claudecodeui instances connect with:

```javascript
// Modified claudecodeui connection
const ws = new WebSocket("wss://orchestrator.example.com/ws/connect");
ws.send(
  JSON.stringify({
    type: "register",
    client_id: "unique-client-id",
    user_token: "user-auth-token",
    metadata: {
      hostname: "dev-machine",
      project: "/path/to/project",
      status: "idle",
    },
  }),
);
```

### File Structure

```
src/
├── lib.rs              # Worker entry point, router
├── auth/
│   ├── mod.rs          # GitHub OAuth flow
│   └── middleware.rs   # Auth middleware
├── handlers/
│   ├── mod.rs
│   ├── clients.rs      # Client list HTMX endpoints
│   ├── dashboard.rs    # Dashboard page
│   └── websocket.rs    # WebSocket upgrade handler
├── durable_objects/
│   ├── mod.rs
│   └── user_hub.rs     # Per-user client hub
├── templates/
│   ├── layout.html     # Base layout
│   ├── dashboard.html  # Main dashboard
│   ├── clients.html    # Client list partial
│   └── client_card.html # Single client card
└── models/
    ├── mod.rs
    ├── user.rs         # User model
    └── client.rs       # Connected client model
```

### wrangler.jsonc Configuration

```jsonc
{
  "$schema": "./node_modules/wrangler/config-schema.json",
  "name": "ai-orchestrator",
  "main": "build/worker/shim.mjs",
  "compatibility_date": "2026-01-01",

  "build": {
    "command": "cargo install -q worker-build && worker-build --release",
  },

  "durable_objects": {
    "bindings": [{ "name": "USER_HUB", "class_name": "UserHub" }],
  },

  "migrations": [{ "tag": "v1", "new_sqlite_classes": ["UserHub"] }],

  "d1_databases": [
    {
      "binding": "DB",
      "database_name": "orchestrator-db",
      "database_id": "<ID>",
    },
  ],

  "r2_buckets": [{ "binding": "ASSETS", "bucket_name": "orchestrator-assets" }],

  "vars": {
    "GITHUB_CLIENT_ID": "",
    "ALLOWED_ORGS": "",
    "ALLOWED_USERS": "",
    "ALLOWED_TEAMS": "",
  },
}
```

### D1 Schema

```sql
-- users table
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    github_id INTEGER UNIQUE NOT NULL,
    github_login TEXT NOT NULL,
    email TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_login DATETIME
);

-- sessions table
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    expires_at DATETIME NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- allowed_entities (orgs, users, teams)
CREATE TABLE allowed_entities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_type TEXT NOT NULL CHECK (entity_type IN ('org', 'user', 'team')),
    entity_id TEXT NOT NULL,
    entity_name TEXT NOT NULL,
    UNIQUE(entity_type, entity_id)
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
CREATE INDEX idx_allowed_type ON allowed_entities(entity_type);
```

## Anti-Patterns (NEVER)

- **No heavy frameworks**: Don't use React, Vue, or large CSS frameworks
- **No long-running requests**: Keep all handlers under 30 seconds (Workers limit)
- **No global state in Worker**: All state belongs in Durable Objects or D1
- **No polling from browser**: Use WebSocket for real-time updates, HTMX triggers for periodic checks
- **No secrets in code**: Use wrangler secrets for `GITHUB_CLIENT_SECRET`
- **No unnecessary wakeups**: Don't ping DOs to keep them alive

## Testing

```bash
# Local development
wrangler dev

# Run Rust tests
cargo test

# Deploy to staging
wrangler deploy --env staging

# Deploy to production
wrangler deploy
```

## Security Considerations

1. **Validate all WebSocket messages**: Never trust client-provided data
2. **Rate limit connections**: Use KV to track connection attempts per IP
3. **Rotate session tokens**: Short-lived sessions with refresh tokens
4. **Verify GitHub tokens**: Re-validate org/team membership periodically
5. **Sanitize HTML output**: Escape all user-provided data in templates

## Performance Targets

| Metric                 | Target             |
| ---------------------- | ------------------ |
| First contentful paint | < 500ms            |
| HTMX partial update    | < 100ms            |
| WebSocket latency      | < 50ms             |
| Worker CPU time        | < 50ms per request |
| DO hibernation         | Within 10s of idle |

## Deployment Checklist

- [ ] Set `GITHUB_CLIENT_SECRET` via `wrangler secret put`
- [ ] Configure allowed orgs/users/teams in D1
- [ ] Create D1 database and run migrations
- [ ] Create R2 bucket for static assets
- [ ] Set up custom domain with SSL
- [ ] Configure GitHub App callback URL
- [ ] Test OAuth flow end-to-end
- [ ] Verify WebSocket connections from claudecodeui
