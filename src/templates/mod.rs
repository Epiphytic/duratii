use crate::models::{Client, ClientStatus, TokenInfo, User};

/// Render the home/login page
pub fn render_home() -> String {
    layout(
        "AI Orchestrator",
        r#"
        <div class="login-container">
            <h1>AI Orchestrator</h1>
            <p>Manage your Claude Code instances from a unified interface.</p>
            <a href="/auth/github" class="btn btn-primary">
                <svg class="icon" viewBox="0 0 16 16" fill="currentColor">
                    <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/>
                </svg>
                Sign in with GitHub
            </a>
        </div>
        "#,
    )
}

/// Render the main dashboard
pub fn render_dashboard(user: &User) -> String {
    let username = escape_html(&user.github_login);

    let content = [
        "<header class=\"dashboard-header\">",
        "<h1>AI Orchestrator</h1>",
        "<div class=\"user-info\">",
        "<span>", &username, "</span>",
        "<a href=\"/auth/logout\" class=\"btn btn-secondary\">Logout</a>",
        "</div></header>",
        "<main class=\"dashboard-main\">",
        "<section class=\"clients-section\">",
        "<div class=\"section-header\">",
        "<h2>Connected Clients</h2>",
        "<span id=\"client-count-badge\" class=\"count-badge\">0</span>",
        "</div>",
        "<div id=\"clients-list\" hx-get=\"/clients\" hx-trigger=\"load, every 30s\" hx-swap=\"innerHTML\">",
        "<div class=\"loading\">Loading clients...</div>",
        "</div></section>",
        "<section class=\"tokens-section\">",
        "<div class=\"section-header\">",
        "<h2>Connection Tokens</h2>",
        "<button class=\"btn btn-primary btn-sm\" hx-get=\"/tokens/new\" hx-target=\"#token-modal\" hx-swap=\"innerHTML\">+ New Token</button>",
        "</div>",
        "<p class=\"section-desc\">Generate tokens for your Claude Code instances to connect.</p>",
        "<div id=\"tokens-list\" hx-get=\"/tokens\" hx-trigger=\"load\" hx-swap=\"innerHTML\">",
        "<div class=\"loading\">Loading tokens...</div>",
        "</div></section></main>",
        "<div id=\"token-modal\"></div>",
        DASHBOARD_SCRIPT,
    ].concat();

    layout("Dashboard - AI Orchestrator", &content)
}

const DASHBOARD_SCRIPT: &str = r#"<script>
let ws;
let reconnectAttempts = 0;
const maxReconnectAttempts = 5;

function connectWebSocket() {
    ws = new WebSocket(
        (location.protocol === 'https:' ? 'wss:' : 'ws:') +
        '//' + location.host + '/ws/connect?type=browser'
    );

    ws.onopen = () => {
        console.log('WebSocket connected');
        reconnectAttempts = 0;
        ws.send(JSON.stringify({ type: 'get_clients' }));
    };

    ws.onmessage = (event) => {
        const msg = JSON.parse(event.data);

        if (msg.type === 'client_update') {
            const clientId = msg.client.id;
            const clientCard = document.getElementById('client-' + clientId);
            if (clientCard) {
                htmx.trigger(clientCard, 'refresh');
            } else {
                htmx.trigger('#clients-list', 'load');
            }
            updateClientCount();
        } else if (msg.type === 'client_disconnected') {
            const clientCard = document.getElementById('client-' + msg.client_id);
            if (clientCard) {
                clientCard.style.opacity = '0.5';
                clientCard.style.transition = 'opacity 0.3s';
                setTimeout(() => {
                    htmx.trigger('#clients-list', 'load');
                }, 300);
            }
            updateClientCount();
        } else if (msg.type === 'client_list') {
            updateClientCount(msg.clients);
            // Refresh the client list to show the cards
            if (msg.clients && msg.clients.length > 0) {
                htmx.trigger('#clients-list', 'load');
            }
        }
    };

    ws.onclose = () => {
        console.log('WebSocket closed');
        if (reconnectAttempts < maxReconnectAttempts) {
            const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 30000);
            reconnectAttempts++;
            console.log('Reconnecting in ' + delay + 'ms...');
            setTimeout(connectWebSocket, delay);
        }
    };

    ws.onerror = (error) => {
        console.error('WebSocket error:', error);
    };
}

function updateClientCount(clients) {
    const badge = document.getElementById('client-count-badge');
    if (badge && clients) {
        const active = clients.filter(c => c.metadata.status === 'active' || c.metadata.status === 'busy').length;
        const total = clients.length;
        badge.textContent = active > 0 ? active + '/' + total : total;
        badge.className = 'count-badge' + (active > 0 ? ' has-active' : '');
    }
}

connectWebSocket();
</script>"#;

/// Render the clients page (full page, used for non-HTMX requests)
pub fn render_clients_page(user: &User, clients: &[Client]) -> String {
    layout(
        "Clients - AI Orchestrator",
        &format!(
            r#"
            <header class="dashboard-header">
                <h1>AI Orchestrator</h1>
                <div class="user-info">
                    <span>{}</span>
                    <a href="/auth/logout" class="btn btn-secondary">Logout</a>
                </div>
            </header>
            <main class="dashboard-main">
                <section class="clients-section">
                    <h2>Connected Clients</h2>
                    <div id="clients-list">
                        {}
                    </div>
                </section>
            </main>
            "#,
            escape_html(&user.github_login),
            render_client_list(clients)
        ),
    )
}

/// Render the client list (HTMX partial)
pub fn render_client_list(clients: &[Client]) -> String {
    if clients.is_empty() {
        return r#"
            <div class="empty-state">
                <p>No clients connected.</p>
                <p class="hint">Connect a Claude Code instance to get started.</p>
            </div>
        "#
        .to_string();
    }

    let cards: Vec<String> = clients.iter().map(render_client_card).collect();
    format!(r#"<div class="clients-grid">{}</div>"#, cards.join("\n"))
}

/// Render a single client card (collapsed view)
pub fn render_client_card(client: &Client) -> String {
    let status_class = match client.metadata.status {
        ClientStatus::Idle => "status-idle",
        ClientStatus::Active => "status-active",
        ClientStatus::Busy => "status-busy",
        ClientStatus::Disconnected => "status-disconnected",
    };

    // Truncate project path for collapsed view
    let short_project = truncate_path(&client.metadata.project, 40);
    let id = escape_html(&client.id);
    let hostname = escape_html(&client.metadata.hostname);
    let full_project = escape_html(&client.metadata.project);
    let short_project_escaped = escape_html(&short_project);
    let last_seen = format_relative_time(&client.last_seen);
    let status = client.metadata.status.to_string();
    let is_connected = !matches!(client.metadata.status, ClientStatus::Disconnected);
    let connect_class = if is_connected { "clickable" } else { "" };

    // Build HTML using concat to avoid Rust 2021 raw identifier issues
    [
        "<div class=\"client-card ",
        connect_class,
        "\" id=\"client-",
        &id,
        "\" hx-get=\"/clients/",
        &id,
        "\" hx-trigger=\"refresh from:body\" data-client-id=\"",
        &id,
        "\">",
        "<div class=\"client-header\">",
        "<span class=\"client-title\">",
        &id,
        "</span>",
        "<div class=\"header-right\">",
        "<span class=\"status-badge ",
        status_class,
        "\">",
        &status,
        "</span>",
        "</div></div>",
        "<div class=\"client-body\" onclick=\"connectToClient('",
        &id,
        "')\">",
        "<div class=\"client-info\">",
        "<div class=\"client-hostname\">",
        &hostname,
        "</div>",
        "<div class=\"client-project\" title=\"",
        &full_project,
        "\">",
        &short_project_escaped,
        "</div>",
        "</div>",
        "<div class=\"client-footer\">",
        "<span class=\"last-seen\">",
        &last_seen,
        "</span>",
        "<button class=\"expand-btn\" hx-get=\"/clients/",
        &id,
        "/details\" hx-target=\"#client-",
        &id,
        "\" hx-swap=\"outerHTML\" onclick=\"event.stopPropagation()\" title=\"Show details\">",
        "<span class=\"expand-icon\">▶</span>",
        "</button>",
        "</div></div></div>",
    ]
    .concat()
}

/// Render expanded client card with full details and actions
pub fn render_client_details(client: &Client) -> String {
    let status_class = match client.metadata.status {
        ClientStatus::Idle => "status-idle",
        ClientStatus::Active => "status-active",
        ClientStatus::Busy => "status-busy",
        ClientStatus::Disconnected => "status-disconnected",
    };

    let last_activity_str = client
        .metadata
        .last_activity
        .as_ref()
        .map(|t| format_relative_time(t))
        .unwrap_or_else(|| "No recent activity".to_string());

    let is_connected = !matches!(client.metadata.status, ClientStatus::Disconnected);
    let id = escape_html(&client.id);
    let hostname = escape_html(&client.metadata.hostname);
    let project = escape_html(&client.metadata.project);
    let connected_at = format_relative_time(&client.connected_at);
    let last_seen = format_relative_time(&client.last_seen);
    let last_activity = escape_html(&last_activity_str);
    let status = client.metadata.status.to_string();
    let connect_class = if is_connected { "clickable" } else { "" };

    let disconnect_btn = if is_connected {
        [
            "<button class=\"btn btn-danger btn-sm\" hx-post=\"/clients/",
            &id,
            "/disconnect\" hx-target=\"#clients-list\" hx-swap=\"innerHTML\" ",
            "hx-confirm=\"Disconnect this client?\">Disconnect</button>",
        ]
        .concat()
    } else {
        "<span class=\"text-muted\">Client disconnected</span>".to_string()
    };

    // Build HTML using concat to avoid Rust 2021 raw identifier issues
    [
        "<div class=\"client-card expanded ",
        connect_class,
        "\" id=\"client-",
        &id,
        "\" hx-get=\"/clients/",
        &id,
        "\" hx-trigger=\"refresh from:body\" data-client-id=\"",
        &id,
        "\">",
        "<div class=\"client-header\">",
        "<span class=\"client-title\">",
        &id,
        "</span>",
        "<div class=\"header-right\">",
        "<span class=\"status-badge ",
        status_class,
        "\">",
        &status,
        "</span>",
        "</div></div>",
        "<div class=\"client-body\">",
        "<div class=\"client-info\" onclick=\"connectToClient('",
        &id,
        "')\">",
        "<div class=\"client-hostname\">",
        &hostname,
        "</div>",
        "</div>",
        "<div class=\"client-details\">",
        "<div class=\"detail-row\"><span class=\"detail-label\">Project</span>",
        "<span class=\"detail-value mono\">",
        &project,
        "</span></div>",
        "<div class=\"detail-row\"><span class=\"detail-label\">Connected</span>",
        "<span class=\"detail-value\">",
        &connected_at,
        "</span></div>",
        "<div class=\"detail-row\"><span class=\"detail-label\">Last Seen</span>",
        "<span class=\"detail-value\">",
        &last_seen,
        "</span></div>",
        "<div class=\"detail-row\"><span class=\"detail-label\">Last Activity</span>",
        "<span class=\"detail-value\">",
        &last_activity,
        "</span></div>",
        "</div>",
        "<div class=\"client-actions\">",
        &disconnect_btn,
        "</div>",
        "<div class=\"client-footer\">",
        "<button class=\"expand-btn\" hx-get=\"/clients/",
        &id,
        "\" hx-target=\"#client-",
        &id,
        "\" hx-swap=\"outerHTML\" title=\"Hide details\">",
        "<span class=\"expand-icon\">▼</span>",
        "</button>",
        "</div></div></div>",
    ]
    .concat()
}

/// Render the token list (HTMX partial)
pub fn render_token_list(tokens: &[TokenInfo]) -> String {
    if tokens.is_empty() {
        return r#"
            <div class="empty-state small">
                <p>No tokens created yet.</p>
                <p class="hint">Create a token to connect Claude Code instances.</p>
            </div>
        "#
        .to_string();
    }

    let cards: Vec<String> = tokens.iter().map(render_token_card).collect();
    ["<div class=\"tokens-grid\">", &cards.join("\n"), "</div>"].concat()
}

/// Render a single token card
pub fn render_token_card(token: &TokenInfo) -> String {
    let id = escape_html(&token.id);
    let name = escape_html(&token.name);
    let created_at = format_relative_time(&token.created_at);
    let last_used = token
        .last_used
        .as_ref()
        .map(|t| format_relative_time(t))
        .unwrap_or_else(|| "Never".to_string());

    let status_class = if token.is_revoked {
        "token-revoked"
    } else {
        "token-active"
    };

    let actions = if token.is_revoked {
        "<span class=\"text-muted\">Revoked</span>".to_string()
    } else {
        [
            "<button class=\"btn btn-secondary btn-sm\" hx-post=\"/api/tokens/",
            &id,
            "/revoke\" hx-target=\"#tokens-list\" hx-swap=\"innerHTML\" hx-confirm=\"Revoke this token? Connected clients will be disconnected.\">Revoke</button>",
        ].concat()
    };

    [
        "<div class=\"token-card ",
        status_class,
        "\" id=\"token-",
        &id,
        "\">",
        "<div class=\"token-header\">",
        "<span class=\"token-name\">",
        &name,
        "</span>",
        "<span class=\"token-id mono\">",
        &id[..8.min(id.len())],
        "...</span>",
        "</div>",
        "<div class=\"token-body\">",
        "<div class=\"token-meta\">",
        "<span>Created: ",
        &created_at,
        "</span>",
        "<span>Last used: ",
        &last_used,
        "</span>",
        "</div>",
        "<div class=\"token-actions\">",
        &actions,
        "</div>",
        "</div></div>",
    ]
    .concat()
}

/// Render the token creation modal
pub fn render_token_modal() -> String {
    r##"
    <div class="modal-backdrop" id="modal-backdrop">
        <div class="modal">
            <div class="modal-header">
                <h3>Create Connection Token</h3>
                <button class="modal-close" hx-get="/tokens/close-modal" hx-target="#token-modal" hx-swap="innerHTML">&times;</button>
            </div>
            <form hx-post="/api/tokens" hx-target="#token-modal" hx-swap="innerHTML">
                <div class="modal-body">
                    <div class="form-group">
                        <label for="token-name">Token Name</label>
                        <input type="text" id="token-name" name="name" placeholder="e.g., Work Laptop" required autofocus>
                        <p class="form-hint">A friendly name to identify this token.</p>
                    </div>
                </div>
                <div class="modal-footer">
                    <button type="button" class="btn btn-secondary" hx-get="/tokens/close-modal" hx-target="#token-modal" hx-swap="innerHTML">Cancel</button>
                    <button type="submit" class="btn btn-primary">Create Token</button>
                </div>
            </form>
        </div>
    </div>
    "##
    .to_string()
}

/// Render the token created success modal (shows the token once)
pub fn render_token_created(token_value: &str, name: &str) -> String {
    let token = escape_html(token_value);
    let name_escaped = escape_html(name);

    [
        "<div class=\"modal-backdrop\" id=\"modal-backdrop\">",
        "<div class=\"modal\">",
        "<div class=\"modal-header\">",
        "<h3>Token Created</h3>",
        "<button class=\"modal-close\" hx-get=\"/tokens/close-modal\" hx-target=\"#token-modal\" hx-swap=\"innerHTML\" hx-on::after-request=\"htmx.trigger('#tokens-list', 'load')\">&times;</button>",
        "</div>",
        "<div class=\"modal-body\">",
        "<div class=\"success-icon\">✓</div>",
        "<p class=\"token-name-display\">", &name_escaped, "</p>",
        "<div class=\"token-display\">",
        "<code id=\"new-token\">", &token, "</code>",
        "<button class=\"btn btn-sm btn-secondary copy-btn\">Copy</button>",
        "</div>",
        "<p class=\"warning-text\">⚠️ This token will only be shown once. Save it now!</p>",
        "</div>",
        "<div class=\"modal-footer\">",
        "<button class=\"btn btn-primary\" hx-get=\"/tokens/close-modal\" hx-target=\"#token-modal\" hx-swap=\"innerHTML\" hx-on::after-request=\"htmx.trigger('#tokens-list', 'load')\">Done</button>",
        "</div>",
        "</div></div>",
        "<script>",
        "document.querySelector('.copy-btn').addEventListener('click', function() {",
        "  navigator.clipboard.writeText(document.getElementById('new-token').textContent);",
        "  this.textContent = 'Copied!';",
        "  setTimeout(() => this.textContent = 'Copy', 2000);",
        "});",
        "</script>",
    ].concat()
}

/// Wrap content in the base layout
fn layout(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <style>
        :root {{
            --bg-primary: #0d1117;
            --bg-secondary: #161b22;
            --bg-tertiary: #21262d;
            --text-primary: #f0f6fc;
            --text-secondary: #8b949e;
            --accent: #58a6ff;
            --success: #3fb950;
            --warning: #d29922;
            --error: #f85149;
            --border: #30363d;
        }}

        * {{ box-sizing: border-box; margin: 0; padding: 0; }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            line-height: 1.5;
            min-height: 100vh;
        }}

        .login-container {{
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            padding: 2rem;
            text-align: center;
        }}

        .login-container h1 {{
            font-size: 2.5rem;
            margin-bottom: 1rem;
        }}

        .login-container p {{
            color: var(--text-secondary);
            margin-bottom: 2rem;
            max-width: 400px;
        }}

        .btn {{
            display: inline-flex;
            align-items: center;
            gap: 0.5rem;
            padding: 0.75rem 1.5rem;
            border-radius: 6px;
            text-decoration: none;
            font-weight: 500;
            transition: all 0.2s;
            border: 1px solid transparent;
            cursor: pointer;
        }}

        .btn-primary {{
            background: var(--accent);
            color: var(--bg-primary);
        }}

        .btn-primary:hover {{
            background: #79c0ff;
        }}

        .btn-secondary {{
            background: var(--bg-tertiary);
            color: var(--text-primary);
            border-color: var(--border);
        }}

        .btn-secondary:hover {{
            background: var(--border);
        }}

        .icon {{
            width: 1.25rem;
            height: 1.25rem;
        }}

        .dashboard-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 1rem 2rem;
            background: var(--bg-secondary);
            border-bottom: 1px solid var(--border);
        }}

        .dashboard-header h1 {{
            font-size: 1.5rem;
        }}

        .user-info {{
            display: flex;
            align-items: center;
            gap: 1rem;
        }}

        .dashboard-main {{
            padding: 2rem;
            max-width: 1200px;
            margin: 0 auto;
        }}

        .clients-section h2 {{
            margin-bottom: 1.5rem;
            font-size: 1.25rem;
        }}

        .clients-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
            gap: 1rem;
        }}

        .client-card {{
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            border-radius: 8px;
            overflow: hidden;
            transition: border-color 0.2s, box-shadow 0.2s;
        }}

        .client-card:hover {{
            border-color: var(--accent);
        }}

        .client-card.clickable {{
            cursor: pointer;
        }}

        .client-card.clickable:hover {{
            box-shadow: 0 0 0 1px var(--accent);
        }}

        .client-card.expanded {{
            border-color: var(--accent);
        }}

        .client-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 0.75rem 1rem;
            background: var(--bg-tertiary);
            border-bottom: 1px solid var(--border);
        }}

        .header-right {{
            display: flex;
            align-items: center;
            gap: 0.75rem;
        }}

        .client-title {{
            font-weight: 600;
            font-size: 1rem;
        }}

        .client-hostname {{
            font-size: 0.875rem;
            color: var(--text-secondary);
        }}

        .client-info {{
            cursor: pointer;
            padding: 0.5rem 0;
        }}

        .client-info:hover {{
            color: var(--accent);
        }}

        .client-footer {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-top: 0.75rem;
            padding-top: 0.75rem;
            border-top: 1px solid var(--border);
        }}

        .expand-btn {{
            background: transparent;
            border: 1px solid var(--border);
            border-radius: 4px;
            padding: 0.25rem 0.5rem;
            cursor: pointer;
            color: var(--text-secondary);
            transition: all 0.2s;
        }}

        .expand-btn:hover {{
            background: var(--bg-tertiary);
            border-color: var(--accent);
            color: var(--accent);
        }}

        .expand-icon {{
            font-size: 0.625rem;
            display: inline-block;
        }}

        .status-badge {{
            padding: 0.25rem 0.75rem;
            border-radius: 12px;
            font-size: 0.75rem;
            font-weight: 500;
            text-transform: uppercase;
        }}

        .status-idle {{ background: var(--bg-tertiary); color: var(--text-secondary); }}
        .status-active {{ background: rgba(63, 185, 80, 0.2); color: var(--success); }}
        .status-busy {{ background: rgba(210, 153, 34, 0.2); color: var(--warning); }}
        .status-disconnected {{ background: rgba(248, 81, 73, 0.2); color: var(--error); }}

        .client-body {{
            padding: 1rem;
        }}

        .client-project {{
            font-family: monospace;
            font-size: 0.875rem;
            color: var(--text-secondary);
            margin-bottom: 1rem;
            word-break: break-all;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }}

        .client-meta {{
            display: flex;
            flex-direction: column;
            gap: 0.25rem;
            font-size: 0.75rem;
            color: var(--text-secondary);
        }}

        /* Expanded card details */
        .client-details {{
            display: flex;
            flex-direction: column;
            gap: 0.75rem;
            margin-bottom: 1rem;
        }}

        .detail-row {{
            display: flex;
            justify-content: space-between;
            align-items: flex-start;
            gap: 1rem;
        }}

        .detail-label {{
            font-size: 0.75rem;
            color: var(--text-secondary);
            flex-shrink: 0;
        }}

        .detail-value {{
            font-size: 0.875rem;
            text-align: right;
            word-break: break-all;
        }}

        .detail-value.mono {{
            font-family: monospace;
        }}

        .detail-value.small {{
            font-size: 0.75rem;
            color: var(--text-secondary);
        }}

        /* Client actions */
        .client-actions {{
            display: flex;
            gap: 0.5rem;
            padding-top: 1rem;
            border-top: 1px solid var(--border);
        }}

        .btn-sm {{
            padding: 0.375rem 0.75rem;
            font-size: 0.75rem;
        }}

        .btn-danger {{
            background: var(--error);
            color: white;
            border: none;
        }}

        .btn-danger:hover {{
            background: #da3633;
        }}

        .text-muted {{
            color: var(--text-secondary);
            font-size: 0.875rem;
        }}

        .empty-state {{
            text-align: center;
            padding: 3rem;
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            border-radius: 8px;
        }}

        .empty-state p {{
            color: var(--text-secondary);
        }}

        .empty-state .hint {{
            margin-top: 0.5rem;
            font-size: 0.875rem;
        }}

        .loading {{
            text-align: center;
            padding: 2rem;
            color: var(--text-secondary);
        }}

        /* Section headers */
        .section-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 1rem;
        }}

        .section-header h2 {{
            margin: 0;
        }}

        .section-desc {{
            color: var(--text-secondary);
            font-size: 0.875rem;
            margin-bottom: 1rem;
        }}

        .count-badge {{
            padding: 0.25rem 0.5rem;
            background: var(--bg-tertiary);
            border-radius: 12px;
            font-size: 0.75rem;
            color: var(--text-secondary);
        }}

        .count-badge.has-active {{
            background: rgba(63, 185, 80, 0.2);
            color: var(--success);
        }}

        /* Tokens section */
        .tokens-section {{
            margin-top: 2rem;
            padding-top: 2rem;
            border-top: 1px solid var(--border);
        }}

        .tokens-grid {{
            display: flex;
            flex-direction: column;
            gap: 0.75rem;
        }}

        .token-card {{
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 1rem;
        }}

        .token-card.token-revoked {{
            opacity: 0.6;
        }}

        .token-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 0.5rem;
        }}

        .token-name {{
            font-weight: 600;
        }}

        .token-id {{
            font-size: 0.75rem;
            color: var(--text-secondary);
        }}

        .token-meta {{
            display: flex;
            gap: 1.5rem;
            font-size: 0.75rem;
            color: var(--text-secondary);
            margin-bottom: 0.75rem;
        }}

        .token-actions {{
            display: flex;
            gap: 0.5rem;
        }}

        .empty-state.small {{
            padding: 1.5rem;
        }}

        /* Modal */
        .modal-backdrop {{
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(0, 0, 0, 0.7);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 1000;
        }}

        .modal {{
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            border-radius: 12px;
            width: 100%;
            max-width: 450px;
            margin: 1rem;
        }}

        .modal-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 1rem 1.5rem;
            border-bottom: 1px solid var(--border);
        }}

        .modal-header h3 {{
            margin: 0;
            font-size: 1.125rem;
        }}

        .modal-close {{
            background: none;
            border: none;
            font-size: 1.5rem;
            color: var(--text-secondary);
            cursor: pointer;
            line-height: 1;
        }}

        .modal-close:hover {{
            color: var(--text-primary);
        }}

        .modal-body {{
            padding: 1.5rem;
        }}

        .modal-footer {{
            display: flex;
            justify-content: flex-end;
            gap: 0.75rem;
            padding: 1rem 1.5rem;
            border-top: 1px solid var(--border);
        }}

        .form-group {{
            margin-bottom: 1rem;
        }}

        .form-group label {{
            display: block;
            margin-bottom: 0.5rem;
            font-weight: 500;
        }}

        .form-group input {{
            width: 100%;
            padding: 0.75rem;
            background: var(--bg-primary);
            border: 1px solid var(--border);
            border-radius: 6px;
            color: var(--text-primary);
            font-size: 1rem;
        }}

        .form-group input:focus {{
            outline: none;
            border-color: var(--accent);
        }}

        .form-hint {{
            margin-top: 0.5rem;
            font-size: 0.75rem;
            color: var(--text-secondary);
        }}

        .success-icon {{
            font-size: 3rem;
            color: var(--success);
            text-align: center;
            margin-bottom: 1rem;
        }}

        .token-name-display {{
            text-align: center;
            font-weight: 600;
            margin-bottom: 1rem;
        }}

        .token-display {{
            display: flex;
            gap: 0.5rem;
            align-items: stretch;
            background: var(--bg-primary);
            padding: 0.75rem;
            border-radius: 6px;
            border: 1px solid var(--border);
        }}

        .token-display code {{
            flex: 1;
            font-size: 0.75rem;
            word-break: break-all;
            color: var(--accent);
        }}

        .warning-text {{
            margin-top: 1rem;
            padding: 0.75rem;
            background: rgba(210, 153, 34, 0.1);
            border: 1px solid rgba(210, 153, 34, 0.3);
            border-radius: 6px;
            color: var(--warning);
            font-size: 0.875rem;
            text-align: center;
        }}

        .mono {{
            font-family: monospace;
        }}

        /* Mobile responsive styles */
        @media (max-width: 768px) {{
            .dashboard-header {{
                flex-direction: column;
                gap: 1rem;
                text-align: center;
                padding: 1rem;
            }}

            .dashboard-header h1 {{
                font-size: 1.25rem;
            }}

            .user-info {{
                width: 100%;
                justify-content: center;
            }}

            .dashboard-main {{
                padding: 1rem;
            }}

            .clients-grid {{
                grid-template-columns: 1fr;
            }}

            .section-header {{
                flex-direction: column;
                gap: 0.75rem;
                align-items: flex-start;
            }}

            .section-header h2 {{
                font-size: 1.1rem;
            }}

            .client-card {{
                margin: 0;
            }}

            .client-header {{
                flex-wrap: wrap;
                gap: 0.5rem;
            }}

            .header-right {{
                width: 100%;
                justify-content: space-between;
            }}

            .detail-row {{
                flex-direction: column;
                gap: 0.25rem;
            }}

            .detail-value {{
                text-align: left;
            }}

            .client-actions {{
                flex-wrap: wrap;
            }}

            .token-meta {{
                flex-direction: column;
                gap: 0.5rem;
            }}

            .modal {{
                margin: 0.5rem;
                max-height: 90vh;
                overflow-y: auto;
            }}

            .modal-body {{
                padding: 1rem;
            }}

            .modal-footer {{
                flex-direction: column-reverse;
                gap: 0.5rem;
            }}

            .modal-footer button {{
                width: 100%;
            }}

            .token-display {{
                flex-direction: column;
            }}

            .token-display button {{
                width: 100%;
            }}
        }}

        @media (max-width: 480px) {{
            .login-container h1 {{
                font-size: 2rem;
            }}

            .btn {{
                padding: 0.625rem 1rem;
                font-size: 0.875rem;
            }}

            .status-badge {{
                padding: 0.2rem 0.5rem;
                font-size: 0.7rem;
            }}

            .client-project {{
                font-size: 0.75rem;
            }}
        }}
    </style>
</head>
<body>
    {}
</body>
</html>"#,
        escape_html(title),
        content
    )
}

/// Escape HTML special characters
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Format a timestamp for display (ISO string to human-readable)
fn format_timestamp(ts: &str) -> String {
    // For now, just return the timestamp as-is
    // In production, you'd format this nicely
    ts.to_string()
}

/// Format timestamp as relative time (e.g., "2 minutes ago")
fn format_relative_time(ts: &str) -> String {
    // Parse ISO timestamp and calculate relative time
    // For now, return a simplified version
    if ts.is_empty() {
        return "Unknown".to_string();
    }

    // Try to extract just the time portion for display
    if let Some(time_part) = ts.split('T').nth(1) {
        if let Some(time) = time_part.split('.').next() {
            return format!("at {}", time);
        }
    }

    ts.to_string()
}

/// Truncate a file path to fit in a given width
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }

    // Try to keep the last part of the path visible
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 2 {
        return format!("...{}", &path[path.len().saturating_sub(max_len - 3)..]);
    }

    // Keep first and last parts
    let last = parts.last().unwrap_or(&"");
    let first = parts.first().unwrap_or(&"");

    if first.len() + last.len() + 5 <= max_len {
        format!("{}/.../{}", first, last)
    } else {
        format!(".../{}", last)
    }
}
