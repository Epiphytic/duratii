<p align="center">
  <img src="logoes/Duratii-Header.png" alt="Duratii - The Tethered Orchestrator" width="100%">
</p>

<p align="center">
  <strong>A persistent, real-time companion that wraps around your AI engine.</strong>
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> •
  <a href="#the-anatomy">The Anatomy</a> •
  <a href="#tech-stack">Tech Stack</a> •
  <a href="#documentation">Docs</a>
</p>

---

Duratii is the mobile-first orchestration layer for your development ecosystem. Named after _Tillandsia duratii_—an epiphytic plant famous for its prehensile leaves that curl around branches—this tool wraps around your [claudecodeui](https://github.com/siteboon/claudecodeui) instances, providing a persistent, bi-directional interface that travels with you.

## Quick Start

```bash
# Install dependencies
npm install -g wrangler

# Set up database
wrangler d1 execute orchestrator-db --local --file=./schema.sql

# Upload static assets to R2
wrangler r2 object put orchestrator-assets/emblem.png --file=./assets/emblem.png
wrangler r2 object put orchestrator-assets/favicon.png --file=./assets/favicon.png

# Run locally
wrangler dev
```

See [docs/QUICKSTART.md](docs/QUICKSTART.md) for full setup instructions.

## The Anatomy

**🔗 Persistent Tendrils (WebSockets)** — Forget stateless requests. Duratii maintains active, bi-directional WebSocket connections to your claudecodeui instances. It creates a "live wire" between your mobile device and your dev environment, streaming output and capturing input in real-time.

**🏗️ Epiphytic Structure** — It does not try to be the IDE. It relies on [siteboon/claudecodeui](https://github.com/siteboon/claudecodeui) for the heavy lifting and code execution. Duratii is simply the lightweight, adaptive layer that floats above it, translating that raw power into a mobile-friendly orchestration experience.

**📱 The Companion View** — Designed for the developer who is moving. Whether you are walking the dog or in transit, Duratii keeps the context of your AI sessions alive, allowing you to guide the generation process without being tethered to a desktop.

## Tech Stack

| Layer     | Technology                               |
| --------- | ---------------------------------------- |
| Backend   | Rust → WebAssembly on Cloudflare Workers |
| Frontend  | HTMX (no heavy JS frameworks)            |
| Database  | Cloudflare D1 (SQLite)                   |
| Real-time | Durable Objects + WebSockets             |
| Auth      | GitHub OAuth                             |

## Documentation

- **[docs/PROJECT.md](docs/PROJECT.md)** — Full architecture, how claudecodeui integration works, detailed file explanations
- **[docs/QUICKSTART.md](docs/QUICKSTART.md)** — Get running locally in 5 minutes
- **[docs/FILE_REFERENCE.md](docs/FILE_REFERENCE.md)** — Quick lookup: find code by feature
- **[CLAUDE.md](CLAUDE.md)** — Development guidelines and work plan

## Dependencies

- [siteboon/claudecodeui](https://github.com/siteboon/claudecodeui) — The Claude Code interface that Duratii orchestrates
- Protocol: `wss://`
