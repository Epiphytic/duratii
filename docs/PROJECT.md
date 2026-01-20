# Duratii Project Documentation

## Overview

Duratii is a lightweight, cost-optimized orchestrator for managing multiple Claude Code instances from a unified web interface. Named after _Tillandsia duratii_—an air plant with prehensile leaves that wrap around branches for stability—the project provides persistent connections that wrap around your compute instances.

### What It Does

- **Aggregates Claude Code instances**: Connects to multiple [claudecodeui](https://github.com/siteboon/claudecodeui) instances running anywhere
- **Real-time monitoring**: WebSocket-based live status updates
- **Mobile-first dashboard**: HTMX-powered UI for managing sessions on the go
- **Secure authentication**: GitHub OAuth with org/team/user restrictions

### Core Concept

Duratii acts as the "nervous system" between your mobile device and distributed development environments. It doesn't try to be an IDE—it relies on claudecodeui for the heavy lifting and code execution. Duratii is the lightweight orchestration layer that floats above it.

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

---

## Technology Stack

### Backend: Rust on Cloudflare Workers

The backend is written in Rust and compiled to WebAssembly (WASM) for execution on Cloudflare Workers. This provides:

- **Low latency**: Edge deployment close to users
- **Cost efficiency**: Pay only for compute time used
- **Type safety**: Rust's compiler catches bugs at build time

Key dependencies:

- **worker-rs (v0.7)**: Official Cloudflare Workers Rust bindings
- **serde/serde_json**: JSON serialization
- **wasm-bindgen**: WebAssembly interop
- **getrandom**: Cryptographic random number generation

### Frontend: HTMX

The UI uses HTMX for hypermedia-driven interactions—no React, Vue, or heavy JavaScript frameworks. Benefits:

- **Minimal JavaScript**: Most logic stays on the server
- **Fast updates**: HTML fragments swapped directly into the DOM
- **Progressive enhancement**: Works without JavaScript for basic functionality

### Cloudflare Services

| Service         | Purpose                                                 |
| --------------- | ------------------------------------------------------- |
| Workers         | HTTP routing, HTMX responses, OAuth flow                |
| Durable Objects | Per-user WebSocket hub, client registry, real-time sync |
| D1              | User accounts, sessions, tokens, access control         |
| R2              | Static assets (CSS, JS)                                 |

---

## How It Uses claudecodeui

[siteboon/claudecodeui](https://github.com/siteboon/claudecodeui) is the actual Claude Code interface—the tool that interacts with Claude and executes code. Duratii connects to claudecodeui instances as a management layer.

### Connection Flow

1. **Token Generation**: User creates a connection token in the Duratii dashboard
2. **claudecodeui Configuration**: The token is configured in claudecodeui settings
3. **WebSocket Connection**: claudecodeui connects to `wss://duratii.example.com/ws/connect?token=ao_<id>_<secret>`
4. **Registration**: claudecodeui sends its metadata (hostname, project path, status)
5. **Real-time Updates**: Status changes (idle/active/busy) are streamed to the dashboard

### Message Protocol

**claudecodeui → Duratii:**

```json
{
  "type": "register",
  "client_id": "unique-client-id",
  "metadata": {
    "hostname": "dev-machine",
    "project": "/path/to/project",
    "status": "idle"
  }
}
```

```json
{
  "type": "status_update",
  "client_id": "unique-client-id",
  "status": "busy"
}
```

**Duratii → Browser:**

```json
{
  "type": "client_update",
  "client": {
    "id": "unique-client-id",
    "hostname": "dev-machine",
    "status": "busy"
  }
}
```

### HTTP Proxy

Duratii can proxy HTTP requests to claudecodeui instances, enabling browser access to the claudecodeui web interface through Duratii:

```
Browser → /clients/{id}/proxy/* → Duratii → claudecodeui callback URL
```

This works with ngrok tunnels, local networks, or any URL the claudecodeui instance provides.

---

## Project Structure

```
duratii/
├── Cargo.toml              # Rust dependencies
├── wrangler.toml           # Cloudflare Workers configuration
├── schema.sql              # D1 database schema
└── src/
    ├── lib.rs              # Worker entry point, route definitions
    ├── auth/
    │   ├── mod.rs          # GitHub OAuth flow
    │   └── middleware.rs   # Session validation
    ├── handlers/
    │   ├── mod.rs          # Static assets, health check
    │   ├── dashboard.rs    # Main dashboard page
    │   ├── clients.rs      # Client list endpoints
    │   ├── websocket.rs    # WebSocket upgrade handler
    │   ├── tokens.rs       # Token CRUD operations
    │   ├── proxy.rs        # HTTP proxy to claudecodeui
    │   └── cloudflare.rs   # Cache purge API
    ├── durable_objects/
    │   ├── mod.rs          # Exports
    │   └── user_hub.rs     # Per-user orchestration hub
    ├── models/
    │   ├── mod.rs          # Exports
    │   ├── user.rs         # User, Session models
    │   ├── client.rs       # Client, ClientMetadata models
    │   └── token.rs        # Token generation/validation
    └── templates/
        └── mod.rs          # Inline HTMX templates
```

---

## Important Files

### `src/lib.rs` — Entry Point

The main worker entry point. Defines all HTTP routes:

```rust
Router::new()
    .get("/", home)
    .get("/health", health)
    .get("/auth/github", start_oauth)
    .get("/auth/github/callback", handle_callback)
    .get("/dashboard", dashboard)
    .get("/clients", get_clients)
    .get("/ws/connect", websocket_upgrade)
    // ... 60+ routes
```

### `src/auth/mod.rs` — GitHub OAuth

Implements the complete OAuth flow:

1. **start_oauth()**: Generates CSRF state, redirects to GitHub
2. **handle_callback()**: Exchanges code for token, validates org membership, creates session
3. **logout()**: Clears session

Key security features:

- CSRF state validation
- Org/team/user whitelist checks
- Secure, HttpOnly session cookies

### `src/auth/middleware.rs` — Session Validation

Validates session cookies against D1:

```rust
pub async fn get_user(req: &Request, env: &Env) -> Result<Option<User>> {
    // Parse session cookie
    // Look up session in D1
    // Check expiration
    // Return user or None
}
```

### `src/durable_objects/user_hub.rs` — The Core

The largest file (~1,000 lines). Each user gets one UserHub Durable Object that:

- **Manages WebSocket connections**: Both browser and claudecodeui connections
- **Stores client registry**: In-memory HashMap backed by SQLite
- **Broadcasts updates**: Notifies all browsers when client status changes
- **Proxies requests**: Forwards HTTP requests to claudecodeui

Key data structures:

```rust
struct UserHub {
    clients: HashMap<String, Client>,           // Connected claudecodeui instances
    browser_connections: Vec<WebSocket>,        // Active browser sessions
    client_connections: HashMap<String, WebSocket>, // claudecodeui WebSocket handles
}
```

The Durable Object uses Cloudflare's hibernation API—it sleeps when idle and wakes on incoming messages, minimizing costs.

### `src/handlers/tokens.rs` — Token Management

CRUD operations for connection tokens:

- **create_token_api()**: Generates secure token, stores hash in D1
- **list_tokens()**: Returns user's tokens (without secrets)
- **revoke_token_htmx()**: Marks token as revoked

Token format: `ao_<id>_<32-char-secret>`

### `src/handlers/proxy.rs` — HTTP Proxy

Proxies requests to claudecodeui instances:

```rust
// Request: GET /clients/abc123/proxy/index.html
// Proxied to: https://ngrok-url.io/index.html
```

Supports two modes:

1. **Direct callback URL**: Forward to claudecodeui's configured URL
2. **WebSocket bridge**: Route through the Durable Object (fallback)

### `src/templates/mod.rs` — UI Templates

All HTML templates inline as Rust strings (~1,700 lines):

- **render_home()**: Login page with GitHub button
- **render_dashboard()**: Main UI with WebSocket connection script
- **render_client_list()**: Grid of client cards (HTMX partial)
- **render_client_card()**: Individual client card
- **render_tokens_list()**: Token management UI

Includes embedded JavaScript for:

- WebSocket connection with auto-reconnect
- Client list management
- UI updates from WebSocket messages

### `src/models/client.rs` — Client Model

```rust
pub enum ClientStatus {
    Idle,
    Active,
    Busy,
    Disconnected,
}

pub struct Client {
    pub id: String,
    pub user_id: String,
    pub metadata: ClientMetadata,
    pub connected_at: String,
}

pub struct ClientMetadata {
    pub hostname: String,
    pub project: String,
    pub status: ClientStatus,
    pub callback_url: Option<String>,
}
```

### `schema.sql` — Database Schema

D1 tables:

```sql
-- User accounts (from GitHub OAuth)
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    github_id INTEGER UNIQUE NOT NULL,
    github_login TEXT NOT NULL,
    email TEXT,
    created_at DATETIME,
    last_login DATETIME
);

-- Session tokens
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    expires_at DATETIME NOT NULL
);

-- Connection tokens for claudecodeui
CREATE TABLE client_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    created_at DATETIME,
    revoked_at DATETIME
);

-- Access control list
CREATE TABLE allowed_entities (
    entity_type TEXT NOT NULL, -- 'org', 'user', 'team'
    entity_id TEXT NOT NULL,
    entity_name TEXT NOT NULL
);
```

### `wrangler.toml` — Cloudflare Configuration

```toml
name = "duratii"
main = "build/worker/shim.mjs"

[build]
command = "cargo install -q worker-build && worker-build --release"

[[durable_objects.bindings]]
name = "USER_HUB"
class_name = "UserHub"

[[d1_databases]]
binding = "DB"
database_name = "orchestrator-db"

[[r2_buckets]]
binding = "ASSETS"
bucket_name = "orchestrator-assets"

[vars]
GITHUB_CLIENT_ID = ""
ALLOWED_USERS = "liamhelmer"
```

---

## Data Flow

### Authentication Flow

```
1. User visits /dashboard
2. No session cookie → redirect to /auth/github
3. GitHub OAuth → user authorizes
4. Callback with code → exchange for access token
5. Fetch user profile, check org membership
6. Create session in D1, set cookie
7. Redirect to /dashboard
```

### Client Connection Flow

```
1. User creates token in dashboard
2. Token stored as hash in D1
3. claudecodeui connects: /ws/connect?token=ao_xxx_yyy
4. Token validated against D1
5. Connection routed to user's Durable Object
6. Client registered in DO's SQLite + in-memory map
7. Browser connections notified via WebSocket
```

### Real-time Updates

```
1. claudecodeui status changes (idle → busy)
2. Sends StatusUpdate message to Durable Object
3. DO updates SQLite + in-memory map
4. DO broadcasts ClientUpdate to all browser connections
5. Browser receives message, updates UI via HTMX swap
```

---

## Development

### Prerequisites

- Rust toolchain (rustup)
- Wrangler CLI (`npm install -g wrangler`)
- Cloudflare account with Workers enabled

### Local Development

```bash
# Install dependencies and run locally
wrangler dev

# The worker will be available at http://localhost:8787
```

### Database Setup

```bash
# Create D1 database
wrangler d1 create orchestrator-db

# Run migrations
wrangler d1 execute orchestrator-db --file=./schema.sql
```

### Deployment

```bash
# Set secrets
wrangler secret put GITHUB_CLIENT_SECRET

# Deploy to production
wrangler deploy
```

### Testing

```bash
# Run Rust tests
cargo test

# Deploy to staging
wrangler deploy --env staging
```

---

## Security

### Authentication

- GitHub OAuth with CSRF state validation
- Session tokens stored in D1 with expiration
- Secure, HttpOnly, SameSite cookies

### Authorization

- Org/team/user whitelist in environment variables
- Per-request session validation
- Token-based authentication for claudecodeui connections

### Data Protection

- Tokens stored as hashes (raw token shown once at creation)
- HTML escaping in templates (XSS prevention)
- Input validation on all endpoints

---

## Performance

### Cost Optimization

- **Durable Object hibernation**: Sleeps after 10s idle
- **SQLite in DO**: Fast local storage, no D1 round-trips for hot data
- **Minimal JavaScript**: HTMX keeps logic server-side
- **Edge deployment**: Low latency, no origin server

### Targets

| Metric                 | Target             |
| ---------------------- | ------------------ |
| First contentful paint | < 500ms            |
| HTMX partial update    | < 100ms            |
| WebSocket latency      | < 50ms             |
| Worker CPU time        | < 50ms per request |
