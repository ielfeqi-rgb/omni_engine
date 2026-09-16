use crate::auth::KeyManager;
use crate::downloader::ModelDownloader;
use crate::llama_manager::LlamaManager;
use crate::logger::LogBuffer;
use crate::openai_api::{self, ChatCompletionRequest};
use crate::system_info::{self, SystemSpecs};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
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
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

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
    State(state): State<AppState>,
    Json(payload): Json<DownloadReq>,
) -> impl IntoResponse {
    state.log_buffer.push(format!("Starting model download from: {}", payload.url));
    let task_id = state.downloader.start_download(payload.url, payload.filename);
    Json(json!({ "task_id": task_id, "status": "started" }))
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
    State(state): State<AppState>,
    Json(payload): Json<StartServerReq>,
) -> impl IntoResponse {
    let threads = payload.threads.unwrap_or(4);
    let ctx = payload.ctx_size.unwrap_or(4096);
    let port = payload.port.unwrap_or(8081);

    state.log_buffer.push(format!("Starting llama-server with model '{}' on port {}", payload.model, port));

    match state.llama_manager.start(payload.model, port, threads, ctx) {
        Ok(pid) => {
            state.log_buffer.push(format!("llama-server started successfully with PID {}", pid));
            (StatusCode::OK, Json(json!({ "pid": pid, "status": "running" })))
        }
        Err(e) => {
            state.log_buffer.push(format!("Error starting llama-server: {}", e));
            (StatusCode::BAD_REQUEST, Json(json!({ "error": e })))
        }
    }
}

async fn handle_stop_server(State(state): State<AppState>) -> impl IntoResponse {
    state.log_buffer.push("Stopping llama-server...".to_string());
    match state.llama_manager.stop() {
        Ok(_) => Json(json!({ "status": "stopped" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn handle_shutdown_server(State(state): State<AppState>) -> impl IntoResponse {
    state.log_buffer.push("Shutting down entire Omni Engine process... Goodbye!".to_string());
    let _ = state.llama_manager.stop();

    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        std::process::exit(0);
    });

    Json(json!({ "status": "shutting_down", "message": "Omni Engine is shutting down safely." }))
}

async fn handle_list_keys(State(state): State<AppState>) -> impl IntoResponse {
    let keys = state.key_manager.list_keys();
    Json(keys)
}

#[derive(Deserialize)]
struct CreateKeyReq {
    name: String,
}

async fn handle_create_key(
    State(state): State<AppState>,
    Json(payload): Json<CreateKeyReq>,
) -> impl IntoResponse {
    let new_key = state.key_manager.create_key(payload.name.clone());
    state.log_buffer.push(format!("Generated new API Key: '{}'", payload.name));
    Json(new_key)
}

async fn handle_revoke_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let ok = state.key_manager.revoke_key(&id);
    state.log_buffer.push(format!("Revoked API Key ID: {}", id));
    Json(json!({ "revoked": ok }))
}

fn check_auth(headers: &HeaderMap, key_manager: &KeyManager) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(token_str) = auth_header.to_str() {
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
    let status = state.llama_manager.status();
    let port = status.port;
    openai_api::proxy_chat_completion(payload, port).await
}
