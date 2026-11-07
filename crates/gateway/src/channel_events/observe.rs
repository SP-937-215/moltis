use std::sync::Arc;

use {
    moltis_channels::{ChannelMessageMeta, ChannelReplyTarget},
    moltis_sessions::PersistedMessage,
    tracing::{debug, warn},
};

use crate::state::GatewayState;

use super::{default_channel_session_key, resolve_channel_session};

/// Append a group aside to the channel session and do not start a model turn.
pub(in crate::channel_events) async fn record_overheard(
    state: &Arc<tokio::sync::OnceCell<Arc<GatewayState>>>,
    text: &str,
    reply_to: &ChannelReplyTarget,
    meta: &ChannelMessageMeta,
) {
    let Some(state) = state.get() else {
        return;
    };
    let session_key = if let Some(ref metadata) = state.services.session_metadata {
        resolve_channel_session(reply_to, metadata).await
    } else {
        default_channel_session_key(reply_to)
    };
    let Some(store) = state.services.session_store.as_ref() else {
        warn!("overheard message dropped: session store is unavailable");
        return;
    };
    let sender = meta
        .sender_name
        .as_deref()
        .or(meta.username.as_deref())
        .unwrap_or("someone");
    let message = PersistedMessage::overheard(sender, text);
    if let Err(error) = store.append(&session_key, &message.to_value()).await {
        warn!(%error, "failed to record overheard group message");
        return;
    }
    if let Some(ref metadata) = state.services.session_metadata {
        let _ = metadata.upsert(&session_key, None).await;
    }
    debug!(session = %session_key, "recorded overheard group message without a reply");
}
