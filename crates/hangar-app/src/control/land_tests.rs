//! The host's part of landing a delivery the person asked for: what the
//! window is told (a notice, the row's failure). The delivery itself is
//! hangar-engine's, tested there; this uses its delivery harness.

use serde_json::json;

use hangar_core::delivery::Kind;
use hangar_engine::control::testing::*;
use hangar_i18n::Locale;

use super::land;
use crate::message::{Intent, Message};
use crate::needs_you;
use crate::shell::Notice;

fn contains(text: &str, part: &str) {
    assert!(text.contains(part), "{text:?} does not contain {part:?}");
}

#[tokio::test]
async fn a_landing_the_person_asked_for_that_fails_says_so_on_its_row_and_in_a_notice() {
    let s = setup();
    let said = land(s.ws(), "t1".into(), s.port(), Locale::En).await;
    let [
        Message::Intent(Intent::Notice(Notice::Problem(notice))),
        Message::NeedsYou(needs_you::Msg::LandFailed { key, error }),
    ] = said.as_slice()
    else {
        panic!("a notice and the row's failure, not {said:?}");
    };
    assert_eq!(key, "task:C:\\dev\\app:t1:ready-to-land");
    contains(
        error,
        "Nothing of \"Add the export button\" waits to land any more",
    );
    contains(notice, error);
    // The landing held the workspace awake while it ran, and let go: the
    // person's hold and the delivery's own end alike.
    let begun = s.fake.logged("begin:");
    assert!(!begun.is_empty());
    assert_eq!(s.fake.logged("end:").len(), begun.len());
}

#[tokio::test]
async fn a_landing_the_person_asked_for_that_lands_only_says_so() {
    let s = setup();
    s.settings(|settings| settings.auto_merge = false);
    s.now(Kind::Ready, "passed, it waits.", |answer| {
        answer.landed_in = None
    });
    s.submit(json!({})).await.unwrap();
    s.fake.settle().await;
    s.now(Kind::Landed, "landed", |_| {});
    let said = land(s.ws(), "t1".into(), s.port(), Locale::En).await;
    let [Message::Intent(Intent::Notice(Notice::Agent(notice)))] = said.as_slice() else {
        panic!("a notice alone, not {said:?}");
    };
    contains(notice, "Add the export button");
}
