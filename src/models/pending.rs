use serde::{Deserialize, Serialize};

/// Info about a pending client (for API responses and templates)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingClientInfo {
    pub pending_id: String,
    pub hostname: String,
    pub project: String,
    pub platform: String,
    pub ip_address: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub connected_at: i64,
    pub allowed_users: Vec<String>,
    pub allowed_orgs: Vec<String>,
    pub allowed_teams: Vec<String>,
}
