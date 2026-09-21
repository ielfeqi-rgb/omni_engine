use axum::{
    body::Body,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub stream: Option<bool>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelObject {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelListResponse {
    pub object: String,
    pub data: Vec<ModelObject>,
}

pub async fn handle_list_models(available_models: Vec<String>) -> Json<ModelListResponse> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut data = Vec::new();
    if available_models.is_empty() {
        data.push(ModelObject {
            id: "omni-llama-default".to_string(),
            object: "model".to_string(),
            created: now,
            owned_by: "omni_engine".to_string(),
        });
    } else {
        for m in available_models {
            data.push(ModelObject {
                id: m,
                object: "model".to_string(),
                created: now,
                owned_by: "omni_engine".to_string(),
            });
        }
    }

    Json(ModelListResponse {
        object: "list".to_string(),
        data,
    })
}

pub async fn proxy_chat_completion(
    req: ChatCompletionRequest,
    target_port: u16,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let is_stream = req.stream.unwrap_or(false);
    let client = reqwest::Client::new();
    let target_url = format!("http://127.0.0.1:{}/v1/chat/completions", target_port);

    info!("Proxying chat completion to {} (stream={})", target_url, is_stream);

    let res = client
        .post(&target_url)
        .json(&req)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to connect to llama-server backend: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "message": format!("llama-server backend error or not running on port {}: {}", target_port, e),
                        "type": "backend_unavailable",
                        "code": 503
                    }
                })),
            )
        })?;

    if !res.status().is_success() {
        let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_REQUEST);
        let text = res.text().await.unwrap_or_default();
        return Err((
            status,
            Json(json!({
                "error": {
                    "message": text,
                    "type": "llama_server_error"
                }
            })),
        ));
    }

    if is_stream {
        let stream = res.bytes_stream().map(|item| {
            item.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        });

        let mut headers = HeaderMap::new();
        headers.insert("content-type", "text/event-stream".parse().unwrap());
        headers.insert("cache-control", "no-cache".parse().unwrap());
        headers.insert("connection", "keep-alive".parse().unwrap());

        let body = Body::from_stream(stream);
        Ok((headers, body).into_response())
    } else {
        let json_body: serde_json::Value = res.json().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "message": format!("Failed to parse json response: {}", e)
                    }
                })),
            )
        })?;

        Ok(Json(json_body).into_response())
    }
}
