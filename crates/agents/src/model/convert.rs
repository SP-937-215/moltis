use crate::{multimodal::parse_data_uri, runner::sanitize_tool_result};

use super::{
    chat::{ChatMessage, ContentPart, UserContent},
    decode_tool_call_arguments_with_diagnostic,
    types::{TOOL_CALL_METADATA_KEYS, ToolCall},
};

/// Extract allowlisted metadata keys from a tool-call JSON object.
///
/// Returns `None` when no metadata keys are present, keeping the common path
/// allocation-free.
#[must_use]
pub fn extract_tool_call_metadata(
    tc: &serde_json::Value,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let obj = tc.as_object()?;
    let nested = obj.get("metadata").and_then(serde_json::Value::as_object);
    let gemini = obj
        .get("extra_content")
        .and_then(|extra| extra.get("google"))
        .and_then(serde_json::Value::as_object);
    let meta: serde_json::Map<_, _> = TOOL_CALL_METADATA_KEYS
        .iter()
        .filter_map(|&k| {
            obj.get(k)
                .or_else(|| nested.and_then(|metadata| metadata.get(k)))
                .or_else(|| gemini.and_then(|metadata| metadata.get(k)))
                .map(|v| (k.to_string(), v.clone()))
        })
        .collect();
    if meta.is_empty() {
        None
    } else {
        Some(meta)
    }
}

fn document_absolute_path_from_media_ref(media_ref: &str) -> String {
    use std::path::Path;
    if Path::new(media_ref).is_absolute() {
        return media_ref.to_string();
    }

    moltis_config::data_dir()
        .join("sessions")
        .join(media_ref)
        .to_string_lossy()
        .to_string()
}

fn truncate_url(url: &str) -> String {
    url.chars().take(80).collect()
}

fn mime_from_filename(filename: &str) -> String {
    match filename
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "gif" => "image/gif".to_string(),
        "webp" => "image/webp".to_string(),
        "svg" => "image/svg+xml".to_string(),
        _ => "image/jpeg".to_string(),
    }
}

fn read_session_media_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() {
        return None;
    }
    std::fs::read(path).ok()
}

fn load_session_media_image(url: &str) -> Option<ContentPart> {
    if moltis_sessions::parse_session_media_api_url(url).is_none() {
        return None;
    }
    let store =
        moltis_sessions::store::SessionStore::new(moltis_config::data_dir().join("sessions"));
    let Some(path) = store.media_path_for_api_url(url) else {
        tracing::warn!(url_prefix = %truncate_url(url), "session media image URL did not resolve");
        return None;
    };
    let Some(bytes) = read_session_media_bytes(&path) else {
        tracing::warn!(path = %path.display(), "failed to read session media image");
        return None;
    };
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image");
    let (data, media_type) = match moltis_media::image_ops::optimize_for_llm(&bytes, None) {
        Ok(image) => (image.data, image.media_type),
        Err(_) => (bytes, mime_from_filename(filename)),
    };
    Some(ContentPart::Image {
        media_type,
        data: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data),
    })
}

/// Convert an OpenAI-style `image_url` into a provider image part.
///
/// Data URIs are parsed in place. Session media URLs are read from disk and
/// converted to base64 so providers never have to fetch the Moltis origin.
#[must_use]
pub fn image_url_to_content_part(url: &str) -> Option<ContentPart> {
    if let Some((media_type, data)) = parse_data_uri(url) {
        return Some(ContentPart::Image {
            media_type: media_type.to_string(),
            data: data.to_string(),
        });
    }
    load_session_media_image(url)
}

/// Convert persisted JSON messages (from session store) to typed `ChatMessage`s.
///
/// Skips messages that don't have a valid `role` field, logging a warning.
/// Metadata fields (`created_at`, `model`, `provider`, `inputTokens`,
/// `outputTokens`, `channel`) are silently dropped — they only exist in
/// the persisted JSON, not in `ChatMessage`.
pub fn values_to_chat_messages(values: &[serde_json::Value]) -> Vec<ChatMessage> {
    values_to_chat_messages_inner(values, true, None)
}

/// Convert persisted JSON messages (from session store) to typed `ChatMessage`s,
/// sanitizing tool results before they are sent back to an LLM provider.
pub fn values_to_chat_messages_with_tool_result_limit(
    values: &[serde_json::Value],
    max_tool_result_bytes: usize,
) -> Vec<ChatMessage> {
    values_to_chat_messages_inner(values, true, Some(max_tool_result_bytes))
}

/// Convert provider-format JSON messages to typed `ChatMessage`s without
/// dropping tool results.
///
/// Hook-modified LLM payloads are already provider-bound, so preserve their
/// tool messages exactly instead of applying session-store orphan filtering.
pub fn provider_values_to_chat_messages(values: &[serde_json::Value]) -> Vec<ChatMessage> {
    values_to_chat_messages_inner(values, false, None)
}

fn maybe_limit_tool_result_content(
    content: String,
    max_tool_result_bytes: Option<usize>,
) -> String {
    max_tool_result_bytes
        .map(|max_bytes| sanitize_tool_result(&content, max_bytes))
        .unwrap_or(content)
}

fn values_to_chat_messages_inner(
    values: &[serde_json::Value],
    filter_orphan_tool_results: bool,
    max_tool_result_bytes: Option<usize>,
) -> Vec<ChatMessage> {
    let mut messages = Vec::with_capacity(values.len());
    // Track tool_call IDs emitted by assistant messages so we only include
    // tool/tool_result messages that have a matching assistant tool_call.
    // Orphan tool results (e.g. from explicit /sh commands) would cause
    // provider API errors.
    let mut pending_tool_call_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for (i, val) in values.iter().enumerate() {
        let Some(role) = val["role"].as_str() else {
            tracing::warn!(index = i, "skipping message with missing/invalid role");
            continue;
        };
        match role {
            "system" => {
                let content = val["content"].as_str().unwrap_or("").to_string();
                messages.push(ChatMessage::system(content));
            },
            "user" => {
                // Extract sender name from persisted channel metadata.
                let sender_name = val
                    .get("channel")
                    .and_then(|ch| {
                        ch["sender_name"]
                            .as_str()
                            .or_else(|| ch["username"].as_str())
                    })
                    .or_else(|| val["name"].as_str())
                    .map(|s| s.to_string());

                let document_context = val["documents"].as_array().and_then(|documents| {
                    let mut sections = Vec::new();
                    for document in documents {
                        let Some(display_name) = document["display_name"].as_str() else {
                            continue;
                        };
                        let Some(mime_type) = document["mime_type"].as_str() else {
                            continue;
                        };
                        let Some(media_ref) = document["media_ref"].as_str() else {
                            continue;
                        };
                        let absolute_path = document_absolute_path_from_media_ref(media_ref);
                        sections.push(format!(
                            "filename: {display_name}\nmime_type: {mime_type}\nlocal_path: {absolute_path}\nmedia_ref: {media_ref}"
                        ));
                    }
                    if sections.is_empty() {
                        None
                    } else {
                        let mut rendered = vec!["[Inbound documents available]".to_string()];
                        rendered.extend(sections);
                        Some(rendered.join("\n\n"))
                    }
                });

                // Content can be a string or an array (multimodal).
                if let Some(text) = val["content"].as_str() {
                    let content = if let Some(ref document_context) = document_context {
                        if text.trim().is_empty() {
                            document_context.clone()
                        } else {
                            format!("{text}\n\n{document_context}")
                        }
                    } else {
                        text.to_string()
                    };
                    messages.push(ChatMessage::User {
                        content: UserContent::Text(content),
                        name: sender_name,
                    });
                } else if let Some(arr) = val["content"].as_array() {
                    let mut parts: Vec<ContentPart> = arr
                        .iter()
                        .filter_map(|block| {
                            let block_type = block["type"].as_str()?;
                            match block_type {
                                "text" => {
                                    let text = block["text"].as_str()?.to_string();
                                    Some(ContentPart::Text(text))
                                },
                                "image_url" => {
                                    let url = block["image_url"]["url"].as_str()?;
                                    image_url_to_content_part(url)
                                },
                                _ => None,
                            }
                        })
                        .collect();
                    if let Some(document_context) = document_context {
                        if let Some(ContentPart::Text(text)) = parts
                            .iter_mut()
                            .find(|part| matches!(part, ContentPart::Text(_)))
                        {
                            if !text.trim().is_empty() {
                                text.push_str("\n\n");
                            }
                            text.push_str(&document_context);
                        } else {
                            parts.insert(0, ContentPart::Text(document_context));
                        }
                    }
                    messages.push(ChatMessage::User {
                        content: UserContent::Multimodal(parts),
                        name: sender_name,
                    });
                } else {
                    messages.push(ChatMessage::User {
                        content: UserContent::Text(document_context.unwrap_or_default()),
                        name: sender_name,
                    });
                }
            },
            "assistant" => {
                let content = val["content"].as_str().map(|s| s.to_string());
                let reasoning = val["reasoning"].as_str().and_then(|s| {
                    let trimmed = s.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                });
                let tool_calls: Vec<ToolCall> = val["tool_calls"]
                    .as_array()
                    .map(|tcs| {
                        tcs.iter()
                            .filter_map(|tc| {
                                let id = tc["id"].as_str()?.to_string();
                                let name = tc["function"]["name"].as_str()?.to_string();
                                let decoded = decode_tool_call_arguments_with_diagnostic(
                                    tc["function"].get("arguments"),
                                );
                                let metadata = extract_tool_call_metadata(tc);
                                Some(ToolCall {
                                    id,
                                    name,
                                    arguments: decoded.arguments,
                                    argument_diagnostic: decoded.diagnostic,
                                    metadata,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                for tc in &tool_calls {
                    pending_tool_call_ids.insert(tc.id.clone());
                }
                messages.push(ChatMessage::Assistant {
                    content,
                    tool_calls,
                    reasoning,
                });
            },
            "tool" => {
                let tool_call_id = val["tool_call_id"].as_str().unwrap_or("").to_string();
                let has_matching_assistant = pending_tool_call_ids.remove(&tool_call_id);
                if filter_orphan_tool_results && !has_matching_assistant {
                    tracing::debug!(tool_call_id, "skipping orphan tool message");
                    continue;
                }
                let content = if let Some(s) = val["content"].as_str() {
                    s.to_string()
                } else {
                    val["content"].to_string()
                };
                let content = maybe_limit_tool_result_content(content, max_tool_result_bytes);
                messages.push(ChatMessage::tool(tool_call_id, content));
            },
            // tool_result entries are persisted tool execution output; convert
            // them to standard tool messages so the LLM sees its own results.
            "tool_result" => {
                let tool_call_id = val["tool_call_id"].as_str().unwrap_or("").to_string();
                let has_matching_assistant = pending_tool_call_ids.remove(&tool_call_id);
                if filter_orphan_tool_results && !has_matching_assistant {
                    tracing::debug!(tool_call_id, "skipping orphan tool_result message");
                    continue;
                }
                if let Some(reasoning) = val["reasoning"].as_str().and_then(|s| {
                    let trimmed = s.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                }) {
                    attach_reasoning_to_assistant_tool_call(
                        &mut messages,
                        &tool_call_id,
                        reasoning,
                    );
                }
                let content = if let Some(err) = val["error"].as_str() {
                    format!("Error: {err}")
                } else if let Some(result) = val.get("result") {
                    if let Some(s) = result.as_str() {
                        s.to_string()
                    } else {
                        result.to_string()
                    }
                } else {
                    String::new()
                };
                let content = maybe_limit_tool_result_content(content, max_tool_result_bytes);
                messages.push(ChatMessage::tool(tool_call_id, content));
            },
            // notice entries are UI-only informational messages.
            "notice" => continue,
            other => {
                tracing::warn!(
                    index = i,
                    role = other,
                    "skipping message with unknown role"
                );
            },
        }
    }
    messages
}

fn attach_reasoning_to_assistant_tool_call(
    messages: &mut [ChatMessage],
    tool_call_id: &str,
    tool_reasoning: String,
) {
    for message in messages.iter_mut().rev() {
        let ChatMessage::Assistant {
            tool_calls,
            reasoning,
            ..
        } = message
        else {
            continue;
        };

        if tool_calls
            .iter()
            .any(|tool_call| tool_call.id == tool_call_id)
        {
            if reasoning.is_none() {
                *reasoning = Some(tool_reasoning);
            }
            return;
        }
    }
    tracing::debug!(
        tool_call_id,
        "no assistant message found for reasoning attachment"
    );
}

#[cfg(test)]
mod image_url_tests {
    use super::*;
    use moltis_sessions::store::SessionStore;

    static DATA_DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct DataDirGuard;
    impl Drop for DataDirGuard {
        fn drop(&mut self) {
            moltis_config::clear_data_dir();
        }
    }

    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn image_url_to_content_part_parses_data_uri() {
        let part = image_url_to_content_part("data:image/png;base64,abc123").unwrap();
        match part {
            ContentPart::Image { media_type, data } => {
                assert_eq!(media_type, "image/png");
                assert_eq!(data, "abc123");
            },
            ContentPart::Text(_) => panic!("expected image part"),
        }
    }

    #[tokio::test]
    async fn image_url_to_content_part_loads_session_media() {
        let _lock = DATA_DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        moltis_config::set_data_dir(dir.path().to_path_buf());
        let _guard = DataDirGuard;
        let store = SessionStore::new(dir.path().join("sessions"));
        store
            .save_media("main", "photo.png", TINY_PNG)
            .await
            .unwrap();

        let part = image_url_to_content_part("/api/sessions/main/media/photo.png").unwrap();
        match part {
            ContentPart::Image { media_type, data } => {
                assert!(media_type.starts_with("image/"));
                assert!(!data.is_empty());
                assert!(!data.starts_with("data:"));
            },
            ContentPart::Text(_) => panic!("expected image part"),
        }
    }

    #[tokio::test]
    async fn values_to_chat_messages_expand_session_media_urls() {
        let _lock = DATA_DIR_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        moltis_config::set_data_dir(dir.path().to_path_buf());
        let _guard = DataDirGuard;
        let store = SessionStore::new(dir.path().join("sessions"));
        store
            .save_media("main", "photo.png", TINY_PNG)
            .await
            .unwrap();

        let values = vec![serde_json::json!({
            "role": "user",
            "content": [
                { "type": "text", "text": "what is this" },
                {
                    "type": "image_url",
                    "image_url": { "url": "/api/sessions/main/media/photo.png" }
                }
            ]
        })];
        let msgs = values_to_chat_messages(&values);
        match &msgs[0] {
            ChatMessage::User {
                content: UserContent::Multimodal(parts),
                ..
            } => {
                assert!(matches!(&parts[0], ContentPart::Text(text) if text == "what is this"));
                assert!(matches!(&parts[1], ContentPart::Image { .. }));
            },
            _ => panic!("expected multimodal user message"),
        }
    }
}
