//! The questions asked before something that cannot be undone from here is
//! done to a workspace on a node (`Overlay::NodeAsk`): handing one this
//! computer drives to its node, and deleting one the node runs itself.
//!
//! Each is a short dialog that says what happens in plain words. Its yes
//! is checked again when it is given: the workspace may have changed, or
//! its node dropped, while the question was on screen.

use hangar_i18n::{Key, args};
use hangar_theme::tokens;
use hangar_ui::{button, dialog, text as typography};
use iced::widget::{Space, column};
use iced::{Element, Length, Task};

use super::{Ask, Msg, hosted};
use crate::ctx::Ctx;
use crate::message::{Intent, Message};
use crate::shell::{Notice, Overlay};
use crate::state::types::Workspace;
use crate::state::{Model, StoreAction};

fn notice(text: String) -> Task<Message> {
    Task::done(Message::Intent(Intent::Notice(Notice::Problem(text))))
}

/// « Move to {machine} » was picked for workspace `ws`: the question is
/// asked, unless the move cannot be made now, which is said instead.
pub fn ask_move(m: &Model, ws: &str) -> Task<Message> {
    let Some(found) = movable(m, ws) else {
        return Task::none();
    };
    match hosted::move_refusal(m, found) {
        Some(reason) => notice(reason),
        None => Task::done(Message::Intent(Intent::OpenOverlay(Overlay::NodeAsk(
            Ask::Move(ws.to_owned()),
        )))),
    }
}

/// Workspace `ws`, when it is one this computer drives on a node.
fn movable<'a>(m: &'a Model, ws: &str) -> Option<&'a Workspace> {
    m.workspace(ws)
        .filter(|found| found.node.is_some() && !found.is_hosted())
}

/// The answer to a question: a no closes it; a yes does what was asked,
/// when it still can be.
pub fn answered(m: &mut Model, ask: Ask, yes: bool) -> Task<Message> {
    let close = Task::done(Message::Intent(Intent::CloseOverlay));
    if !yes {
        return close;
    }
    match ask {
        Ask::Move(ws) => {
            let Some(found) = movable(m, &ws) else {
                return close;
            };
            if let Some(reason) = hosted::move_refusal(m, found) {
                return Task::batch([close, notice(reason)]);
            }
            hosted::hand_over(m, &ws, hosted::Handover::Moved);
            close
        }
        // Refused, and said, by the store's door while the node is away.
        Ask::Remove(ws) => {
            m.apply(StoreAction::RemoveWorkspace(ws));
            close
        }
    }
}

/// The dialog of `ask`; nothing once its workspace is gone.
pub fn view<'a>(ctx: Ctx<'a>, ask: &'a Ask) -> Element<'a, Message> {
    let t = ctx.theme;
    let (Ask::Move(id) | Ask::Remove(id)) = ask;
    let Some(ws) = ctx.model.workspace(id) else {
        return Space::new().width(0).height(0).into();
    };
    let node = ws.node.as_deref().unwrap_or_default();
    let label = super::label(ctx.model, node);
    let named = args!("name" => ws.name.as_str(), "node" => label.as_str());
    let words = |key: Key| ctx.i18n.t_with(key, &named).into_owned();
    let answer = |yes: bool| Message::Nodes(Msg::Answer(ask.clone(), yes));

    let (title, mut lines, confirm) = match ask {
        Ask::Move(_) => (
            words(Key::NodesMoveTitle),
            vec![
                words(Key::NodesMoveRestart),
                words(Key::NodesMoveStays),
                words(Key::NodesMoveNoUndo),
            ],
            button::primary(t, words(Key::NodesMoveConfirm)),
        ),
        Ask::Remove(_) => (
            words(Key::NodesRemoveTitle),
            vec![words(Key::NodesRemoveBody), words(Key::NodesRemoveKept)],
            button::danger(t, ctx.t(Key::NodesRemoveConfirm)),
        ),
    };
    if matches!(ask, Ask::Move(_)) {
        let busy = hosted::working(ctx.model, ws);
        match busy.as_slice() {
            [] => {}
            [one] => lines.insert(
                1,
                ctx.i18n
                    .t_with(Key::NodesMoveWorkingOne, &args!("name" => one.as_str()))
                    .into_owned(),
            ),
            many => lines.insert(
                1,
                ctx.i18n
                    .t_with(Key::NodesMoveWorkingMany, &args!("count" => many.len()))
                    .into_owned(),
            ),
        }
    }

    let mut body = column![].spacing(tokens::space::MD).width(Length::Fill);
    for line in lines {
        body = body.push(typography::body(t, line));
    }
    let panel = dialog::dialog(t, title, body)
        .width(460.0)
        .close_tip(ctx.t(Key::ModalClose))
        .action(button::ghost(t, ctx.t(Key::NodesRowCancel)).on_press(answer(false)))
        .action(confirm.on_press(answer(true)))
        .on_close(answer(false));
    let page = Space::new().width(Length::Fill).height(Length::Fill);
    dialog::over(t, page, Some(panel)).into()
}
