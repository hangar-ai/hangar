//! A Discord of one's own, to run a Hangar's bot against with nothing
//! leaving the machine (`tests/support/fake.rs`).
//!
//! ```sh
//! cargo run -p hangar-discord --example fake_discord -- 7777
//! # then, in a debug build of Hangar:
//! HANGAR_DISCORD_API=http://127.0.0.1:7777/api \
//! HANGAR_DISCORD_GATEWAY=ws://127.0.0.1:7777/gateway hangar-ai
//! curl -s 127.0.0.1:7777/fake/calls                 # what the bot did
//! curl -s -XPOST 127.0.0.1:7777/fake/send -d '{"t":"MESSAGE_CREATE","d":{…}}'
//! ```

#[path = "../tests/support/fake.rs"]
mod fake;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    let discord = fake::Discord::start_on(port, std::time::Duration::ZERO).await;
    println!("fake Discord on {}", discord.base);
    std::future::pending::<()>().await;
}
