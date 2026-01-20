# File Reference

Quick lookup for finding code by functionality.

## Configuration Files

| File            | Purpose                                                   |
| --------------- | --------------------------------------------------------- |
| `Cargo.toml`    | Rust dependencies (worker-rs, serde, wasm-bindgen)        |
| `wrangler.toml` | Cloudflare Workers config (DO bindings, D1, R2, env vars) |
| `schema.sql`    | D1 database schema (users, sessions, tokens, clients)     |

## Source Files

### Entry Point

| File         | Purpose                                                |
| ------------ | ------------------------------------------------------ |
| `src/lib.rs` | Worker entry point, all route definitions (~60 routes) |

### Authentication (`src/auth/`)

| File            | Purpose                                                     |
| --------------- | ----------------------------------------------------------- |
| `mod.rs`        | GitHub OAuth flow (start, callback, logout), access control |
| `middleware.rs` | Session cookie validation, user lookup from D1              |

### Request Handlers (`src/handlers/`)

| File            | Purpose                                          |
| --------------- | ------------------------------------------------ |
| `mod.rs`        | Health check, home page, R2 static asset serving |
| `dashboard.rs`  | Main dashboard page (requires auth)              |
| `clients.rs`    | Client list/detail endpoints (HTMX partials)     |
| `websocket.rs`  | WebSocket upgrade, routes to UserHub DO          |
| `tokens.rs`     | Token CRUD (create, list, revoke)                |
| `proxy.rs`      | HTTP proxy to claudecodeui callback URLs         |
| `cloudflare.rs` | Cache purge API integration                      |

### Durable Objects (`src/durable_objects/`)

| File          | Purpose                                                                                      |
| ------------- | -------------------------------------------------------------------------------------------- |
| `mod.rs`      | Exports UserHub                                                                              |
| `user_hub.rs` | **Core component**: Per-user WebSocket hub, client registry, SQLite storage, message routing |

### Data Models (`src/models/`)

| File        | Purpose                               |
| ----------- | ------------------------------------- |
| `mod.rs`    | Exports all models                    |
| `user.rs`   | User and Session structs              |
| `client.rs` | Client, ClientMetadata, ClientStatus  |
| `token.rs`  | Token generation, hashing, validation |

### Templates (`src/templates/`)

| File     | Purpose                                                             |
| -------- | ------------------------------------------------------------------- |
| `mod.rs` | All HTML templates inline: login, dashboard, client cards, token UI |

## By Feature

### "How do I add a new route?"

→ `src/lib.rs` - Add to the Router chain

### "How does authentication work?"

→ `src/auth/mod.rs` - OAuth flow
→ `src/auth/middleware.rs` - Session validation

### "How do WebSocket messages work?"

→ `src/durable_objects/user_hub.rs` - Message handling in `handle_message()`

### "How do claudecodeui instances connect?"

→ `src/handlers/websocket.rs` - Token validation and DO routing
→ `src/durable_objects/user_hub.rs` - Registration in `register_client()`

### "How do I modify the UI?"

→ `src/templates/mod.rs` - All templates are inline Rust strings

### "How does the HTTP proxy work?"

→ `src/handlers/proxy.rs` - Forwards requests to callback URLs

### "Where is client data stored?"

→ D1: `schema.sql` (persistent, queryable)
→ DO SQLite: `user_hub.rs` (per-user, fast access)

### "How are tokens validated?"

→ `src/models/token.rs` - Hash/verify functions
→ `src/handlers/websocket.rs` - Token lookup on connect

## Code Statistics

| Component          | Lines  | Notes                            |
| ------------------ | ------ | -------------------------------- |
| `user_hub.rs`      | ~1,076 | Largest file, core orchestration |
| `templates/mod.rs` | ~1,720 | All UI templates + embedded JS   |
| `auth/mod.rs`      | ~373   | OAuth + access control           |
| `tokens.rs`        | ~371   | Token CRUD operations            |
| `proxy.rs`         | ~245   | HTTP forwarding                  |
| Total              | ~4,500 | Excluding generated code         |
