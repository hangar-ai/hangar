//! One repository on several machines: the spelling that makes two
//! addresses one, which workspaces count as its other checkouts, and what
//! is asked of whom to know.

use std::time::Instant;

use hangar_i18n::{Locale, Translator};

use super::*;
use crate::nodes::hosted::{self, OwnState};
use crate::nodes::testing::{machine, up};
use crate::state::AppState;
use crate::state::types::MachineKind;

#[test]
fn two_spellings_of_an_address_are_one_repository() {
    let one = "github.com/acme/shop";
    for spelled in [
        "git@github.com:Acme/Shop.git",
        "https://github.com/acme/shop",
        "https://GitHub.com/acme/shop/",
        "https://github.com/acme/shop.git/",
        "https://me@github.com/acme/shop.GIT",
        "ssh://git@github.com/acme/shop.git",
        "ssh://git@github.com:22/acme/shop",
        "git://github.com/acme/shop.git",
        "  https://github.com/acme/shop\n",
    ] {
        assert_eq!(identity(spelled).as_deref(), Some(one), "{spelled}");
    }
    assert_eq!(
        identity("https://gitlab.example:8443/Group/Sub/Shop.git").as_deref(),
        Some("gitlab.example/group/sub/shop"),
        "a path of any depth, the port left out"
    );
    assert_ne!(
        identity("git@github.com:acme/shop.git"),
        identity("git@gitlab.com:acme/shop.git"),
        "another host is another repository"
    );
    assert_ne!(
        identity("git@github.com:acme/shop.git"),
        identity("git@github.com:acme/shop-api.git")
    );
}

#[test]
fn an_address_without_a_host_is_no_identity() {
    for local in [
        "",
        "   ",
        "/srv/git/shop.git",
        "../shop",
        "file:///srv/git/shop.git",
        "C:\\repos\\shop",
        "C:/repos/shop",
        "https://github.com/",
        "github.com",
    ] {
        assert_eq!(identity(local), None, "{local:?}");
    }
}

fn ws(id: &str, name: &str, node: Option<&str>) -> Workspace {
    Workspace {
        id: id.into(),
        name: name.into(),
        cwd: format!("/home/me/{name}"),
        node: node.map(str::to_owned),
        hosted: node.map(|_| true),
        ..Workspace::default()
    }
}

/// `shop` here, `api` on vps and `render` on studio are one repository;
/// `notes` here has no origin, `site` on vps another one.
fn three_machines() -> Model {
    let mut state = AppState::default();
    state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
    state.settings.nodes = vec![
        machine("vps", "vps", MachineKind::Cloud),
        machine("studio", "studio", MachineKind::Tower),
    ];
    state.workspaces = vec![
        ws("w1", "shop", None),
        ws("w2", "notes", None),
        ws("w3", "api", Some("vps")),
        ws("w4", "site", Some("vps")),
        ws("w5", "render", Some("studio")),
    ];
    let mut model = Model::default();
    model.hydrate(state);
    up(&mut model, "vps", "vps-01");
    up(&mut model, "studio", "studio-01");
    let said = [
        ("w1", Some("git@github.com:Acme/Shop.git")),
        ("w2", None),
        ("w3", Some("https://github.com/acme/shop")),
        ("w4", Some("https://github.com/acme/site")),
        ("w5", Some("ssh://git@github.com/acme/shop.git")),
    ];
    for (id, origin) in said {
        let ws = model.workspace(id).unwrap().clone();
        model.live.nodes.repos.said(&ws, origin);
    }
    model
}

fn names(m: &Model, id: &str) -> Vec<String> {
    siblings(m, m.workspace(id).unwrap())
        .into_iter()
        .map(|ws| ws.name.clone())
        .collect()
}

#[test]
fn a_repository_open_here_and_on_two_nodes_is_found_from_each() {
    let m = three_machines();
    assert_eq!(names(&m, "w1"), ["api", "render"]);
    assert_eq!(names(&m, "w3"), ["shop", "render"]);
    assert_eq!(names(&m, "w5"), ["shop", "api"]);
    assert!(names(&m, "w4").is_empty(), "another repository");
    let tr = Translator::new(Locale::En);
    let shop = m.workspace("w1").unwrap();
    assert_eq!(
        also_on(&m, tr, shop).as_deref(),
        Some("Also on vps (api), studio (render)")
    );
    let api = m.workspace("w3").unwrap();
    assert_eq!(
        tip_with_others(&m, tr, api, "vps · connected".into()),
        "vps · connected · also on this computer (shop), studio (render)"
    );
    let site = m.workspace("w4").unwrap();
    assert_eq!(also_on(&m, tr, site), None);
    assert_eq!(
        tip_with_others(&m, tr, site, "vps · connected".into()),
        "vps · connected"
    );
    let fr = Translator::new(Locale::Fr);
    assert_eq!(
        tip_with_others(&m, fr, api, "vps · connecté".into()),
        "vps · connecté · aussi sur cet ordinateur (shop), studio (render)"
    );
}

#[test]
fn a_workspace_without_an_origin_is_never_grouped() {
    let mut m = three_machines();
    assert!(names(&m, "w2").is_empty());
    // Not with another that has none either.
    let other = m.workspace("w4").unwrap().clone();
    m.live.nodes.repos.said(&other, None);
    assert!(names(&m, "w2").is_empty());
    assert!(names(&m, "w4").is_empty());
    assert_eq!(elsewhere(&m, m.workspace("w2").unwrap()), None);
}

#[test]
fn two_checkouts_on_one_machine_are_not_siblings() {
    let mut m = three_machines();
    let site = m.workspace("w4").unwrap().clone();
    m.live
        .nodes
        .repos
        .said(&site, Some("git@github.com:acme/shop.git"));
    // `site` and `api` are both on vps: each sees the other machines only.
    assert_eq!(names(&m, "w4"), ["shop", "render"]);
    assert_eq!(names(&m, "w3"), ["shop", "render"]);
}

#[test]
fn a_node_switched_off_does_not_count() {
    let mut m = three_machines();
    let mut state = m.state().clone();
    state.settings.nodes[1].enabled = Some(false);
    m.hydrate(state);
    assert_eq!(names(&m, "w1"), ["api"], "studio is off, its rows hidden");
    assert!(names(&m, "w5").is_empty(), "and its own row says nothing");
    // Nor without the experiment: nothing of the machines shows then.
    let mut state = m.state().clone();
    state.settings.experiments.clear();
    m.hydrate(state);
    assert!(names(&m, "w1").is_empty());
}

#[test]
fn what_was_read_for_another_folder_is_not_believed() {
    let mut m = three_machines();
    let mut state = m.state().clone();
    state.workspaces[0].cwd = "/home/me/elsewhere".into();
    m.hydrate(state);
    assert!(names(&m, "w1").is_empty(), "its folder changed: not known");
    assert_eq!(names(&m, "w3"), ["render"]);
    // An answer for the old folder is dropped; one for the new is kept.
    let known = &mut m.live.nodes.repos;
    known.answered(
        "w1",
        "/home/me/shop",
        None,
        Some("git@github.com:acme/shop.git"),
    );
    assert!(names(&m, "w1").is_empty());
}

#[test]
fn the_next_machine_is_the_first_that_has_no_checkout() {
    let mut m = three_machines();
    // Open here, on vps and on studio: nowhere left.
    let shop = m.workspace("w1").unwrap().clone();
    assert_eq!(elsewhere(&m, &shop), Some(Elsewhere::Everywhere));
    // `site` is on vps only: this computer comes first.
    let site = m.workspace("w4").unwrap().clone();
    assert_eq!(
        elsewhere(&m, &site),
        Some(Elsewhere::Ready {
            repo: "https://github.com/acme/site".into(),
            place: None,
        })
    );
    // A repository of this computer goes to the first connected node.
    let notes = m.workspace("w2").unwrap().clone();
    m.live
        .nodes
        .repos
        .said(&notes, Some("git@github.com:me/notes.git"));
    assert_eq!(
        elsewhere(&m, &notes),
        Some(Elsewhere::Ready {
            repo: "git@github.com:me/notes.git".into(),
            place: Some("vps".into()),
        })
    );
    // A node that is not connected cannot be given it.
    m.live.nodes.statuses.remove("vps");
    assert_eq!(
        elsewhere(&m, &notes),
        Some(Elsewhere::Ready {
            repo: "git@github.com:me/notes.git".into(),
            place: Some("studio".into()),
        })
    );
}

/// What `refresh` asks for is marked, so that it is asked once; a mirror's
/// is its node's to say, with its state.
#[tokio::test]
async fn each_workspace_is_asked_once_and_a_mirror_never() {
    let mut state = three_machines().state().clone();
    // `site` is driven from here on vps.
    state.workspaces[3].hosted = None;
    let mut m = Model::default();
    m.hydrate(state);
    up(&mut m, "vps", "vps-01");
    let now = Instant::now();
    let asked = |m: &Model| {
        let mut ids: Vec<String> = m
            .live
            .nodes
            .repos
            .entries
            .iter()
            .filter(|(_, entry)| entry.asked)
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();
        ids
    };
    let _ = refresh(&mut m, now);
    assert_eq!(asked(&m), ["w1", "w2", "w4"], "the mirrors are not asked");
    let before = m.live.nodes.repos.entries.clone();
    let _ = refresh(&mut m, now);
    assert_eq!(m.live.nodes.repos.entries, before, "nothing is asked twice");

    let known = &mut m.live.nodes.repos;
    known.answered(
        "w1",
        "/home/me/shop",
        None,
        Some("git@github.com:Acme/Shop.git"),
    );
    known.answered("w2", "/home/me/notes", None, None);
    known.answered(
        "w4",
        "/home/me/site",
        Some("vps"),
        Some("https://github.com/acme/shop"),
    );
    assert!(asked(&m).is_empty());
    assert_eq!(names(&m, "w1"), ["site"]);

    // The window back in front after a while: this computer's are read
    // again, what was known standing meanwhile.
    m.live.nodes.repos.stale_local(now + LOCAL_FRESH);
    let _ = refresh(&mut m, now + LOCAL_FRESH);
    assert_eq!(asked(&m), ["w1", "w2"]);
    assert_eq!(names(&m, "w1"), ["site"]);
    // A node that reconnects is asked again for what is driven there.
    m.live.nodes.repos.stale_node("vps");
    let _ = refresh(&mut m, now + LOCAL_FRESH);
    assert_eq!(asked(&m), ["w1", "w2", "w4"]);

    // A workspace removed takes what was known of it.
    let mut state = m.state().clone();
    state.workspaces.remove(1);
    m.hydrate(state);
    let _ = refresh(&mut m, now + LOCAL_FRESH);
    assert!(!m.live.nodes.repos.entries.contains_key("w2"));
}

#[test]
fn a_node_says_the_origins_of_what_it_runs_with_its_state() {
    let value = serde_json::json!({
        "boot": "b1",
        "revision": 3,
        "workspaces": [{ "id": "o1", "name": "api", "cwd": "/srv/api", "panes": [] }],
        "activity": {},
        "origins": { "o1": "https://github.com/acme/shop.git" },
    });
    let own: OwnState = hosted::parse(&value).unwrap();
    assert_eq!(
        own.origins
            .as_ref()
            .and_then(|origins| origins.get("o1"))
            .map(String::as_str),
        Some("https://github.com/acme/shop.git")
    );
    assert!(
        own.workspaces[0].extra.is_empty(),
        "beside the workspaces, never one of their fields"
    );
    // An older node says nothing of them: what is known is left alone.
    let mut older = value.clone();
    older.as_object_mut().unwrap().remove("origins");
    assert_eq!(hosted::parse(&older).unwrap().origins, None);

    let mut m = three_machines();
    hosted::take_in(&mut m, "vps", own, 1_000);
    let api = m.workspace("o1").unwrap();
    assert!(api.is_hosted());
    assert_eq!(
        m.live
            .nodes
            .repos
            .origin(api)
            .map(|origin| origin.identity.as_str()),
        Some("github.com/acme/shop")
    );
    assert_eq!(names(&m, "w1"), ["render", "api"]);
}
