//! `/inbox`: what is waiting in this bot's inbox folder, and what the host
//! inbox pipeline did recently.
//!
//! The NAS supervisor files uploads out of `files/home/0.Inbox` and appends
//! one stamped line per action to `inbox-activity.log` in the data dir. This
//! only reads those two paths, under this process's own data dir.

use std::path::Path;

const INBOX_DIR: &str = "files/home/0.Inbox";
const ACTIVITY_LOG: &str = "inbox-activity.log";
const MAX_FILES: usize = 20;
const MAX_ACTIVITY: usize = 15;

#[must_use]
pub(super) fn inbox_report(data_dir: &Path) -> String {
    let files = waiting_files(&data_dir.join(INBOX_DIR));
    let activity = recent_activity(&data_dir.join(ACTIVITY_LOG));
    if files.is_empty() && activity.is_empty() {
        return "📥 0.Inbox is empty. No host activity recorded.".to_string();
    }

    let mut lines = Vec::new();
    if files.is_empty() {
        lines.push("📥 0.Inbox is empty.".to_string());
    } else {
        let noun = if files.len() == 1 {
            "file"
        } else {
            "files"
        };
        lines.push(format!("📥 0.Inbox: {} {noun}", files.len()));
        lines.extend(files.iter().take(MAX_FILES).map(|name| format!("• {name}")));
        if files.len() > MAX_FILES {
            lines.push(format!("… and {} more", files.len() - MAX_FILES));
        }
    }
    if !activity.is_empty() {
        lines.push(String::new());
        lines.push("Recent host activity:".to_string());
        lines.extend(activity.iter().map(|line| format!("• {line}")));
    }
    lines.join("\n")
}

fn waiting_files(inbox: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(inbox) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    names
}

/// The newest stamped lines, newest first, as `YYYY-MM-DDTHH:MM <step> <detail>`.
fn recent_activity(log: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(log) else {
        return Vec::new();
    };
    text.lines()
        .rev()
        .filter_map(format_activity)
        .take(MAX_ACTIVITY)
        .collect()
}

/// `<rfc3339 stamp> <tree> <step> <detail>` → `<minute> <step> <detail>`.
fn format_activity(line: &str) -> Option<String> {
    let mut parts = line.splitn(3, ' ');
    let stamp = parts.next()?;
    let _tree = parts.next()?;
    let rest = parts.next()?.trim();
    let minute = stamp.get(..16)?;
    let looks_stamped = minute.as_bytes().get(10) == Some(&b'T')
        && minute
            .get(..4)
            .is_some_and(|year| year.bytes().all(|b| b.is_ascii_digit()));
    (looks_stamped && !rest.is_empty()).then(|| format!("{minute} {rest}"))
}
