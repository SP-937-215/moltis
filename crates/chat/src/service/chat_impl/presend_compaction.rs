//! Whether to compact a session before sending, and with what scope.
//!
//! `compact()` normally starts with a silent memory turn that writes the
//! owner's memory files. A turn without private context (a group chat, or a
//! queued channel turn that was downgraded) must not do that, so it used to
//! skip pre-send compaction entirely and only recovered after an overflow
//! error. It now compacts the history alone.

use serde_json::Value;

/// `chat.compact` param: compact the history without the silent memory turn.
/// Setting it can only make a compaction do less, so callers need no privilege.
pub(super) const SKIP_MEMORY_TURN_PARAM: &str = "_skip_memory_turn";

/// `Some(skip_memory_turn)` when the next request should compact first.
#[must_use]
pub(super) fn presend_compaction(
    private_context: bool,
    ephemeral: bool,
    estimated_next_input: u64,
    threshold: u64,
) -> Option<bool> {
    (!ephemeral && estimated_next_input >= threshold).then_some(!private_context)
}

#[must_use]
pub(super) fn compact_params(session_key: &str, skip_memory_turn: bool) -> Value {
    let mut params = serde_json::json!({ "_session_key": session_key });
    if skip_memory_turn {
        params[SKIP_MEMORY_TURN_PARAM] = Value::Bool(true);
    }
    params
}

#[must_use]
pub(super) fn runs_silent_memory_turn(params: &Value) -> bool {
    params.get(SKIP_MEMORY_TURN_PARAM).and_then(Value::as_bool) != Some(true)
}
