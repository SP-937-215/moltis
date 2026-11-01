//! Operator documents sent in a group without a mention.
//!
//! The mention rule drops those messages before download. Hold the saved file
//! and attach it to the next allowed message in the same chat.

use std::collections::HashMap;

use moltis_channels::ChannelDocumentFile;

use crate::access::AccessDenied;

const MAX_HELD_PER_CHAT: usize = 5;

#[derive(Debug, Clone)]
pub struct HeldDocument {
    pub file: ChannelDocumentFile,
}

#[derive(Debug, Default)]
pub struct PendingDocuments {
    by_chat: HashMap<String, Vec<HeldDocument>>,
}

impl PendingDocuments {
    pub fn hold(&mut self, chat_id: &str, document: HeldDocument) {
        let pending = self.by_chat.entry(chat_id.to_string()).or_default();
        pending.push(document);
        let overflow = pending.len().saturating_sub(MAX_HELD_PER_CHAT);
        if overflow > 0 {
            pending.drain(0..overflow);
        }
    }

    pub fn take(&mut self, chat_id: &str) -> Vec<HeldDocument> {
        self.by_chat.remove(chat_id).unwrap_or_default()
    }
}

#[must_use]
pub fn should_hold_unmentioned_document(
    reason: &AccessDenied,
    sender_is_operator: bool,
    has_downloadable_document: bool,
) -> bool {
    *reason == AccessDenied::NotMentioned && sender_is_operator && has_downloadable_document
}

pub fn merge_held_documents(
    body: &str,
    documents: Option<Vec<ChannelDocumentFile>>,
    held: Vec<HeldDocument>,
) -> (String, Option<Vec<ChannelDocumentFile>>) {
    if held.is_empty() {
        return (body.to_string(), documents);
    }
    let names = held
        .iter()
        .map(|document| document.file.display_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut merged = documents.unwrap_or_default();
    merged.extend(held.into_iter().map(|document| document.file));
    let note = format!("[Files uploaded just before this mention: {names}]");
    let body = if body.trim().is_empty() {
        note
    } else {
        format!("{body}\n\n{note}")
    };
    (body, Some(merged))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> HeldDocument {
        HeldDocument {
            file: ChannelDocumentFile {
                display_name: name.to_string(),
                stored_filename: format!("stored-{name}"),
                mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                    .to_string(),
                size_bytes: Some(12),
            },
        }
    }

    #[test]
    fn only_an_operator_document_without_a_mention_is_held() {
        assert!(should_hold_unmentioned_document(
            &AccessDenied::NotMentioned,
            true,
            true,
        ));
        assert!(!should_hold_unmentioned_document(
            &AccessDenied::NotMentioned,
            false,
            true,
        ));
        assert!(!should_hold_unmentioned_document(
            &AccessDenied::NotMentioned,
            true,
            false,
        ));
        assert!(!should_hold_unmentioned_document(
            &AccessDenied::GroupNotOnAllowlist,
            true,
            true,
        ));
    }

    #[test]
    fn held_files_attach_to_the_next_message_and_then_clear() {
        let mut pending = PendingDocuments::default();
        pending.hold("-100", file("budget.xlsx"));
        pending.hold("-100", file("notes.xlsx"));

        let (body, documents) = merge_held_documents("read the file", None, pending.take("-100"));
        assert!(body.contains("budget.xlsx"));
        assert!(body.contains("notes.xlsx"));
        assert_eq!(documents.expect("files").len(), 2);
        assert!(pending.take("-100").is_empty());
    }

    #[test]
    fn a_chat_keeps_only_the_latest_held_files() {
        let mut pending = PendingDocuments::default();
        for index in 0..7 {
            pending.hold("chat", file(&format!("file-{index}.xlsx")));
        }
        let held = pending.take("chat");
        assert_eq!(held.len(), 5);
        assert_eq!(held[0].file.display_name, "file-2.xlsx");
    }
}
