use agent_meter_mcp_wrapper::proxy;

/// A minimal MCP upstream that responds to tools/list and tools/call.
async fn mock_upstream() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        let app = axum::Router::new()
            .route("/", axum::routing::post(handle_mcp))
            .route("/health", axum::routing::get(|| async { "ok" }));

        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    base_url
}

async fn handle_mcp(
    axum::Json(body): axum::Json<serde_json::Value>,
) -> axum::Json<serde_json::Value> {
    let method = body["method"].as_str().unwrap_or("");
    let id = body.get("id");

    match method {
        "tools/list" => axum::Json(serde_json::json!({
            "jsonrpc": "2.0", "id": id,
            "result": {"tools": [{"name": "mock_tool", "description": "mock", "inputSchema": {"type": "object"}}]}
        })),
        "tools/call" => axum::Json(serde_json::json!({
            "jsonrpc": "2.0", "id": id,
            "result": {"content": [{"type": "text", "text": "ok"}], "isError": false}
        })),
        _ => axum::Json(serde_json::json!({
            "jsonrpc": "2.0", "id": id,
            "error": {"code": -32601, "message": "method not found"}
        })),
    }
}

async fn spawned_proxy(upstream: &str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let app = proxy::router(
        upstream.to_string(),
        "http://collector-test.invalid".into(),
        reqwest::Client::new(),
    );

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    base_url
}

#[tokio::test]
async fn test_proxy_health() {
    let upstream = mock_upstream().await;
    let proxy = spawned_proxy(&upstream).await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/health", proxy))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_proxy_tools_list() {
    let upstream = mock_upstream().await;
    let proxy = spawned_proxy(&upstream).await;
    let client = reqwest::Client::new();

    let req = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}});
    let resp = client
        .post(format!("{}/", proxy))
        .json(&req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["jsonrpc"], "2.0");
    assert!(body["result"]["tools"].is_array());
}

#[tokio::test]
async fn test_proxy_tools_call() {
    let upstream = mock_upstream().await;
    let proxy = spawned_proxy(&upstream).await;
    let client = reqwest::Client::new();

    let req = serde_json::json!({
        "jsonrpc":"2.0","id":2,"method":"tools/call",
        "params":{"name":"mock_tool","arguments":{"x":1}}
    });
    let resp = client
        .post(format!("{}/", proxy))
        .json(&req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"]["content"][0]["text"], "ok");
}

#[tokio::test]
async fn test_proxy_unknown_method() {
    let upstream = mock_upstream().await;
    let proxy = spawned_proxy(&upstream).await;
    let client = reqwest::Client::new();

    let req = serde_json::json!({"jsonrpc":"2.0","id":3,"method":"unknown_method","params":{}});
    let resp = client
        .post(format!("{}/", proxy))
        .json(&req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some(), "unknown methods should error");
}

#[tokio::test]
async fn test_proxy_upstream_down() {
    // Point at a port that's not listening
    let app = proxy::router(
        "http://127.0.0.1:1".into(),
        "http://collector-test.invalid".into(),
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap(),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let req = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}});
    let resp = client
        .post(format!("{}/", base_url))
        .json(&req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some(), "should return upstream error");
    assert_eq!(body["error"]["code"], -32603);
}
