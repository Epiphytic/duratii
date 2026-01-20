# Quick Start Guide

Get Duratii running locally in under 5 minutes.

## Prerequisites

1. **Rust toolchain**

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Wrangler CLI**

   ```bash
   npm install -g wrangler
   wrangler login
   ```

3. **GitHub OAuth App** (for authentication)
   - Go to GitHub Settings → Developer settings → OAuth Apps → New OAuth App
   - Set callback URL to `http://localhost:8787/auth/github/callback`
   - Note the Client ID and Client Secret

## Setup

### 1. Clone and Configure

```bash
git clone https://github.com/Epiphytic/duratii.git
cd duratii
```

### 2. Create D1 Database

```bash
# Create the database
wrangler d1 create orchestrator-db

# Note the database_id in the output, then update wrangler.toml:
# [[d1_databases]]
# database_id = "<your-database-id>"

# Run the schema
wrangler d1 execute orchestrator-db --local --file=./schema.sql
```

### 3. Set Environment Variables

Edit `wrangler.toml`:

```toml
[vars]
GITHUB_CLIENT_ID = "your-github-client-id"
ALLOWED_USERS = "your-github-username"
```

Set the secret:

```bash
wrangler secret put GITHUB_CLIENT_SECRET
# Paste your GitHub OAuth client secret
```

### 4. Run Locally

```bash
wrangler dev
```

Visit http://localhost:8787

## First Steps

1. **Login**: Click "Sign in with GitHub" to authenticate
2. **Create a Token**: In the dashboard, click "New Token" to generate a connection token
3. **Connect claudecodeui**: Configure your claudecodeui instance with the token

## Connecting claudecodeui

In your claudecodeui configuration, add:

```json
{
  "orchestrator": {
    "enabled": true,
    "url": "ws://localhost:8787/ws/connect",
    "token": "ao_xxx_your-token-here"
  }
}
```

Or set environment variables:

```bash
export ORCHESTRATOR_URL="ws://localhost:8787/ws/connect"
export ORCHESTRATOR_TOKEN="ao_xxx_your-token-here"
```

## Deployment

Deploy to Cloudflare Workers:

```bash
# Set production secrets
wrangler secret put GITHUB_CLIENT_SECRET

# Deploy
wrangler deploy
```

Update your GitHub OAuth App callback URL to your production domain:
`https://your-domain.workers.dev/auth/github/callback`

## Troubleshooting

### "Unauthorized" on login

Check that your GitHub username is in `ALLOWED_USERS` in wrangler.toml.

### claudecodeui won't connect

1. Verify the token is correct (starts with `ao_`)
2. Check the WebSocket URL matches your deployment
3. Ensure the token hasn't been revoked

### Database errors

Re-run the schema:

```bash
wrangler d1 execute orchestrator-db --local --file=./schema.sql
```

## Next Steps

- Read [PROJECT.md](./PROJECT.md) for detailed architecture documentation
- Check [CLAUDE.md](../CLAUDE.md) for development guidelines
