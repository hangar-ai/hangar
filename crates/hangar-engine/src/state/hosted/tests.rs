//! A node's store and a desktop's side by side: what the node says is
//! merged into the mirror, what the person does on the mirror is sent back
//! as ops, and neither undoes the other.

use serde_json::json;

use super::*;
use crate::state::layout::move_leaf;
use crate::state::store::{CommandScope, SavedCommandDraft, WorkspaceDraft};
use crate::state::types::{AppState, SplitDir, ViewPane, WindowBounds, WsId};

const NODE: &str = "n1";

fn store() -> Store {
    let mut store = Store::default();
    store.hydrate(AppState::default());
    store
}

/// A node's store with one workspace of `layout` Claude Code panes.
fn node_with(layout: u8) -> (Store, WsId) {
    let mut node = store();
    let ws = node
        .apply(StoreAction::AddWorkspace(WorkspaceDraft {
            name: "Site".into(),
            cwd: "/srv/site".into(),
            layout,
            agents: vec![AgentId::Claude; usize::from(layout)],
            ..WorkspaceDraft::default()
        }))
        .workspace
        .unwrap();
    (node, ws)
}

/// The desktop's store told what `node` runs.
fn sync(desktop: &mut Store, node: &Store) -> StoreOutcome {
    desktop.apply(StoreAction::SyncHosted {
        node: NODE.into(),
        workspaces: node.state().workspaces.clone(),
        keep: Vec::new(),
    })
}

/// A desktop mirroring `node`.
fn mirroring(node: &Store) -> Store {
    let mut desktop = store();
    sync(&mut desktop, node);
    desktop.take_respawns();
    desktop
}

fn ws<'a>(store: &'a Store, id: &str) -> &'a Workspace {
    store.workspace(id).unwrap()
}

fn pane_ids(store: &Store, id: &str) -> Vec<PaneId> {
    ws(store, id).panes.iter().map(|p| p.id.clone()).collect()
}

/// `action` applied to the mirror as the person would, and the ops that
/// tell the node of it.
fn ops_of(desktop: &mut Store, id: &str, action: StoreAction) -> Vec<HostedOp> {
    let before = ws(desktop, id).clone();
    desktop.apply(action);
    let ops = diff(&before, desktop.workspace(id), desktop.respawns());
    desktop.take_respawns();
    ops
}

fn update_pane(id: &str, pane: &str, edit: fn(&mut Pane)) -> StoreAction {
    StoreAction::UpdatePane {
        ws: id.into(),
        pane: pane.into(),
        patch: PanePatch::new(edit),
    }
}

#[test]
fn an_op_is_tagged_json_and_a_patch_names_only_what_it_sets() {
    let patch = PaneFields {
        title: Some(Some("Build".into())),
        session_id: Some(None),
        ..PaneFields::default()
    };
    let op = HostedOp::PatchPane {
        pane: "p1".into(),
        patch: patch.clone(),
    };
    let written = serde_json::to_value(&op).unwrap();
    assert_eq!(
        written,
        json!({ "op": "patchPane", "pane": "p1", "patch": { "title": "Build", "sessionId": null } })
    );
    assert_eq!(serde_json::from_value::<HostedOp>(written).unwrap(), op);
    // A field left out is not named; `null` empties one.
    let read: PaneFields = serde_json::from_value(json!({ "sessionId": null })).unwrap();
    assert_eq!(read.session_id, Some(None));
    assert_eq!(read.title, None);

    let shapes = [
        (
            HostedOp::ClosePane { pane: "p1".into() },
            json!({ "op": "closePane", "pane": "p1" }),
        ),
        (
            HostedOp::RespawnPane {
                from: "p1".into(),
                to: "p2".into(),
                patch: PaneFields::default(),
            },
            json!({ "op": "respawnPane", "from": "p1", "to": "p2" }),
        ),
        (
            HostedOp::PatchWorkspace {
                patch: WorkspaceFields {
                    name: Some("Shop".into()),
                    extra_roots: Some(vec!["/srv/lib".into()]),
                    claude_account: Some(None),
                    ..WorkspaceFields::default()
                },
            },
            json!({ "op": "patchWorkspace", "patch": {
                "name": "Shop", "extraRoots": ["/srv/lib"], "claudeAccount": null
            } }),
        ),
        (
            HostedOp::RemoveWorkspace,
            json!({ "op": "removeWorkspace" }),
        ),
    ];
    for (op, shape) in shapes {
        assert_eq!(serde_json::to_value(&op).unwrap(), shape);
        assert_eq!(serde_json::from_value::<HostedOp>(shape).unwrap(), op);
    }
    let put = serde_json::to_value(HostedOp::PutPane {
        pane: Pane {
            id: "p9".into(),
            name: "Ava".into(),
            agent: AgentId::Claude,
            ..Pane::default()
        },
    })
    .unwrap();
    assert_eq!(put["op"], "putPane");
    assert_eq!(put["pane"]["id"], "p9");
    assert_eq!(put["pane"]["agent"], "claude");
}

#[test]
fn a_new_mirror_takes_what_the_node_owns_and_its_layout_alone() {
    let (mut node, id) = node_with(2);
    node.apply(StoreAction::UpdateWorkspace(
        id.clone(),
        WorkspacePatch::new(|ws: &mut Workspace| {
            ws.theme_id = Some("paper".into());
            ws.folder_id = Some("f".into());
            ws.chat_view = Some(true);
        }),
    ));
    let theirs = ws(&node, &id);
    let mirror = mirror_of(NODE, theirs);
    assert_eq!(mirror.id, id);
    assert_eq!(mirror.node.as_deref(), Some(NODE));
    assert!(mirror.is_hosted());
    assert_eq!(mirror.name, "Site");
    assert_eq!(mirror.cwd, "/srv/site");
    assert_eq!(mirror.panes, theirs.panes);
    assert_eq!(mirror.tree, theirs.tree);
    assert_eq!(mirror.theme_id, None);
    assert_eq!(mirror.folder_id, None);
    assert_eq!(mirror.chat_view, None);
}

#[test]
fn a_merge_replaces_what_the_node_owns_and_keeps_the_presentation() {
    let (mut node, id) = node_with(2);
    let mut mirror = mirror_of(NODE, ws(&node, &id));
    let panes = pane_ids(&node, &id);
    let tree = move_leaf(
        mirror.tree.as_ref().unwrap(),
        &panes[0],
        &panes[1],
        DropZone::Bottom,
    );
    mirror.tree = Some(tree.clone());
    mirror.folder_id = Some("work".into());
    mirror.chat_view = Some(true);
    mirror.theme_id = Some("paper".into());
    mirror.detached = Some(true);
    mirror.window_bounds = Some(WindowBounds {
        x: 1.0,
        y: 2.0,
        width: 800.0,
        height: 600.0,
    });
    mirror.mobile_ports = Some(vec![3000]);

    node.apply(StoreAction::UpdateWorkspace(
        id.clone(),
        WorkspacePatch::new(|ws: &mut Workspace| {
            ws.name = "Shop".into();
            ws.hq = Some(true);
            ws.theme_id = Some("node's own".into());
        }),
    ));
    node.apply(update_pane(&id, &panes[0], |pane| {
        pane.session_id = Some("s-1".into());
        pane.title = Some("Build".into());
    }));
    let (merged, respawns) = merge(&mirror, ws(&node, &id));
    assert!(respawns.is_empty());
    assert_eq!(merged.name, "Shop");
    assert_eq!(merged.hq, Some(true));
    assert_eq!(merged.panes, ws(&node, &id).panes);
    let kept = Workspace {
        name: mirror.name.clone(),
        hq: None,
        panes: mirror.panes.clone(),
        ..merged.clone()
    };
    assert_eq!(kept, mirror, "everything else is the mirror's, as it was");
    assert_eq!(merged.tree, Some(tree));
}

#[test]
fn a_merge_turns_a_workspace_this_hangar_drove_into_the_mirror() {
    let (node, id) = node_with(1);
    let driven = Workspace {
        node: Some(NODE.into()),
        hosted: None,
        folder_id: Some("work".into()),
        ..ws(&node, &id).clone()
    };
    let (merged, _) = merge(&driven, ws(&node, &id));
    assert!(merged.is_hosted());
    assert_eq!(merged.folder_id.as_deref(), Some("work"));
}

#[test]
fn a_pane_the_node_respawned_keeps_its_leaf_and_is_answered() {
    let (mut node, id) = node_with(3);
    let mirror = mirror_of(NODE, ws(&node, &id));
    let before = leaf_ids(mirror.tree.as_ref().unwrap());
    let old = before[1].clone();
    node.apply(update_pane(&id, &old, |pane| {
        pane.session_id = Some("s-1".into())
    }));
    let mirror = merge(&mirror, ws(&node, &id)).0;
    let new = node
        .apply(StoreAction::RespawnPane {
            ws: id.clone(),
            pane: old.clone(),
            patch: PanePatch::new(|pane: &mut Pane| pane.session_id = None),
        })
        .pane
        .unwrap();
    // The node's own layout is not what places it: only its name does.
    node.apply(StoreAction::TidyLayout(id.clone()));
    let (merged, respawns) = merge(&mirror, ws(&node, &id));
    let mut expected = before.clone();
    expected[1] = new.clone();
    assert_eq!(leaf_ids(merged.tree.as_ref().unwrap()), expected);
    assert_eq!(
        respawns,
        [Respawn {
            ws: id.clone(),
            from: old,
            to: new,
            keeps_session: false,
        }]
    );
}

#[test]
fn a_pane_that_appeared_lands_beside_the_last_and_one_gone_leaves_its_room() {
    let (mut node, id) = node_with(2);
    let mut mirror = mirror_of(NODE, ws(&node, &id));
    mirror.view_panes.push(ViewPane::new("view"));
    let panes = pane_ids(&node, &id);
    mirror.tree = Some(insert_leaf(
        mirror.tree.as_ref().unwrap(),
        &panes[0],
        "view",
        DropZone::Bottom,
    ));
    let added = node
        .apply(StoreAction::AddPane {
            ws: id.clone(),
            agent: None,
            near: None,
            dir: None,
            cwd: None,
            name: None,
            init: None,
        })
        .pane
        .unwrap();
    node.apply(StoreAction::ClosePane {
        ws: id.clone(),
        pane: panes[1].clone(),
    });
    let (merged, respawns) = merge(&mirror, ws(&node, &id));
    assert!(respawns.is_empty(), "another name: another pane");
    assert_eq!(
        leaf_ids(merged.tree.as_ref().unwrap()),
        [panes[0].clone(), "view".to_string(), added]
    );
    assert_eq!(merged.view_panes, mirror.view_panes);
}

#[test]
fn what_is_only_the_mirror_s_presentation_sends_the_node_nothing() {
    let (node, id) = node_with(2);
    let mut desktop = mirroring(&node);
    let panes = pane_ids(&desktop, &id);
    let folder = desktop
        .apply(StoreAction::AddFolder("Work".into()))
        .folder
        .unwrap();
    let quiet = [
        StoreAction::UpdateWorkspace(
            id.clone(),
            WorkspacePatch::new(|ws: &mut Workspace| {
                ws.detached = Some(true);
                ws.window_bounds = Some(WindowBounds::default());
                ws.theme_id = Some("paper".into());
                ws.mobile_ports = Some(vec![5173]);
            }),
        ),
        StoreAction::MovePane {
            ws: id.clone(),
            drag: panes[0].clone(),
            target: panes[1].clone(),
            zone: DropZone::Bottom,
        },
        StoreAction::TidyLayout(id.clone()),
        StoreAction::SetChatView {
            ws: id.clone(),
            on: true,
        },
        StoreAction::MoveWorkspace {
            id: id.clone(),
            folder: Some(folder),
            before: None,
        },
        StoreAction::AddView {
            ws: id.clone(),
            id: "view".into(),
            near: None,
            dir: None,
        },
        StoreAction::AddSavedCommand {
            scope: CommandScope::Workspace,
            ws: id.clone(),
            draft: SavedCommandDraft::default(),
        },
    ];
    for action in quiet {
        let said = format!("{action:?}");
        let before = desktop.revision();
        assert_eq!(ops_of(&mut desktop, &id, action), [], "{said}");
        assert_ne!(desktop.revision(), before, "{said} changed the mirror");
    }
}

#[test]
fn each_change_to_what_the_node_owns_is_the_op_that_says_it() {
    let (node, id) = node_with(2);
    let mut desktop = mirroring(&node);
    let panes = pane_ids(&desktop, &id);

    let rename = StoreAction::UpdateWorkspace(
        id.clone(),
        WorkspacePatch::new(|ws: &mut Workspace| ws.name = "Shop".into()),
    );
    assert_eq!(
        ops_of(&mut desktop, &id, rename),
        [HostedOp::PatchWorkspace {
            patch: WorkspaceFields {
                name: Some("Shop".into()),
                ..WorkspaceFields::default()
            }
        }]
    );

    let titled = update_pane(&id, &panes[0], |pane| pane.title = Some("Build".into()));
    assert_eq!(
        ops_of(&mut desktop, &id, titled),
        [HostedOp::PatchPane {
            pane: panes[0].clone(),
            patch: PaneFields {
                title: Some(Some("Build".into())),
                ..PaneFields::default()
            }
        }]
    );

    let split = StoreAction::AddPane {
        ws: id.clone(),
        agent: Some(AgentId::Shell),
        near: Some(panes[0].clone()),
        dir: Some(SplitDir::Col),
        cwd: None,
        name: None,
        init: None,
    };
    let ops = ops_of(&mut desktop, &id, split);
    let added = ws(&desktop, &id).panes.last().unwrap().clone();
    assert_eq!(
        ops,
        [HostedOp::PutPane {
            pane: added.clone()
        }]
    );

    let close = StoreAction::ClosePane {
        ws: id.clone(),
        pane: added.id.clone(),
    };
    assert_eq!(
        ops_of(&mut desktop, &id, close),
        [HostedOp::ClosePane { pane: added.id }]
    );

    let fresh = StoreAction::RespawnPane {
        ws: id.clone(),
        pane: panes[1].clone(),
        patch: PanePatch::new(|pane: &mut Pane| pane.agent = AgentId::Shell),
    };
    let ops = ops_of(&mut desktop, &id, fresh);
    let to = ws(&desktop, &id).panes[1].id.clone();
    assert_ne!(to, panes[1]);
    assert_eq!(
        ops,
        [HostedOp::RespawnPane {
            from: panes[1].clone(),
            to,
            patch: PaneFields {
                agent: Some(AgentId::Shell),
                ..PaneFields::default()
            }
        }]
    );

    let remove = StoreAction::RemoveWorkspace(id.clone());
    assert_eq!(
        ops_of(&mut desktop, &id, remove),
        [HostedOp::RemoveWorkspace]
    );
}

#[test]
fn a_preset_is_the_panes_it_opens_then_those_it_closes() {
    let (node, id) = node_with(2);
    let mut desktop = mirroring(&node);
    let panes = pane_ids(&desktop, &id);
    let grow = StoreAction::ApplyPreset {
        ws: id.clone(),
        count: 4,
        model_pane: None,
    };
    let ops = ops_of(&mut desktop, &id, grow);
    let now = ws(&desktop, &id).panes.clone();
    assert_eq!(
        ops,
        [
            HostedOp::PutPane {
                pane: now[2].clone()
            },
            HostedOp::PutPane {
                pane: now[3].clone()
            },
        ]
    );
    let shrink = StoreAction::ApplyPreset {
        ws: id.clone(),
        count: 1,
        model_pane: None,
    };
    let ops = ops_of(&mut desktop, &id, shrink);
    let closed: Vec<HostedOp> = [&panes[1], &now[2].id, &now[3].id]
        .into_iter()
        .map(|pane| HostedOp::ClosePane { pane: pane.clone() })
        .collect();
    assert_eq!(ops, closed);
}

#[test]
fn a_pane_restarted_twice_before_the_node_hears_is_one_respawn() {
    let (node, id) = node_with(1);
    let mut desktop = mirroring(&node);
    let first = pane_ids(&desktop, &id)[0].clone();
    let before = ws(&desktop, &id).clone();
    let restart = |desktop: &mut Store, pane: &PaneId| {
        desktop
            .apply(StoreAction::RespawnPane {
                ws: id.clone(),
                pane: pane.clone(),
                patch: PanePatch::none(),
            })
            .pane
            .unwrap()
    };
    let second = restart(&mut desktop, &first);
    let third = restart(&mut desktop, &second);
    assert_eq!(
        diff(&before, desktop.workspace(&id), desktop.respawns()),
        [HostedOp::RespawnPane {
            from: first.clone(),
            to: third.clone(),
            patch: PaneFields::default(),
        }]
    );
    // Without the respawns it reads as a pane opened and another closed.
    let blind = diff(&before, desktop.workspace(&id), &[]);
    assert!(matches!(blind[0], HostedOp::PutPane { .. }), "{blind:?}");
    assert_eq!(blind[1], HostedOp::ClosePane { pane: first });
}

#[test]
fn an_op_applied_twice_changes_nothing_the_second_time() {
    let (mut node, id) = node_with(2);
    let panes = pane_ids(&node, &id);
    let ops = [
        HostedOp::PutPane {
            pane: Pane {
                id: "desk-1".into(),
                name: "Zed".into(),
                agent: AgentId::Claude,
                ..Pane::default()
            },
        },
        HostedOp::PatchPane {
            pane: "desk-1".into(),
            patch: PaneFields {
                model: Some(Some("opus".into())),
                ..PaneFields::default()
            },
        },
        HostedOp::RespawnPane {
            from: panes[0].clone(),
            to: "desk-2".into(),
            patch: PaneFields {
                session_id: Some(Some("s-9".into())),
                ..PaneFields::default()
            },
        },
        HostedOp::ClosePane {
            pane: panes[1].clone(),
        },
        HostedOp::PatchWorkspace {
            patch: WorkspaceFields {
                name: Some("Shop".into()),
                hq: Some(Some(true)),
                ..WorkspaceFields::default()
            },
        },
    ];
    for op in &ops {
        assert!(apply_op(&mut node, &id, op.clone()).changed, "{op:?}");
        let after = ws(&node, &id).clone();
        assert!(!apply_op(&mut node, &id, op.clone()).changed, "{op:?}");
        assert_eq!(*ws(&node, &id), after);
    }
    let now = ws(&node, &id);
    assert_eq!(now.name, "Shop");
    assert_eq!(pane_ids(&node, &id), ["desk-2", "desk-1"]);
    assert_eq!(now.panes[0].session_id.as_deref(), Some("s-9"));
    assert_eq!(now.panes[1].model.as_deref(), Some("opus"));
    assert_eq!(now.panes[1].name, "Zed");
    assert_eq!(leaf_ids(now.tree.as_ref().unwrap()), ["desk-2", "desk-1"]);

    assert!(apply_op(&mut node, &id, HostedOp::RemoveWorkspace).changed);
    assert!(!apply_op(&mut node, &id, HostedOp::RemoveWorkspace).changed);
    assert!(node.workspace(&id).is_none());
}

#[test]
fn the_node_keeps_its_panes_names_apart_whatever_a_mirror_asks() {
    let (mut node, id) = node_with(2);
    let panes = pane_ids(&node, &id);
    let taken = ws(&node, &id).panes[0].name.clone();
    apply_op(
        &mut node,
        &id,
        HostedOp::PutPane {
            pane: Pane {
                id: "desk-1".into(),
                name: taken.clone(),
                ..Pane::default()
            },
        },
    );
    let put = ws(&node, &id).panes[2].name.clone();
    assert!(!put.is_empty() && put != taken, "{put}");
    let renamed = HostedOp::PatchPane {
        pane: panes[1].clone(),
        patch: PaneFields {
            name: Some(taken.clone()),
            title: Some(Some("Docs".into())),
            ..PaneFields::default()
        },
    };
    apply_op(&mut node, &id, renamed);
    let pane = &ws(&node, &id).panes[1];
    assert_ne!(pane.name, taken, "the name is another pane's");
    assert_eq!(pane.title.as_deref(), Some("Docs"), "the rest is applied");
}

/// What the person does on the mirror, sent to the node and heard back
/// from it, leaves the mirror exactly as the person left it.
#[test]
fn a_mirror_s_own_changes_come_back_from_the_node_unchanged() {
    let (mut node, id) = node_with(2);
    let mut desktop = mirroring(&node);
    let panes = pane_ids(&desktop, &id);
    let actions = [
        StoreAction::UpdateWorkspace(
            id.clone(),
            WorkspacePatch::new(|ws: &mut Workspace| {
                ws.name = "Shop".into();
                ws.detached = Some(true);
                ws.claude_account = Some("work".into());
            }),
        ),
        StoreAction::AddPane {
            ws: id.clone(),
            agent: Some(AgentId::Shell),
            near: Some(panes[0].clone()),
            dir: Some(SplitDir::Col),
            cwd: Some("/srv/site/docs".into()),
            name: None,
            init: None,
        },
        StoreAction::ApplyPreset {
            ws: id.clone(),
            count: 4,
            model_pane: Some(panes[1].clone()),
        },
        update_pane(&id, &panes[0], |pane| {
            pane.title = Some("Build".into());
            pane.model = Some("opus".into());
        }),
        StoreAction::RespawnPane {
            ws: id.clone(),
            pane: panes[1].clone(),
            patch: PanePatch::new(|pane: &mut Pane| pane.session_id = Some("s-2".into())),
        },
        StoreAction::MovePane {
            ws: id.clone(),
            drag: panes[0].clone(),
            target: panes[1].clone(),
            zone: DropZone::Center,
        },
        StoreAction::ApplyPreset {
            ws: id.clone(),
            count: 2,
            model_pane: None,
        },
        StoreAction::ClosePane {
            ws: id.clone(),
            pane: panes[0].clone(),
        },
    ];
    for action in actions {
        let said = format!("{action:?}");
        for op in ops_of(&mut desktop, &id, action) {
            apply_op(&mut node, &id, op);
        }
        let (revision, mirror) = (desktop.revision(), ws(&desktop, &id).clone());
        assert!(!sync(&mut desktop, &node).changed, "{said}");
        assert_eq!(desktop.revision(), revision, "{said}");
        assert_eq!(*ws(&desktop, &id), mirror, "{said}");
        assert_eq!(desktop.take_respawns(), [], "{said}");
        // And the node holds what the mirror shows, in an order of its own.
        let by_id = |panes: &[Pane]| {
            let mut panes = panes.to_vec();
            panes.sort_by(|a, b| a.id.cmp(&b.id));
            panes
        };
        assert_eq!(by_id(&ws(&node, &id).panes), by_id(&mirror.panes), "{said}");
        assert_eq!(ws(&node, &id).name, mirror.name, "{said}");
    }
    for op in ops_of(&mut desktop, &id, StoreAction::RemoveWorkspace(id.clone())) {
        apply_op(&mut node, &id, op);
    }
    assert!(node.state().workspaces.is_empty());
    assert!(!sync(&mut desktop, &node).changed);
}

#[test]
fn a_tree_without_its_views_is_what_a_node_is_handed() {
    let (node, id) = node_with(3);
    let mut workspace = ws(&node, &id).clone();
    let panes = pane_ids(&node, &id);
    let terminals = workspace.tree.clone().unwrap();
    workspace.view_panes.push(ViewPane::new("view"));
    workspace.tree = Some(insert_leaf(&terminals, &panes[1], "view", DropZone::Bottom));
    assert_eq!(terminals_tree(&workspace), terminals);
}
