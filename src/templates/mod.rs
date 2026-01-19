use crate::models::{Client, ClientStatus, User};

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
    layout(
        "Dashboard - AI Orchestrator",
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
                    <div id="clients-list"
                         hx-get="/clients"
                         hx-trigger="load, every 30s"
                         hx-swap="innerHTML">
                        <div class="loading">Loading clients...</div>
                    </div>
                </section>
            </main>
            <script>
                // WebSocket connection for real-time updates
                const ws = new WebSocket(
                    (location.protocol === 'https:' ? 'wss:' : 'ws:') +
                    '//' + location.host + '/ws/connect?type=browser'
                );

                ws.onmessage = (event) => {{
                    const msg = JSON.parse(event.data);
                    if (msg.type === 'client_update' || msg.type === 'client_disconnected') {{
                        // Trigger HTMX refresh
                        htmx.trigger('#clients-list', 'refresh');
                    }}
                }};

                ws.onclose = () => {{
                    console.log('WebSocket closed, will reconnect on next poll');
                }};
            </script>
            "#,
            escape_html(&user.github_login)
        ),
    )
}

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

    format!(
        r#"
        <div class="client-card" id="client-{id}" hx-get="/clients/{id}" hx-trigger="refresh from:body">
            <div class="client-header" hx-get="/clients/{id}/details" hx-target="#client-{id}" hx-swap="outerHTML">
                <span class="client-hostname">{hostname}</span>
                <div class="header-right">
                    <span class="status-badge {status_class}">{status}</span>
                    <span class="expand-icon">▶</span>
                </div>
            </div>
            <div class="client-body">
                <div class="client-project" title="{full_project}">{short_project}</div>
                <div class="client-meta">
                    <span class="last-seen">Last seen: {last_seen}</span>
                </div>
            </div>
        </div>
        "#,
        id = escape_html(&client.id),
        hostname = escape_html(&client.metadata.hostname),
        status_class = status_class,
        status = client.metadata.status,
        full_project = escape_html(&client.metadata.project),
        short_project = escape_html(&short_project),
        last_seen = format_relative_time(&client.last_seen)
    )
}

/// Render expanded client card with full details and actions
pub fn render_client_details(client: &Client) -> String {
    let status_class = match client.metadata.status {
        ClientStatus::Idle => "status-idle",
        ClientStatus::Active => "status-active",
        ClientStatus::Busy => "status-busy",
        ClientStatus::Disconnected => "status-disconnected",
    };

    let last_activity = client
        .metadata
        .last_activity
        .as_ref()
        .map(|t| format_relative_time(t))
        .unwrap_or_else(|| "No recent activity".to_string());

    let is_connected = !matches!(client.metadata.status, ClientStatus::Disconnected);

    format!(
        r#"
        <div class="client-card expanded" id="client-{id}" hx-get="/clients/{id}" hx-trigger="refresh from:body">
            <div class="client-header" hx-get="/clients/{id}" hx-target="#client-{id}" hx-swap="outerHTML">
                <span class="client-hostname">{hostname}</span>
                <div class="header-right">
                    <span class="status-badge {status_class}">{status}</span>
                    <span class="expand-icon">▼</span>
                </div>
            </div>
            <div class="client-body">
                <div class="client-details">
                    <div class="detail-row">
                        <span class="detail-label">Project</span>
                        <span class="detail-value mono">{project}</span>
                    </div>
                    <div class="detail-row">
                        <span class="detail-label">Connected</span>
                        <span class="detail-value">{connected_at}</span>
                    </div>
                    <div class="detail-row">
                        <span class="detail-label">Last Seen</span>
                        <span class="detail-value">{last_seen}</span>
                    </div>
                    <div class="detail-row">
                        <span class="detail-label">Last Activity</span>
                        <span class="detail-value">{last_activity}</span>
                    </div>
                    <div class="detail-row">
                        <span class="detail-label">Client ID</span>
                        <span class="detail-value mono small">{id}</span>
                    </div>
                </div>
                <div class="client-actions">
                    {disconnect_btn}
                </div>
            </div>
        </div>
        "#,
        id = escape_html(&client.id),
        hostname = escape_html(&client.metadata.hostname),
        status_class = status_class,
        status = client.metadata.status,
        project = escape_html(&client.metadata.project),
        connected_at = format_relative_time(&client.connected_at),
        last_seen = format_relative_time(&client.last_seen),
        last_activity = escape_html(&last_activity),
        disconnect_btn = if is_connected {
            format!(
                r#"<button class="btn btn-danger btn-sm" hx-post="/clients/{}/disconnect" hx-confirm="Disconnect this client?">Disconnect</button>"#,
                escape_html(&client.id)
            )
        } else {
            r#"<span class="text-muted">Client disconnected</span>"#.to_string()
        }
    )
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
            transition: border-color 0.2s;
        }}

        .client-card:hover {{
            border-color: var(--accent);
        }}

        .client-card.expanded {{
            border-color: var(--accent);
        }}

        .client-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 1rem;
            background: var(--bg-tertiary);
            border-bottom: 1px solid var(--border);
            cursor: pointer;
            transition: background 0.2s;
        }}

        .client-header:hover {{
            background: var(--border);
        }}

        .header-right {{
            display: flex;
            align-items: center;
            gap: 0.75rem;
        }}

        .expand-icon {{
            font-size: 0.75rem;
            color: var(--text-secondary);
            transition: transform 0.2s;
        }}

        .client-hostname {{
            font-weight: 600;
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
