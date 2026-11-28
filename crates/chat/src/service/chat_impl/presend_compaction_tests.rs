#![allow(clippy::expect_used)]

use super::presend_compaction::{
    SKIP_MEMORY_TURN_PARAM, compact_params, presend_compaction, runs_silent_memory_turn,
};

#[test]
fn a_full_private_session_compacts_with_the_memory_turn() {
    let params = presend_compaction(true, false, 900, 800).map(|skip| compact_params("s", skip));
    let params = params.expect("compacts");
    assert_eq!(params["_session_key"], "s");
    assert!(runs_silent_memory_turn(&params));
}

#[test]
fn a_full_group_session_compacts_without_the_memory_turn() {
    // Group and queued channel turns carry no private context. They used to
    // skip compaction entirely and only recover after an overflow error.
    let params = presend_compaction(false, false, 900, 800)
        .map(|skip| compact_params("s", skip))
        .expect("compacts");
    assert_eq!(params[SKIP_MEMORY_TURN_PARAM], true);
    assert!(!runs_silent_memory_turn(&params));
}

#[test]
fn below_the_threshold_or_ephemeral_does_not_compact() {
    assert!(presend_compaction(true, false, 700, 800).is_none());
    assert!(presend_compaction(false, true, 900, 800).is_none());
}

#[test]
fn an_explicit_compact_request_runs_the_memory_turn() {
    assert!(runs_silent_memory_turn(
        &serde_json::json!({"_session_key": "s"})
    ));
}
