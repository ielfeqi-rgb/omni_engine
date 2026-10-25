// ============================================================================
// [SOVEREIGN ENGINE CONSTITUTION - INVIOLABLE SAFETY DIRECTIVE]
// STRICT ARCHITECTURAL RULE:
// It is strictly forbidden, under any circumstances or for any reason whatsoever,
// to remove, comment out, bypass, or weaken `#![deny(unused_must_use)]` from
// this file or ANY other .rs file in this codebase.
// All fallible operations, Results, and Options must be explicitly handled.
// Zero silent error swallowing under the rug. This engine is built to endure for decades.
// ============================================================================
#![deny(unused_must_use)]

// [GUIDANCE] No tests exist for any HTTP handler. Add integration tests.
// The shutdown handler (line ~129) does not catch SIGTERM/SIGINT -- orphan processes result.
use crate::auth::KeyManager;
use crate::downloader::ModelDownloader;
use crate::llama_manager::LlamaManager;
use crate::logger::LogBuffer;
use crate::openai_api::{self, ChatCompletionRequest};
use crate::system_info::{self, SystemSpecs};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub key_manager: KeyManager,
    pub downloader: ModelDownloader,
    pub llama_manager: LlamaManager,
    pub log_buffer: LogBuffer,
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::ORIGIN,
        ])
        .allow_origin(Any);

    Router::new()
        .route("/", get(serve_index))
        .route("/api/status", get(handle_status))
        .route("/api/system/specs", get(handle_system_specs))
        .route("/api/logs", get(handle_get_logs))
        .route("/api/models/download", post(handle_start_download))
        .route("/api/downloads", get(handle_get_downloads))
        .route("/api/server/start", post(handle_start_server))
        .route("/api/server/stop", post(handle_stop_server))
        .route("/api/shutdown", post(handle_shutdown_server))
        .route("/api/keys", get(handle_list_keys))
        .route("/api/keys/generate", post(handle_create_key))
        .route("/api/keys/revoke/:id", delete(handle_revoke_key))
        // OpenAI Public Spec Endpoints with API Key authentication
        .route("/v1/models", get(handle_v1_models))
        .route("/v1/chat/completions", post(handle_v1_chat_completions))
        .layer(cors)
        .with_state(state)
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn handle_status(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.llama_manager.status();
    Json(status)
}

async fn handle_system_specs() -> Json<SystemSpecs> {
    Json(system_info::get_system_specs())
}

async fn handle_get_logs(State(state): State<AppState>) -> Json<Vec<String>> {
    Json(state.log_buffer.get_logs())
}

#[derive(Deserialize)]
struct DownloadReq {
    url: String,
    filename: String,
}

async fn handle_start_download(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<DownloadReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check_admin_auth(&headers, &state.key_manager)?;
    state.log_buffer.push(format!("Starting model download from: {}", payload.url));
    let task_id = state.downloader.start_download(payload.url, payload.filename);
    Ok(Json(json!({ "task_id": task_id, "status": "started" })))
}

async fn handle_get_downloads(State(state): State<AppState>) -> impl IntoResponse {
    let tasks = state.downloader.get_tasks();
    Json(tasks)
}

#[derive(Deserialize)]
struct StartServerReq {
    model: String,
    threads: Option<usize>,
    ctx_size: Option<usize>,
    port: Option<u16>,
}

async fn handle_start_server(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<StartServerReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check_admin_auth(&headers, &state.key_manager)?;
    let threads = payload.threads.unwrap_or(4);
    let ctx = payload.ctx_size.unwrap_or(4096);
    let port = payload.port.unwrap_or(8081);

    state.log_buffer.push(format!("Starting llama-server with model '{}' on port {}", payload.model, port));

    match state.llama_manager.start(payload.model, port, threads, ctx) {
        Ok(pid) => {
            state.log_buffer.push(format!("llama-server started successfully with PID {}", pid));
            Ok((StatusCode::OK, Json(json!({ "pid": pid, "status": "running" }))))
        }
        Err(e) => {
            state.log_buffer.push(format!("Error starting llama-server: {}", e));
            Ok((StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
        }
    }
}

async fn handle_stop_server(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check_admin_auth(&headers, &state.key_manager)?;
    state.log_buffer.push("Stopping llama-server...".to_string());
    match state.llama_manager.stop() {
        Ok(_) => Ok(Json(json!({ "status": "stopped" }))),
        Err(e) => Ok(Json(json!({ "error": e }))),
    }
}

async fn handle_shutdown_server(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check_admin_auth(&headers, &state.key_manager)?;
    state.log_buffer.push("Shutting down entire Omni Engine process... Goodbye!".to_string());
    let _ = state.llama_manager.stop();

    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        std::process::exit(0);
    });

    Ok(Json(json!({ "status": "shutting_down", "message": "Omni Engine is shutting down safely." })))
}

async fn handle_list_keys(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check_admin_auth(&headers, &state.key_manager)?;
    let keys = state.key_manager.list_keys();
    Ok(Json(keys))
}

#[derive(Deserialize)]
struct CreateKeyReq {
    name: String,
}

async fn handle_create_key(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<CreateKeyReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check_admin_auth(&headers, &state.key_manager)?;
    let new_key = state.key_manager.create_key(payload.name.clone());
    state.log_buffer.push(format!("Generated new API Key: '{}'", payload.name));
    Ok(Json(new_key))
}

async fn handle_revoke_key(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check_admin_auth(&headers, &state.key_manager)?;
    let ok = state.key_manager.revoke_key(&id);
    state.log_buffer.push(format!("Revoked API Key ID: {}", id));
    Ok(Json(json!({ "revoked": ok })))
}

fn check_admin_auth(headers: &HeaderMap, key_manager: &KeyManager) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    // 1. If valid Bearer API key is present, ALWAYS authorized
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(raw_token) = auth_header.to_str() {
            let token_str = raw_token.strip_prefix("Bearer ").unwrap_or(raw_token);
            if key_manager.validate_key(token_str) {
                return Ok(());
            }
        }
    }

    // 2. Reject Cross-Origin requests without Bearer API key (CSRF protection)
    if let Some(origin) = headers.get("Origin") {
        if let Ok(origin_str) = origin.to_str() {
            if !origin_str.starts_with("http://127.0.0.1") && !origin_str.starts_with("http://localhost") {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": {
                            "message": "Cross-Origin Admin access forbidden without valid Bearer API key.",
                            "type": "forbidden",
                            "code": 403
                        }
                    })),
                ));
            }
        }
    }

    // 3. Reject Cross-Site requests
    if let Some(sec_site) = headers.get("sec-fetch-site") {
        if let Ok(site_str) = sec_site.to_str() {
            if site_str == "cross-site" {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": {
                            "message": "Cross-Site request to Admin API blocked. Provide valid Bearer API key.",
                            "type": "forbidden",
                            "code": 403
                        }
                    })),
                ));
            }
        }
    }

    Ok(())
}

fn check_auth(headers: &HeaderMap, key_manager: &KeyManager) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(raw_token) = auth_header.to_str() {
            let token_str = raw_token.strip_prefix("Bearer ").unwrap_or(raw_token);
            if key_manager.validate_key(token_str) {
                return Ok(());
            }
        }
    }
    Err((
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": {
                "message": "Missing or invalid API key. Please provide 'Authorization: Bearer omni_sk_...'",
                "type": "unauthorized",
                "code": 401
            }
        })),
    ))
}

async fn handle_v1_models(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state.key_manager)?;
    let models = state.llama_manager.list_available_models();
    let resp = openai_api::handle_list_models(models).await;
    Ok(resp)
}

async fn handle_v1_chat_completions(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state.key_manager)?;
    state.log_buffer.push(format!("OpenAI API Request: /v1/chat/completions (messages={})", payload.messages.len()));
    
    let model_opt = {
        let lock = state.llama_manager.native_model.lock().unwrap();
        lock.clone()
    };
    
    if let Some(model) = model_opt {
        openai_api::native_chat_completion(payload, model).await
    } else {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": {
                    "message": "No model loaded. Call /api/engine/start first.",
                    "type": "model_not_loaded"
                }
            }))
        ))
    }
}
