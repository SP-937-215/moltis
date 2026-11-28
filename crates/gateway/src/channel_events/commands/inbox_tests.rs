#![allow(clippy::unwrap_used)]

use super::inbox::inbox_report;

fn tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("files/home/0.Inbox")).unwrap();
    dir
}

#[test]
fn an_empty_inbox_with_no_activity_says_so() {
    let dir = tree();
    assert_eq!(
        inbox_report(dir.path()),
        "📥 0.Inbox is empty. No host activity recorded."
    );
}

#[test]
fn the_report_lists_waiting_files_and_recent_host_moves() {
    let dir = tree();
    let inbox = dir.path().join("files/home/0.Inbox");
    std::fs::write(inbox.join("scan.pdf"), b"x").unwrap();
    std::fs::write(inbox.join(".hidden"), b"x").unwrap();
    std::fs::write(
        dir.path().join("inbox-activity.log"),
        "2026-11-28T10:00:00+0800 quentin mirror copied scan.pdf\n\
         2026-11-28T11:00:00+0800 quentin reconcile filed 2.Areas/finance/20261128-a.pdf\n\
         2026-11-28T11:05:00+0800 quentin notify failed rc=1\n\
         Traceback (most recent call last):\n",
    )
    .unwrap();

    let report = inbox_report(dir.path());

    assert!(report.contains("📥 0.Inbox: 1 file"));
    assert!(report.contains("• scan.pdf"));
    assert!(!report.contains(".hidden"));
    assert!(report.contains("2026-11-28T11:00 reconcile filed 2.Areas/finance/20261128-a.pdf"));
    assert!(report.contains("2026-11-28T11:05 notify failed rc=1"));
    assert!(report.contains("2026-11-28T10:00 mirror copied scan.pdf"));
    assert!(!report.contains("Traceback"));
}

#[test]
fn only_the_latest_activity_is_shown() {
    let dir = tree();
    let lines: String = (0..40)
        .map(|n| format!("2026-11-28T10:{n:02}:00+0800 quentin reconcile filed f{n}.pdf\n"))
        .collect();
    std::fs::write(dir.path().join("inbox-activity.log"), lines).unwrap();

    let report = inbox_report(dir.path());

    assert!(report.contains("f39.pdf"));
    assert!(!report.contains("f24.pdf"));
}
