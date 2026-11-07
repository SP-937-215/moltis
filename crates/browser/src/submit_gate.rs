//! Host-checked gate for sending a form.
//!
//! The model can fill a form. It cannot send one until the person in the chat
//! confirms on a later message. A tool argument is not confirmation.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

/// Shown to the model when a send is refused. It must not retry in the same turn.
pub const SUBMIT_BLOCKED: &str = "Form send is blocked until the user confirms. \
Show the user exactly what will be sent, including every field you filled, then stop. \
Do not click send again in this turn. The user must reply with exactly \"submit\", \
\"send it\", \"send the form\", or \"confirm\".";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFacts {
    pub tag: String,
    pub input_type: Option<String>,
    pub text: Option<String>,
    pub aria_label: Option<String>,
    pub role: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateState {
    Idle,
    AwaitingConfirmation,
    Confirmed,
}

fn gates() -> &'static Mutex<HashMap<String, GateState>> {
    static GATES: OnceLock<Mutex<HashMap<String, GateState>>> = OnceLock::new();
    GATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock() -> std::sync::MutexGuard<'static, HashMap<String, GateState>> {
    gates()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Record the user's latest message. Confirmation only counts after a send was
/// already refused, and a later non-confirmation cancels it.
pub fn note_user_message(session_key: &str, text: &str) {
    if session_key.is_empty() {
        return;
    }
    let mut gates = lock();
    let state = gates
        .entry(session_key.to_string())
        .or_insert(GateState::Idle);
    *state = match *state {
        GateState::AwaitingConfirmation | GateState::Confirmed if is_submit_confirmation(text) => {
            GateState::Confirmed
        },
        GateState::AwaitingConfirmation | GateState::Confirmed => GateState::Idle,
        GateState::Idle => GateState::Idle,
    };
}

/// Allow one send, or refuse and arm the gate for the next user message.
pub fn authorize_submit(session_key: &str) -> Result<(), &'static str> {
    if session_key.is_empty() {
        return Err(SUBMIT_BLOCKED);
    }
    let mut gates = lock();
    let state = gates
        .entry(session_key.to_string())
        .or_insert(GateState::Idle);
    if *state == GateState::Confirmed {
        *state = GateState::Idle;
        Ok(())
    } else {
        *state = GateState::AwaitingConfirmation;
        Err(SUBMIT_BLOCKED)
    }
}

/// A control that would send the form. A bare `<button>` is `type=submit` in
/// the DOM, so the label decides; only `<input type="submit">` is a send
/// with no label. Links are navigation.
#[must_use]
pub fn is_submit_control(facts: &ControlFacts) -> bool {
    let tag = facts.tag.to_ascii_lowercase();
    let input_type = facts
        .input_type
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    if tag == "input" && input_type == "submit" {
        return true;
    }
    if tag == "a" {
        return false;
    }
    let role = facts.role.as_deref().unwrap_or("").to_ascii_lowercase();
    let is_button = tag == "button" || role == "button";
    if !is_button {
        return false;
    }
    let name = [
        facts.text.as_deref(),
        facts.aria_label.as_deref(),
        facts.value.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    submit_label(&name)
}

/// Enter in a single-line field submits the surrounding form.
#[must_use]
pub fn enter_would_submit(facts: &ControlFacts, text: &str) -> bool {
    if !text.chars().any(|ch| ch == '\n') {
        return false;
    }
    let tag = facts.tag.to_ascii_lowercase();
    if tag == "textarea" {
        return false;
    }
    let input_type = facts
        .input_type
        .as_deref()
        .unwrap_or("text")
        .to_ascii_lowercase();
    tag == "input"
        && matches!(
            input_type.as_str(),
            "text" | "email" | "password" | "search" | "tel" | "url" | "number" | ""
        )
}

/// JavaScript that sends a form, not a read of the page.
#[must_use]
pub fn evaluate_would_submit(code: &str) -> bool {
    let lower = code.to_ascii_lowercase();
    lower.contains(".submit(") || lower.contains("requestsubmit(")
}

/// A short later reply that means "send it". A first-message instruction does
/// not count; the gate only consults this after a send was refused.
#[must_use]
pub fn is_submit_confirmation(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 80 {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("don't")
        || lower.contains("do not")
        || lower.contains("dont")
        || lower.contains("not yet")
        || lower.contains("wait")
        || lower.contains("fill")
    {
        return false;
    }
    matches!(
        lower.as_str(),
        "submit"
            | "submit it"
            | "submit the form"
            | "send it"
            | "send the form"
            | "yes, submit"
            | "yes submit"
            | "yes, send it"
            | "yes send it"
            | "confirm"
            | "confirm submit"
            | "please submit"
            | "ok submit"
            | "ok, submit"
    )
}

fn submit_label(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    let name = name.trim_end_matches(|ch: char| matches!(ch, '.' | '!' | '…'));
    if matches!(
        name,
        "submit"
            | "submit application"
            | "submit form"
            | "send"
            | "send application"
            | "send form"
            | "send message"
            | "apply"
            | "apply now"
            | "place order"
            | "pay"
            | "pay now"
            | "confirm and submit"
            | "confirm and send"
            | "confirm and pay"
    ) {
        return true;
    }
    name.split_whitespace().any(|word| word == "submit")
}

#[cfg(test)]
mod tests {
    use super::{
        ControlFacts, authorize_submit, enter_would_submit, evaluate_would_submit,
        is_submit_confirmation, is_submit_control, note_user_message,
    };

    fn facts(
        tag: &str,
        input_type: Option<&str>,
        text: Option<&str>,
        role: Option<&str>,
    ) -> ControlFacts {
        ControlFacts {
            tag: tag.to_string(),
            input_type: input_type.map(str::to_string),
            text: text.map(str::to_string),
            aria_label: None,
            role: role.map(str::to_string),
            value: None,
        }
    }

    #[test]
    fn a_submit_button_needs_confirmation_and_a_link_does_not() {
        assert!(is_submit_control(&facts(
            "button",
            Some("submit"),
            Some("Apply now"),
            None
        )));
        assert!(is_submit_control(&facts(
            "button",
            Some("button"),
            Some("Submit application"),
            None
        )));
        assert!(!is_submit_control(&facts(
            "a",
            None,
            Some("Apply"),
            Some("link")
        )));
        assert!(!is_submit_control(&facts(
            "button",
            Some("submit"),
            Some("Next"),
            None
        )));
        assert!(is_submit_control(&facts(
            "input",
            Some("submit"),
            None,
            None
        )));
    }

    #[test]
    fn enter_in_a_text_field_sends_and_a_textarea_does_not() {
        let field = facts("input", Some("email"), None, None);
        assert!(enter_would_submit(&field, "ada@example.com\n"));
        assert!(!enter_would_submit(&field, "ada@example.com"));
        assert!(!enter_would_submit(
            &facts("textarea", Some("textarea"), None, None),
            "line\n"
        ));
    }

    #[test]
    fn only_form_submit_javascript_is_a_send() {
        assert!(evaluate_would_submit("document.forms[0].submit()"));
        assert!(evaluate_would_submit("form.requestSubmit()"));
        assert!(!evaluate_would_submit("document.title"));
    }

    #[test]
    fn the_first_send_is_refused_until_a_later_confirmation() {
        let session = "submit-gate-first-send";
        note_user_message(session, "fill this form and submit it");
        assert!(authorize_submit(session).is_err());
        note_user_message(session, "yes");
        assert!(authorize_submit(session).is_err());
        note_user_message(session, "submit");
        assert!(authorize_submit(session).is_ok());
        assert!(authorize_submit(session).is_err());
    }

    #[test]
    fn a_change_request_cancels_a_pending_confirmation() {
        let session = "submit-gate-cancel";
        assert!(authorize_submit(session).is_err());
        note_user_message(session, "change the phone number");
        assert!(authorize_submit(session).is_err());
        assert!(is_submit_confirmation("submit"));
        assert!(!is_submit_confirmation("don't submit"));
    }
}
