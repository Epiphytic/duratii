-- AI Orchestrator D1 Database Schema
-- Run with: wrangler d1 execute orchestrator-db --file=./schema.sql

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    github_id INTEGER UNIQUE NOT NULL,
    github_login TEXT NOT NULL,
    email TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_login DATETIME
);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    expires_at DATETIME NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Allowed entities (orgs, users, teams) for access control
CREATE TABLE IF NOT EXISTS allowed_entities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_type TEXT NOT NULL CHECK (entity_type IN ('org', 'user', 'team')),
    entity_id TEXT NOT NULL,
    entity_name TEXT NOT NULL,
    UNIQUE(entity_type, entity_id)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_allowed_type ON allowed_entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_users_github_id ON users(github_id);
