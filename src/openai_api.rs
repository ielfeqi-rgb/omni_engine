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
pub struct ToolDefinition {
    pub r#type: String, // "function"
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String, // "function"
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub stream: Option<bool>,
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar_root: Option<String>,
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

pub fn format_tools_system_prompt(tools: &[ToolDefinition]) -> String {
    let mut s = String::from(
        "\n# Available Tools\nYou have access to the following functions to query data or perform actions:\n\n"
    );
    for tool in tools {
        s.push_str(&format!("## Function: `{}`\n", tool.function.name));
        if let Some(ref desc) = tool.function.description {
            s.push_str(&format!("Description: {}\n", desc));
        }
        if let Some(ref params) = tool.function.parameters {
            s.push_str(&format!("Parameters (JSON Schema): {}\n", params));
        }
        s.push('\n');
    }
    s.push_str(
        "IMPORTANT RULES:\n\
        1. Tool & Database Priority: When the user asks about products, prices, stock, materials, or placing orders, ALWAYS invoke the appropriate tool first to retrieve verified real data. Never fabricate, guess, or hallucinate prices or product specifications.\n\
        2. To invoke a tool, output strictly:\n\
        <tool_call>\n\
        {\"name\": \"function_name\", \"arguments\": { ... }}\n\
        </tool_call>\n\
        When calling a tool, do not add natural language commentary before or after.\n\
        3. After receiving tool results, formulate a helpful, friendly, and complete natural language response to the user in their language (e.g. Arabic) presenting the information clearly. Do NOT call another tool unless additional data is needed."
    );
    s
}

pub fn build_tools_gbnf(tools: &[ToolDefinition]) -> String {
    let mut names = Vec::new();
    for t in tools {
        names.push(format!("\"\\\"{}\\\"\"", t.function.name));
    }
    let tool_names_rule = if names.is_empty() {
        "\"\\\"unknown\\\"\"".to_string()
    } else {
        names.join(" | ")
    };

    format!(
r#"root ::= (tool-call | text)+
tool-call ::= "<tool_call>\n" "{{" ws "\"name\":" ws tool-name ws "," ws "\"arguments\":" ws object ws "}}\n</tool_call>"
tool-name ::= {}
ws ::= [ \t\n]*
object ::= "{{" ws (string ws ":" ws value (ws "," ws string ws ":" ws value)*)? ws "}}"
value ::= object | array | string | number | "true" | "false" | "null"
array ::= "[" ws (value (ws "," ws value)*)? ws "]"
string ::= "\"" ([^"\\] | "\\" .)* "\""
number ::= "-"? [0-9]+ ("." [0-9]+)?
text ::= [^<]+ | "<"
"#,
        tool_names_rule
    )
}

pub fn resolve_canonical_tool_name<'a>(
    name: &'a str,
    valid_tools: Option<&'a [ToolDefinition]>,
) -> Option<&'a str> {
    let tools = match valid_tools {
        Some(t) => t,
        None => return Some(name),
    };

    // 1. Exact match
    if let Some(t) = tools.iter().find(|t| t.function.name == name) {
        return Some(&t.function.name);
    }
    // 2. Case-insensitive match
    if let Some(t) = tools.iter().find(|t| t.function.name.eq_ignore_ascii_case(name)) {
        return Some(&t.function.name);
    }
    // 3. Normalized punctuation match (strip _, -, spaces)
    let norm_name = name.replace(['_', '-', ' '], "").to_lowercase();
    if let Some(t) = tools.iter().find(|t| t.function.name.replace(['_', '-', ' '], "").to_lowercase() == norm_name) {
        return Some(&t.function.name);
    }
    // 4. Token overlap match (e.g. "get_price" words are a subset of "get_product_price")
    let name_tokens: Vec<&str> = name
        .split(|c: char| c == '_' || c == '-' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect();

    let matching_tools: Vec<&'a ToolDefinition> = tools
        .iter()
        .filter(|t| {
            let t_tokens: Vec<&str> = t.function.name
                .split(|c: char| c == '_' || c == '-' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .collect();
            let all_in_t = !name_tokens.is_empty() && name_tokens.iter().all(|nt| t_tokens.iter().any(|tt| tt.eq_ignore_ascii_case(nt)));
            let all_in_name = !t_tokens.is_empty() && t_tokens.iter().all(|tt| name_tokens.iter().any(|nt| nt.eq_ignore_ascii_case(tt)));
            all_in_t || all_in_name
        })
        .collect();

    if matching_tools.len() == 1 {
        return Some(&matching_tools[0].function.name);
    }

    None
}

pub fn extract_tool_calls(
    text: &str,
    valid_tools: Option<&[ToolDefinition]>,
) -> (Option<String>, Option<Vec<ToolCall>>) {
    let mut tool_calls = Vec::new();
    let mut clean_text = String::new();
    let mut cursor = 0;

    let open_tag = "<tool_call>";
    let close_tag = "</tool_call>";

    while let Some(start_pos) = text[cursor..].find(open_tag) {
        let abs_start = cursor + start_pos;
        clean_text.push_str(&text[cursor..abs_start]);

        let inner_start = abs_start + open_tag.len();
        if let Some(end_pos) = text[inner_start..].find(close_tag) {
            let abs_end = inner_start + end_pos;
            let raw_json = text[inner_start..abs_end].trim();

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw_json) {
                if let Some(raw_name) = val.get("name").and_then(|n| n.as_str()) {
                    if let Some(canonical_name) = resolve_canonical_tool_name(raw_name, valid_tools) {
                        let arguments = if let Some(args) = val.get("arguments") {
                            if args.is_string() {
                                args.as_str().unwrap().to_string()
                            } else {
                                args.to_string()
                            }
                        } else {
                            "{}".to_string()
                        };

                        tool_calls.push(ToolCall {
                            id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                            r#type: "function".to_string(),
                            function: FunctionCall {
                                name: canonical_name.to_string(),
                                arguments,
                            },
                        });
                    } else {
                        tracing::warn!("[TOOL CALL REJECTED] Unrecognized tool name '{}' was rejected.", raw_name);
                    }
                }
            }
            cursor = abs_end + close_tag.len();
        } else {
            clean_text.push_str(&text[abs_start..]);
            cursor = text.len();
            break;
        }
    }

    if cursor < text.len() {
        clean_text.push_str(&text[cursor..]);
    }

    if tool_calls.is_empty() {
        let mut trimmed = text.trim();
        // Strip markdown code fences if present (e.g. ```json ... ```)
        if trimmed.starts_with("```") {
            if let Some(first_nl) = trimmed.find('\n') {
                trimmed = &trimmed[first_nl + 1..];
            }
            if let Some(end_fence) = trimmed.rfind("```") {
                trimmed = &trimmed[..end_fence];
            }
            trimmed = trimmed.trim();
        }

        if let (Some(start_brace), Some(end_brace)) = (trimmed.find('{'), trimmed.rfind('}')) {
            if end_brace >= start_brace {
                let candidate = &trimmed[start_brace..=end_brace];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(candidate) {
                    if let Some(raw_name) = val.get("name").and_then(|n| n.as_str()) {
                        if let Some(canonical_name) = resolve_canonical_tool_name(raw_name, valid_tools) {
                            let arguments = if let Some(args) = val.get("arguments") {
                                if args.is_string() {
                                    args.as_str().unwrap().to_string()
                                } else {
                                    args.to_string()
                                }
                            } else {
                                "{}".to_string()
                            };

                            tool_calls.push(ToolCall {
                                id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                                r#type: "function".to_string(),
                                function: FunctionCall {
                                    name: canonical_name.to_string(),
                                    arguments,
                                },
                            });
                            return (None, Some(tool_calls));
                        }
                    }
                }
            }
        }
    }

    let final_content = if clean_text.trim().is_empty() {
        if tool_calls.is_empty() {
            Some(text.to_string())
        } else {
            None
        }
    } else {
        Some(clean_text.trim().to_string())
    };

    let final_calls = if tool_calls.is_empty() {
        None
    } else {
        Some(tool_calls)
    };

    (final_content, final_calls)
}

pub fn build_prompt_with_config(
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
    cfg: &crate::native_llama::auto_detect::OmniModelConfig,
) -> String {
    let tools_instruction = tools.filter(|t| !t.is_empty()).map(|t| format_tools_system_prompt(t));

    let mut system_text: Option<String> = None;
    for msg in messages {
        if msg.role == "system" {
            let base = msg.content.as_deref().unwrap_or_default();
            if let Some(ref tools_prompt) = tools_instruction {
                system_text = Some(format!("{}{}", base, tools_prompt));
            } else {
                system_text = Some(base.to_string());
            }
            break;
        }
    }
    if system_text.is_none() {
        if let Some(ref tools_prompt) = tools_instruction {
            system_text = Some(tools_prompt.clone());
        }
    }

    let mut prompt = String::new();

    if let Some(ref sys) = system_text {
        if !sys.trim().is_empty() {
            prompt.push_str(&cfg.system_prefix);
            prompt.push_str(sys);
            prompt.push_str(&cfg.turn_suffix);
        }
    }

    for msg in messages {
        match msg.role.as_str() {
            "system" => {
                // Handled in system block above
            }
            "user" => {
                prompt.push_str(&cfg.user_prefix);
                if let Some(ref content) = msg.content {
                    prompt.push_str(content);
                }
                prompt.push_str(&cfg.turn_suffix);
            }
            "assistant" => {
                prompt.push_str(&cfg.assistant_prefix);
                if let Some(ref content) = msg.content {
                    prompt.push_str(content);
                }
                if let Some(ref calls) = msg.tool_calls {
                    for call in calls {
                        prompt.push_str(&format!(
                            "\n<tool_call>\n{{\"name\": \"{}\", \"arguments\": {}}}\n</tool_call>",
                            call.function.name, call.function.arguments
                        ));
                    }
                }
                prompt.push_str(&cfg.turn_suffix);
            }
            "tool" => {
                prompt.push_str(&cfg.user_prefix);
                let id = msg.tool_call_id.as_deref().unwrap_or("unknown");
                let content = msg.content.as_deref().unwrap_or("");
                prompt.push_str(&format!("[Tool Result (call_id: {})]: {}", id, content));
                prompt.push_str(&cfg.turn_suffix);
            }
            _ => {}
        }
    }

    prompt.push_str(&cfg.assistant_prefix);
    prompt
}

pub fn build_prompt(
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
    model: &crate::native_llama::NativeLlamaModel,
) -> String {
    build_prompt_with_config(messages, tools, &model.config)
}

pub async fn native_chat_completion(
    req: ChatCompletionRequest,
    model: std::sync::Arc<crate::native_llama::NativeLlamaModel>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let is_stream = req.stream.unwrap_or(false);
    let prompt = build_prompt(&req.messages, req.tools.as_deref(), &model);
    let max_tokens = req.max_tokens.unwrap_or(512) as usize;
    
    let temp = req.temperature.unwrap_or(0.7);
    
    let mut config = crate::native_llama::SamplingConfig::default();
    config.temperature = temp;
    if let Some(ref g) = req.grammar {
        config.grammar = Some(g.clone());
        config.grammar_root = req.grammar_root.clone();
    }

    if !is_stream {
        let res = tokio::task::spawn_blocking(move || {
            let mut ctx = model.create_context(4096, 512, 4).map_err(|e| e.to_string())?;
            let tokens = model.tokenize(&prompt, true).map_err(|e| e.to_string())?;
            ctx.eval_tokens(&tokens, 0).map_err(|e| e.to_string())?;
            
            let mut sampler = crate::native_llama::NativeLlamaSampler::with_model(&config, &model).map_err(|e| e.to_string())?;
            let mut generated = String::new();
            
            for _ in 0..max_tokens {
                let token = ctx.sample(&mut sampler).map_err(|e| e.to_string())?;
                let piece = model.token_to_piece(token).map_err(|e| e.to_string())?;
                
                if piece.is_empty() || model.is_stop_token(token) || model.is_stop_piece(&piece) {
                    break;
                }
                
                generated.push_str(&piece);
                ctx.eval_tokens(&[token], 0).map_err(|e| e.to_string())?;
            }
            Ok::<String, String>(generated)
        }).await.unwrap_or_else(|_| Err("Spawn blocking failed".to_string()));
        
        match res {
            Ok(text) => {
                let has_tools = req.tools.as_ref().map(|t| !t.is_empty()).unwrap_or(false);
                let (content, tool_calls) = if has_tools {
                    extract_tool_calls(&text, req.tools.as_deref())
                } else {
                    (Some(text), None)
                };
                let finish_reason = if tool_calls.is_some() { "tool_calls" } else { "stop" };
                let body = json!({
                    "id": format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
                    "object": "chat.completion",
                    "created": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                    "model": req.model.unwrap_or_else(|| "omni".to_string()),
                    "choices": [{
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": content,
                            "tool_calls": tool_calls
                        },
                        "finish_reason": finish_reason
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
        
        let mut sampler = match crate::native_llama::NativeLlamaSampler::with_model(&config, &model) {
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
            
            if piece.is_empty() || model.is_stop_token(token) || model.is_stop_piece(&piece) {
                break;
            }
            
            let chunk = json!({
                "id": format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tool_calls_with_tags() {
        let text = "Let me check that for you.\n<tool_call>\n{\"name\": \"get_product_price\", \"arguments\": {\"item\": \"black_tshirt\"}}\n</tool_call>";
        let (content, calls) = extract_tool_calls(text, None);
        assert_eq!(content, Some("Let me check that for you.".to_string()));
        let calls = calls.expect("Should extract tool calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "get_product_price");
        assert!(calls[0].function.arguments.contains("black_tshirt"));
    }

    #[test]
    fn test_extract_tool_calls_markdown_code_fences() {
        let text = "```json\n{\"name\": \"get_product_price\", \"arguments\": {\"sku\": \"cX23\"}}\n```";
        let (content, calls) = extract_tool_calls(text, None);
        assert_eq!(content, None);
        let calls = calls.expect("Should extract tool calls from markdown code fence");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "get_product_price");
        assert!(calls[0].function.arguments.contains("cX23"));
    }

    #[test]
    fn test_extract_tool_calls_pure_json() {
        let text = "{\"name\": \"get_stock\", \"arguments\": {\"id\": 42}}";
        let (content, calls) = extract_tool_calls(text, None);
        assert_eq!(content, None);
        let calls = calls.expect("Should extract tool calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "get_stock");
        assert!(calls[0].function.arguments.contains("42"));
    }

    #[test]
    fn test_extract_tool_calls_rejects_hallucinated_name() {
        let valid_tools = vec![ToolDefinition {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: "get_product_price".to_string(),
                description: None,
                parameters: None,
            },
        }];

        // Model attempts to call an unregistered tool "hack_database"
        let text = "<tool_call>\n{\"name\": \"hack_database\", \"arguments\": {}}\n</tool_call>";
        let (_content, calls) = extract_tool_calls(text, Some(&valid_tools));
        assert!(calls.is_none(), "Unregistered tool must be rejected by validator");

        // Model calls the registered tool "get_product_price"
        let valid_text = "<tool_call>\n{\"name\": \"get_product_price\", \"arguments\": {\"item\": \"shoes\"}}\n</tool_call>";
        let (_content, valid_calls) = extract_tool_calls(valid_text, Some(&valid_tools));
        assert!(valid_calls.is_some(), "Registered tool must be accepted");
        assert_eq!(valid_calls.unwrap()[0].function.name, "get_product_price");
    }

    #[test]
    fn test_extract_tool_calls_none() {
        let text = "Hello! Welcome to our store. How can I help you today?";
        let (content, calls) = extract_tool_calls(text, None);
        assert_eq!(content, Some(text.to_string()));
        assert!(calls.is_none());
    }

    #[test]
    fn test_chat_message_serde_with_tools() {
        let json_str = r#"{
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "call_123",
                "type": "function",
                "function": {
                    "name": "lookup",
                    "arguments": "{\"q\": \"shoes\"}"
                }
            }]
        }"#;
        let msg: ChatMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, None);
        assert!(msg.tool_calls.is_some());
        let tc = msg.tool_calls.unwrap();
        assert_eq!(tc[0].function.name, "lookup");
    }

    #[test]
    fn test_format_tools_system_prompt() {
        let tools = vec![ToolDefinition {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: "check_price".to_string(),
                description: Some("Check item price".to_string()),
                parameters: Some(json!({"type": "object", "properties": {"item": {"type": "string"}}})),
            },
        }];
        let prompt = format_tools_system_prompt(&tools);
        assert!(prompt.contains("## Function: `check_price`"));
        assert!(prompt.contains("Check item price"));
        assert!(prompt.contains("<tool_call>"));
    }

    #[test]
    fn test_fuzzy_tool_resolution() {
        let tools = vec![
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_product_price".to_string(),
                    description: None,
                    parameters: None,
                },
            },
            ToolDefinition {
                r#type: "function".to_string(),
                function: FunctionDefinition {
                    name: "track_customer_order".to_string(),
                    description: None,
                    parameters: None,
                },
            },
        ];

        // 1. Model writes "get_price" or "getProductPrice" instead of exact "get_product_price"
        let call_text = "<tool_call>\n{\"name\": \"get_price\", \"arguments\": {\"sku\": \"cX23\"}}\n</tool_call>";
        let (_, calls) = extract_tool_calls(call_text, Some(&tools));
        assert!(calls.is_some(), "Fuzzy matcher must resolve 'get_price' to 'get_product_price'");
        assert_eq!(calls.unwrap()[0].function.name, "get_product_price");

        // 2. Model writes "track_order" instead of "track_customer_order"
        let call_text2 = "<tool_call>\n{\"name\": \"track_order\", \"arguments\": {\"id\": \"123\"}}\n</tool_call>";
        let (_, calls2) = extract_tool_calls(call_text2, Some(&tools));
        assert!(calls2.is_some());
        assert_eq!(calls2.unwrap()[0].function.name, "track_customer_order");

        // 3. Complete hallucination must still be rejected
        let call_text3 = "<tool_call>\n{\"name\": \"delete_database\", \"arguments\": {}}\n</tool_call>";
        let (_, calls3) = extract_tool_calls(call_text3, Some(&tools));
        assert!(calls3.is_none(), "Unrelated malicious or hallucinated tools must be rejected");
    }

    #[test]
    fn test_build_prompt_multi_turn() {
        let mut cfg = crate::native_llama::auto_detect::OmniModelConfig::default();
        cfg.system_prefix = "<|im_start|>system\n".to_string();
        cfg.user_prefix = "<|im_start|>user\n".to_string();
        cfg.assistant_prefix = "<|im_start|>assistant\n".to_string();
        cfg.turn_suffix = "<|im_end|>\n".to_string();

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: Some("Store assistant".to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: Some("عندكم حاجة ووتربروف؟".to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: Some("نعم لدينا جاكيت ووتربروف".to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: Some("السعر؟".to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];

        let prompt = build_prompt_with_config(&messages, None, &cfg);
        assert!(prompt.contains("<|im_start|>system\nStore assistant<|im_end|>\n"));
        assert!(prompt.contains("<|im_start|>user\nعندكم حاجة ووتربروف؟<|im_end|>\n"));
        assert!(prompt.contains("<|im_start|>assistant\nنعم لدينا جاكيت ووتربروف<|im_end|>\n"));
        assert!(prompt.contains("<|im_start|>user\nالسعر؟<|im_end|>\n"));
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }
}
