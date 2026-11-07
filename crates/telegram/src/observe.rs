//! A group message that names a person is not a request to the bot.

/// True when the message @mentions someone other than this bot, and does not
/// @mention the bot. The bot should keep the text and not start a reply.
#[must_use]
pub fn group_turn_is_for_someone_else(text: &str, bot_username: Option<&str>) -> bool {
    let mentions = mentions(text);
    if mentions.is_empty() {
        return false;
    }
    let bot = bot_username.unwrap_or("").trim_start_matches('@');
    if bot.is_empty() {
        return true;
    }
    !mentions
        .iter()
        .any(|mention| mention.eq_ignore_ascii_case(bot))
}

fn mentions(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'@' {
            let preceded_by_word = index > 0 && bytes[index - 1].is_ascii_alphanumeric();
            if !preceded_by_word {
                let start = index + 1;
                let mut end = start;
                while end < bytes.len()
                    && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_')
                {
                    end += 1;
                }
                if end > start {
                    found.push(text[start..end].to_string());
                    index = end;
                    continue;
                }
            }
        }
        index += 1;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::group_turn_is_for_someone_else;

    #[test]
    fn a_human_mention_is_not_a_request_to_the_bot() {
        assert!(group_turn_is_for_someone_else(
            "@carrie the school called",
            Some("gYPf6S1x_bot")
        ));
        assert!(group_turn_is_for_someone_else(
            "please look @Quentin",
            Some("@gYPf6S1x_bot")
        ));
    }

    #[test]
    fn addressing_the_bot_still_starts_a_reply() {
        assert!(!group_turn_is_for_someone_else(
            "@gYPf6S1x_bot what is this form",
            Some("gYPf6S1x_bot")
        ));
        assert!(!group_turn_is_for_someone_else(
            "@carrie @gYPf6S1x_bot can you fill this",
            Some("gYPf6S1x_bot")
        ));
        assert!(!group_turn_is_for_someone_else(
            "please fill this form",
            Some("gYPf6S1x_bot")
        ));
    }

    #[test]
    fn an_email_address_is_not_a_mention() {
        assert!(!group_turn_is_for_someone_else(
            "write to ada@example.com",
            Some("gYPf6S1x_bot")
        ));
    }
}
