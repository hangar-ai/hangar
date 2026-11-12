//! What both hosts of the connector share around it: the bot kept running,
//! and what its setup tells the person (the invite link, from the token).

use std::sync::Arc;
use std::time::Duration;

use crate::{Hangar, Options};

/// The longest wait between two tries of a bot that stopped.
const RETRY_MAX: Duration = Duration::from_secs(300);

/// The bot, tried again after a while when it stops (Discord out of reach,
/// the network cut, a token refused until the person fixes it), until the
/// task is aborted.
pub async fn keep_running(hangar: Arc<dyn Hangar>, options: Options) {
    let mut wait = Duration::from_secs(5);
    loop {
        match crate::run(hangar.clone(), options.clone()).await {
            Ok(()) => return,
            Err(why) => eprintln!("discord: {why}; trying again in {}s", wait.as_secs()),
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(RETRY_MAX);
    }
}

/// The bot's user id, read from its token: the token's first part is that
/// id in base64. An application's bot shares its id, which the invite link
/// names.
pub fn bot_id(token: &str) -> Option<String> {
    let first = token.trim().split('.').next()?;
    let bytes = base64_decode(first)?;
    let id = String::from_utf8(bytes).ok()?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then_some(id)
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for c in text.bytes().filter(|c| *c != b'=') {
        buffer = (buffer << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The link that adds the bot to a server, with the scopes and permissions
/// it needs (discord-connector.md, "Setup and security").
pub fn invite(bot: &str) -> String {
    format!(
        "https://discord.com/oauth2/authorize?client_id={bot}&scope=bot%20applications.commands&permissions={}",
        crate::bot::BOT_ALLOW
    )
}

/// A server's id is digits.
pub fn is_guild_id(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bot_s_id_is_read_from_its_token_and_names_the_invite() {
        // "123456789012345678" in base64, as Discord writes it.
        let token = "MTIzNDU2Nzg5MDEyMzQ1Njc4.GAbcde.xyz";
        assert_eq!(bot_id(token).as_deref(), Some("123456789012345678"));
        assert_eq!(bot_id("not-a-token"), None);
        assert_eq!(bot_id(""), None);
        let link = invite("123");
        assert!(link.contains("client_id=123"));
        assert!(link.contains("scope=bot%20applications.commands"));
        assert!(is_guild_id(" 42 "));
        assert!(!is_guild_id("my-server"));
    }
}
