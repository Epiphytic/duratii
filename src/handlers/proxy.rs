use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::*;

use crate::auth::AuthMiddleware;

/// Proxy request to send to the Durable Object
#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub query: Option<String>,
}

/// Proxy response from the Durable Object
#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// Proxy HTTP requests to claudecodeui instances
pub async fn proxy_to_client(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Authenticate user
    let user = match AuthMiddleware::require_auth(&req, &ctx.env).await? {
        Ok(user) => user,
        Err(redirect) => return Ok(redirect),
    };

    // Get client ID from path parameter
    let client_id = ctx
        .param("id")
        .ok_or("Missing client ID")?
        .clone();

    // Get the proxy path (everything after /proxy/)
    let proxy_path = ctx.param("path").unwrap_or(&"".to_string()).clone();

    // Get query string from original request
    let url = req.url()?;
    let query_string = url.query().map(|q| q.to_string());

    // Collect headers (filter out hop-by-hop headers)
    let mut headers: Vec<(String, String)> = Vec::new();
    let hop_by_hop = [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailers",
        "transfer-encoding",
        "upgrade",
        "host",
    ];

    for (key, value) in req.headers() {
        let key_lower = key.to_lowercase();
        if !hop_by_hop.contains(&key_lower.as_str()) {
            headers.push((key, value));
        }
    }

    // Get request body if present
    let body = if req.method() != Method::Get && req.method() != Method::Head {
        req.text().await.ok()
    } else {
        None
    };

    // Build proxy request
    let proxy_req = ProxyRequest {
        method: req.method().to_string(),
        path: format!("/{}", proxy_path),
        headers,
        body,
        query: query_string,
    };

    // Forward to user's Durable Object
    let namespace = ctx.env.durable_object("USER_HUB")?;
    let id = namespace.id_from_name(&user.id)?;
    let stub = id.get_stub()?;

    // Create request to DO's proxy endpoint
    let do_url = format!("https://do/proxy/{}", client_id);
    let mut init = RequestInit::new();
    init.with_method(Method::Post);

    let body_json = serde_json::to_string(&proxy_req)?;
    let do_headers = Headers::new();
    do_headers.set("Content-Type", "application/json")?;
    init.with_headers(do_headers);
    init.with_body(Some(JsValue::from_str(&body_json)));

    let do_req = Request::new_with_init(&do_url, &init)?;
    let do_resp = stub.fetch_with_request(do_req).await?;

    // Check if we got a successful proxy response
    if do_resp.status_code() != 200 {
        return Ok(do_resp);
    }

    // Parse the proxy response
    let mut do_resp_mut = do_resp;
    let resp_text = do_resp_mut.text().await?;
    let proxy_resp: ProxyResponse = serde_json::from_str(&resp_text)
        .map_err(|e| Error::RustError(format!("Failed to parse proxy response: {}", e)))?;

    // Build the response to return to the client
    let mut resp_headers = Headers::new();
    for (key, value) in &proxy_resp.headers {
        let key_lower = key.to_lowercase();
        // Skip hop-by-hop headers in response too
        if !hop_by_hop.contains(&key_lower.as_str()) {
            let _ = resp_headers.set(key, value);
        }
    }

    // URL rewriting is handled by claudecodeui (it receives proxy_base in the request)
    let response_body = proxy_resp.body;

    // Create response with the proxied status and body
    // We need to create a new response with the correct status
    // worker-rs doesn't have a clean way to set status, so we rebuild it
    let response = if proxy_resp.status >= 400 {
        Response::error(&response_body, proxy_resp.status)
            .map(|r| r.with_headers(resp_headers))?
    } else {
        Response::ok(response_body)?.with_headers(resp_headers)
    };

    Ok(response)
}

/// Rewrite absolute URLs in HTML to go through the proxy path
fn rewrite_html_urls(html: &str, client_id: &str) -> String {
    let proxy_base = format!("/clients/{}/proxy", client_id);

    // Rewrite common absolute URL patterns in HTML
    // src="/..." -> src="/clients/{id}/proxy/..."
    // href="/..." -> href="/clients/{id}/proxy/..."
    // action="/..." -> action="/clients/{id}/proxy/..."
    html.replace("src=\"/", &format!("src=\"{}/", proxy_base))
        .replace("href=\"/", &format!("href=\"{}/", proxy_base))
        .replace("action=\"/", &format!("action=\"{}/", proxy_base))
        .replace("src='/", &format!("src='{}/", proxy_base))
        .replace("href='/", &format!("href='{}/", proxy_base))
        .replace("action='/", &format!("action='{}/", proxy_base))
}
