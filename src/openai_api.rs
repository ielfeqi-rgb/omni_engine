// [GUIDANCE] This module has ZERO tests. It is the main user-facing API endpoint.
// Add timeout to reqwest client: .timeout(Duration::from_secs(300))
// Add at least one integration test (mock llama-server or use a real model).
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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
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

pub fn build_prompt(messages: &[ChatMessage]) -> String {
    let mut prompt = String::new();
    for msg in messages {
        prompt.push_str(&format!("<|im_start|>{}
{}<|im_end|>
", msg.role, msg.content));
    }
    prompt.push_str("<|im_start|>assistant
");
    prompt
}

pub async fn native_chat_completion(
    req: ChatCompletionRequest,
    model: std::sync::Arc<crate::native_llama::NativeLlamaModel>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let is_stream = req.stream.unwrap_or(false);
    let prompt = build_prompt(&req.messages);
    let max_tokens = req.max_tokens.unwrap_or(512) as usize;
    
    let temp = req.temperature.unwrap_or(0.7);
    
    let mut config = crate::native_llama::SamplingConfig::default();
    config.temperature = temp;
    if temp <= 0.0 {
         
    }

    if !is_stream {
        let res = tokio::task::spawn_blocking(move || {
            let mut ctx = model.create_context(4096, 512, 4).map_err(|e| e.to_string())?;
            let tokens = model.tokenize(&prompt, true).map_err(|e| e.to_string())?;
            ctx.eval_tokens(&tokens, 0).map_err(|e| e.to_string())?;
            
            let mut sampler = crate::native_llama::NativeLlamaSampler::new(&config).map_err(|e| e.to_string())?;
            let mut generated = String::new();
            
            for _ in 0..max_tokens {
                let token = ctx.sample(&mut sampler).map_err(|e| e.to_string())?;
                let piece = model.token_to_piece(token).map_err(|e| e.to_string())?;
                
                if piece.contains("<|im_end|>") || piece.contains("</s>") {
                    break;
                }
                
                generated.push_str(&piece);
                ctx.eval_tokens(&[token], 0).map_err(|e| e.to_string())?;
            }
            Ok::<String, String>(generated)
        }).await.unwrap_or_else(|_| Err("Spawn blocking failed".to_string()));
        
        match res {
            Ok(text) => {
                let body = json!({
                    "id": "chatcmpl-native",
                    "object": "chat.completion",
                    "created": 1337,
                    "model": req.model.unwrap_or_else(|| "omni".to_string()),
                    "choices": [{
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": text
                        },
                        "finish_reason": "stop"
                    }]
                });
                return Ok(Json(body).into_response());
            }
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": { "message": e } }))
                ));
            }
        }
    }
    
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<axum::body::Bytes, std::io::Error>>();
    let model_clone = req.model.clone().unwrap_or_else(|| "omni".to_string());
    
    tokio::task::spawn_blocking(move || {
        let mut ctx = match model.create_context(4096, 512, 4) {
            Ok(c) => c,
            Err(e) => { let _ = tx.send(Err(std::io::Error::new(std::io::ErrorKind::Other, e))); return; }
        };
        let tokens = match model.tokenize(&prompt, true) {
            Ok(t) => t,
            Err(e) => { let _ = tx.send(Err(std::io::Error::new(std::io::ErrorKind::Other, e))); return; }
        };
        if let Err(e) = ctx.eval_tokens(&tokens, 0) {
            let _ = tx.send(Err(std::io::Error::new(std::io::ErrorKind::Other, e))); return;
        }
        
        let mut sampler = match crate::native_llama::NativeLlamaSampler::new(&config) {
            Ok(s) => s,
            Err(e) => { let _ = tx.send(Err(std::io::Error::new(std::io::ErrorKind::Other, e))); return; }
        };
        
        for _ in 0..max_tokens {
            let token = match ctx.sample(&mut sampler) {
                Ok(t) => t,
                Err(e) => { let _ = tx.send(Err(std::io::Error::new(std::io::ErrorKind::Other, e))); return; }
            };
            let piece = match model.token_to_piece(token) {
                Ok(p) => p,
                Err(e) => { let _ = tx.send(Err(std::io::Error::new(std::io::ErrorKind::Other, e))); return; }
            };
            
            if piece.contains("<|im_end|>") || piece.contains("</s>") {
                break;
            }
            
            let chunk = json!({
                "id": "chatcmpl-native",
                "object": "chat.completion.chunk",
                "model": &model_clone,
                "choices": [{
                    "delta": {
                        "content": piece
                    }
                }]
            });
            let data = format!("data: {}\n\n", chunk.to_string());
            if tx.send(Ok(axum::body::Bytes::from(data))).is_err() {
                break;
            }
            
            if ctx.eval_tokens(&[token], 0).is_err() {
                break;
            }
        }
        let _ = tx.send(Ok(axum::body::Bytes::from("data: [DONE]\n\n")));
    });

    let stream = async_stream::stream! {
        while let Some(item) = rx.recv().await {
            yield item;
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert("content-type", "text/event-stream".parse().unwrap());
    headers.insert("cache-control", "no-cache".parse().unwrap());
    headers.insert("connection", "keep-alive".parse().unwrap());

    Ok((headers, Body::from_stream(stream)).into_response())
}
