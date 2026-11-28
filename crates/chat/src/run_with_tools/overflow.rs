//! When a run that overflowed the context window may be compacted and retried.

use moltis_agents::AgentRunError;

/// Whether an overflowed run should compact its session and retry once.
///
/// This does not depend on the turn's audience. The retry compacts with
/// `compact_session`, which only rewrites this session's own history and
/// runs no silent memory turn, so a public or downgraded queued turn gains
/// no private context from it. Gating it on private context left a session
/// that hit the limit during a queued burst failing every later turn.
#[must_use]
pub(crate) fn compacts_on_overflow(error: &AgentRunError, has_session_store: bool) -> bool {
    has_session_store && matches!(error, AgentRunError::ContextWindowExceeded(_))
}
