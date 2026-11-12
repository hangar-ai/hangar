//! One pane drawn (`TerminalPane.tsx`'s view): its header, the notes its
//! launch left, and its terminal with the search bar floating over it.

use std::time::Duration;

use hangar_i18n::{Key, args};
use hangar_theme::{style, tokens};
use hangar_ui::{Icon, button, icon};
use iced::widget::{Column, center, column, container, row, stack, text};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::ctx::Ctx;
use crate::message::Message;
use crate::nodes::presence::{self, Presence};
use crate::panes::controller::{PaneRt, shell_of};
use crate::panes::header::{self, Head};
use crate::panes::term_widget::pane_terminal;
use crate::panes::{Msg, Picker, State, look, search};
use crate::state::types::{Pane, Workspace};

/// How opaque a terminal is when its pane is not the focused one
/// (`.pane:not(.pane--focused) .pane__term { opacity: 0.94 }`): the panes you
/// are not typing into step back a little, only a little, as a theme's dim
/// output is often near 3:1 already. The TS faded the change in over a
/// transition (`--dur`, 140 ms); here it is at once, since a frame of the
/// old look costs nothing and the fade would keep the window redrawing.
pub const UNFOCUSED_OPACITY: f32 = 0.94;

/// What a pane's terminal is drawn over, and how opaque it is, for a pane
/// that is `focused` or not. The pane's own background (`var(--bg)`) is what
/// shows through.
pub fn dimming(focused: bool, theme: &hangar_theme::Theme) -> Option<(iced::Color, f32)> {
    (!focused).then_some((theme.colors.bg, UNFOCUSED_OPACITY))
}

/// Where the pane stands in its grid.
pub struct Placing {
    pub focused: bool,
    pub zoomed: bool,
    pub can_close: bool,
    pub can_zoom: bool,
    /// The pane's width on screen, for the header's breakpoints.
    pub width: f32,
    /// Its place in the workspace's reading order, from 0: the first nine
    /// are reached with `pane.focus1`…`9`.
    pub index: usize,
}

pub fn view<'a>(
    st: &'a State,
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    pane: &'a Pane,
    rt: &'a PaneRt,
    placing: Placing,
) -> Element<'a, Message> {
    let t = ctx.theme;
    let picker = st
        .picker
        .as_ref()
        .filter(|(id, _)| id == &pane.id)
        .map(|(_, picker)| *picker);
    let head = header::view(
        ctx,
        Head {
            ws,
            pane,
            rt,
            focused: placing.focused,
            hovered: st.hovered.as_deref() == Some(pane.id.as_str()),
            zoomed: placing.zoomed,
            can_close: placing.can_close,
            can_zoom: placing.can_zoom,
            width: placing.width,
            index: placing.index,
            shell: shell_of(ctx.model, ws, pane),
            picker: picker.filter(|p| matches!(p, Picker::Agent | Picker::Shell)),
        },
    );

    let mut pane_column: Column<'a, Message> = column![head];
    for note in rt.notes.iter().filter(|note| !rt.dismissed.contains(*note)) {
        pane_column = pane_column.push(note_row(ctx, &pane.id, note));
    }

    // Shortcuts reach the app before the program: the keymap says which.
    let filter = Box::new(crate::keymap::terminal_filter(ctx));
    let mut terminal = pane_terminal(&rt.link, &pane.id, rt.look.padding, filter)
        .scale(crate::panes::scale_of(st, ctx.window, ctx.metrics.scale));
    let away = node_away(ctx, ws, pane);
    if away.is_some() {
        // Greyed while its node is away: what it shows is the last it heard.
        terminal = terminal.dimmed(t.colors.bg, AWAY_OPACITY);
    } else if let Some((behind, opacity)) = dimming(placing.focused, t) {
        terminal = terminal.dimmed(behind, opacity);
    }
    let mut body = stack![terminal].width(Length::Fill).height(Length::Fill);
    if let Some(away) = away {
        body = body.push(away_card(ctx, away));
    }
    if let Some(open) = &rt.search {
        let status = rt.link.lock().search_status();
        body = body.push(search::view(ctx, &pane.id, open, status));
    }
    let background = look::theme_of(ctx.model.settings(), ws).terminal.background;
    pane_column = pane_column.push(
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(background)),
                ..container::Style::default()
            }),
    );
    container(pane_column)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(style::container::window(t))
        .into()
}

/// How opaque the terminal of a pane whose node is away is.
const AWAY_OPACITY: f32 = 0.4;

/// The widest the card over a pane whose node is away gets: a failure's
/// sentence wraps inside it.
const AWAY_CARD_WIDTH: f32 = 420.0;

/// The node a pane runs on, while nothing can be asked of it, and what is
/// known of why.
pub struct Away {
    pub node: String,
    pub label: String,
    pub presence: Presence,
    /// How long its link has been down.
    pub down_for: Option<Duration>,
}

/// `Some` for a pane on a node that cannot be reached — of a workspace on a
/// node, or a worker a local workspace sent to one: its terminal is greyed
/// under a card that says so and offers what can be done. Its state stays
/// as it was, and its shell runs on in the node. A link that has only just
/// dropped says nothing yet: most are back within the grace period.
pub fn node_away(ctx: Ctx<'_>, ws: &Workspace, pane: &Pane) -> Option<Away> {
    let node = crate::nodes::away::node_of(ws, pane)?;
    let presence = presence::of(ctx.model, &node, ctx.now);
    if matches!(presence, Presence::Connected | Presence::Settling) {
        return None;
    }
    let down_for = ctx
        .model
        .live
        .nodes
        .links
        .get(&node)
        .and_then(|link| link.down_for(ctx.now));
    Some(Away {
        label: crate::nodes::label(ctx.model, &node),
        node,
        presence,
        down_for,
    })
}

/// What the card says of the machine, and what it offers: trying again now
/// for a link that is down, switching the machine off once it has been
/// down a while, switching it back on for one that is off.
fn away_card<'a>(ctx: Ctx<'a>, away: Away) -> Element<'a, Message> {
    let t = ctx.theme;
    let said = |key: Key| {
        ctx.i18n
            .t_with(key, &args!("node" => away.label.as_str()))
            .into_owned()
    };
    let words = match &away.presence {
        Presence::Connecting { again: true } => said(Key::NodesPaneReconnecting),
        Presence::Connecting { again: false } => said(Key::NodesPaneConnecting),
        Presence::Offline(Some(failure)) => ctx
            .i18n
            .t_with(
                Key::NodesPaneOffline,
                &args!(
                    "node" => away.label.as_str(),
                    "reason" => crate::nodes::failure_words(ctx.i18n, failure)
                ),
            )
            .into_owned(),
        Presence::Off => said(Key::NodesPaneOff),
        _ => said(Key::NodesPaneAway),
    };
    let nodes = |msg: crate::nodes::Msg| Message::Nodes(msg);
    let mut actions = row![].spacing(tokens::space::MD);
    if matches!(
        away.presence,
        Presence::Connecting { .. } | Presence::Offline(Some(_))
    ) {
        actions = actions.push(
            button::secondary(t, ctx.t(Key::NodesRetry))
                .on_press(nodes(crate::nodes::Msg::Retry(away.node.clone()))),
        );
        if away
            .down_for
            .is_some_and(|down| down >= presence::SWITCH_OFF_AFTER)
        {
            actions = actions.push(
                button::ghost(t, ctx.t(Key::NodesSwitchOff))
                    .tip(ctx.t(Key::NodesSwitchOffHint))
                    .on_press(nodes(crate::nodes::Msg::SetEnabled(
                        away.node.clone(),
                        false,
                    ))),
            );
        }
    } else if away.presence == Presence::Off {
        actions = actions.push(
            button::secondary(t, ctx.t(Key::NodesSwitchOn)).on_press(nodes(
                crate::nodes::Msg::SetEnabled(away.node.clone(), true),
            )),
        );
    }
    // The machine by its own icon, as everywhere; the colour is its
    // state's, and the words say the rest.
    let look = presence::look_of(ctx.model, &away.node, ctx.now);
    let machine = presence::icon_of(ctx.model, &away.node);
    let fg = t.colors.fg;
    let words: Element<'a, Message> = text(words)
        .size(tokens::font_size::UI)
        .style(move |_| text::Style { color: Some(fg) })
        .into();
    // What the tool said in full is a look away, not in the sentence.
    let detail = match &away.presence {
        Presence::Offline(Some(failure)) => crate::nodes::failure::detail(ctx.i18n, failure),
        _ => None,
    };
    let words = match detail {
        Some(detail) => hangar_ui::tip::bottom(t, words, detail),
        None => words,
    };
    let said = column![words, actions].spacing(tokens::space::LG);
    let card = container(
        row![
            // Level with the first line of the words, which may wrap.
            container(icon(machine, tokens::size::ICON_LG).color(look.tone.color(t))).padding(
                Padding {
                    top: 1.0,
                    ..Padding::ZERO
                }
            ),
            said,
        ]
        .spacing(tokens::space::LG),
    )
    .max_width(AWAY_CARD_WIDTH)
    .padding(Padding::from([tokens::space::XL, tokens::space::XXL]))
    .style(style::container::popover(t));
    center(card).padding(tokens::space::XL).into()
}

/// A note of the launch: a kit left out, an account that is gone. In the
/// pane's bar rather than its terminal, which Claude Code clears as it
/// starts; each one dismissible.
fn note_row<'a>(ctx: Ctx<'a>, pane: &'a str, note: &'a str) -> Element<'a, Message> {
    let t = ctx.theme;
    let warning = t.colors.warning;
    container(
        row![
            text(note)
                .size(tokens::font_size::CAPTION)
                .width(Length::Fill)
                .style(move |_| text::Style {
                    color: Some(warning)
                }),
            button::icon_button(t, Icon::Close)
                .side(18.0)
                .icon_size(10.0)
                .tip(ctx.t(Key::PaneNoteDismiss).into_owned())
                .on_press(Message::Panes(Msg::DismissNote(
                    pane.to_owned(),
                    note.to_owned()
                ))),
        ]
        .spacing(tokens::space::MD)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([tokens::space::XXS, tokens::space::MD]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(warning.scale_alpha(0.1))),
        border: Border {
            color: warning.scale_alpha(0.25),
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::nodes::model::{Failure, Step};
    use crate::nodes::testing::{down, machine, up};
    use crate::state::types::MachineKind;
    use crate::workspace_dialog::testing::{Harness, messages};

    /// A window with the experiment on and a node, `GPU box`, that the
    /// first workspace runs on.
    fn remote(locale: hangar_i18n::Locale, theme: &str) -> Harness {
        Harness::new(locale, theme).edit(|state| {
            state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
            state.settings.nodes = vec![machine("n1", "GPU box", MachineKind::Tower)];
            state.workspaces[0].node = Some("n1".into());
        })
    }

    fn timed_out() -> Failure {
        Failure::Ssh {
            step: Step::Probe,
            detail: "ssh: connect to host gpu port 22: Connection timed out".into(),
        }
    }

    fn sent(
        ui: iced_test::Simulator<'_, Message>,
        wanted: impl Fn(&crate::nodes::Msg) -> bool,
    ) -> bool {
        messages(ui)
            .into_iter()
            .any(|message| matches!(&message, Message::Nodes(msg) if wanted(msg)))
    }

    /// A link that has only just dropped says nothing; past the grace
    /// period the pane is greyed under a card that says the machine is
    /// reconnecting, with a way to try now.
    #[test]
    fn a_pane_whose_link_dropped_waits_then_says_it_is_reconnecting() {
        let mut h = remote(hangar_i18n::Locale::En, "graphite");
        let ws = h.model.state().workspaces[0].clone();
        let mut st = State::default();
        st.mount_for_test(&ws);
        let id = ws.id.clone();

        up(&mut h.model, "n1", "gpu-01");
        {
            let mut ui = h.show(crate::panes::view_workspace(&st, h.ctx(), &id, false));
            assert!(ui.find("Retry now").is_err());
        }
        let at = down(&mut h.model, "n1", 2, None);
        {
            let ctx = Ctx { now: at, ..h.ctx() };
            let mut ui = h.show(crate::panes::view_workspace(&st, ctx, &id, false));
            assert!(
                ui.find("GPU box is reconnecting…").is_err(),
                "no alarm within the grace period"
            );
        }
        down(&mut h.model, "n1", 8, None);
        let mut ui = h.show(crate::panes::view_workspace(&st, h.ctx(), &id, false));
        assert!(ui.find("GPU box is reconnecting…").is_ok());
        assert!(ui.find("Switch off").is_err(), "not offered so soon");
        h.shot(&mut ui, "machines-card-reconnecting-en-graphite");
        ui.click("Retry now").expect("a way to try now");
        assert!(sent(ui, |msg| matches!(
            msg,
            crate::nodes::Msg::Retry(node) if node == "n1"
        )));
    }

    /// Once an attempt has failed the card gives the reason, and after
    /// twenty seconds down it also offers to switch the machine off.
    #[test]
    fn a_pane_whose_node_is_offline_says_why_and_offers_to_switch_it_off() {
        let mut h = remote(hangar_i18n::Locale::En, "graphite");
        let ws = h.model.state().workspaces[0].clone();
        let mut st = State::default();
        st.mount_for_test(&ws);
        let id = ws.id.clone();

        down(&mut h.model, "n1", 10, Some(timed_out()));
        {
            let mut ui = h.show(crate::panes::view_workspace(&st, h.ctx(), &id, false));
            assert!(
                ui.find("GPU box is offline: unreachable over SSH (Connection timed out)")
                    .is_ok()
            );
            assert!(ui.find("Retry now").is_ok());
            assert!(ui.find("Switch off").is_err());
        }
        down(&mut h.model, "n1", 25, Some(timed_out()));
        let mut ui = h.show(crate::panes::view_workspace(&st, h.ctx(), &id, false));
        h.shot(&mut ui, "machines-card-offline-en-graphite");
        ui.click("Switch off")
            .expect("offered after twenty seconds");
        assert!(sent(ui, |msg| matches!(
            msg,
            crate::nodes::Msg::SetEnabled(node, false) if node == "n1"
        )));
    }

    /// The card in French, in a light theme.
    #[test]
    fn the_card_speaks_french_in_a_light_theme() {
        let mut h = remote(hangar_i18n::Locale::Fr, "github-light");
        let ws = h.model.state().workspaces[0].clone();
        let mut st = State::default();
        st.mount_for_test(&ws);
        let id = ws.id.clone();
        down(&mut h.model, "n1", 25, Some(timed_out()));
        let mut ui = h.show(crate::panes::view_workspace(&st, h.ctx(), &id, false));
        assert!(ui.find("Réessayer maintenant").is_ok());
        assert!(ui.find("Désactiver").is_ok());
        h.shot(&mut ui, "machines-card-offline-fr-github-light");
    }

    /// A machine that is switched off says so, with the way back on; one
    /// the settings no longer hold only says it is not connected.
    #[test]
    fn a_pane_of_a_machine_switched_off_offers_to_switch_it_on() {
        let h = remote(hangar_i18n::Locale::En, "graphite").edit(|state| {
            state.settings.nodes[0].enabled = Some(false);
        });
        let ws = h.model.state().workspaces[0].clone();
        let mut st = State::default();
        st.mount_for_test(&ws);
        let id = ws.id.clone();
        {
            let mut ui = h.show(crate::panes::view_workspace(&st, h.ctx(), &id, false));
            assert!(ui.find("GPU box is switched off").is_ok());
            assert!(ui.find("Retry now").is_err());
            ui.click("Switch on").expect("the way back");
            assert!(sent(ui, |msg| matches!(
                msg,
                crate::nodes::Msg::SetEnabled(node, true) if node == "n1"
            )));
        }
        let h = h.edit(|state| state.settings.nodes.clear());
        let mut ui = h.show(crate::panes::view_workspace(&st, h.ctx(), &id, false));
        assert!(ui.find("n1 is not connected").is_ok());
        assert!(ui.find("Retry now").is_err());
        assert!(ui.find("Switch on").is_err());
    }

    /// A worker a local workspace sent to a node: its bar names the node,
    /// and while that node is away it is greyed and says so; the workspace's
    /// other panes, of this machine, are drawn as they are.
    #[test]
    fn a_worker_on_a_node_names_it_and_says_when_it_is_away() {
        let mut h = Harness::new(hangar_i18n::Locale::En, "graphite").edit(|state| {
            state.settings.experiments = vec![crate::settings::experiments::NODES.into()];
            state.settings.nodes = vec![machine("n1", "GPU box", MachineKind::Tower)];
            let first = &mut state.workspaces[0].panes[0];
            first.node = Some("n1".into());
        });
        let ws = h.model.state().workspaces[0].clone();
        assert!(ws.node.is_none(), "a workspace of this machine");
        let mut st = State::default();
        st.mount_for_test(&ws);
        let id = ws.id.clone();
        down(&mut h.model, "n1", 10, Some(timed_out()));
        {
            let mut ui = h.show(crate::panes::view_workspace(&st, h.ctx(), &id, false));
            assert!(ui.find("GPU box").is_ok(), "the bar names the node");
            assert!(ui.find("Retry now").is_ok());
            h.shot(&mut ui, "machines-card-sent-pane-en-graphite");
        }
        up(&mut h.model, "n1", "gpu-01");
        let mut ui = h.show(crate::panes::view_workspace(&st, h.ctx(), &id, false));
        assert!(ui.find("Retry now").is_err());
        assert!(ui.find("GPU box").is_ok());
    }

    #[test]
    fn only_the_panes_that_are_not_focused_step_back() {
        let theme = hangar_theme::default_theme();
        assert_eq!(dimming(true, theme), None);
        let (behind, opacity) = dimming(false, theme).expect("an unfocused pane");
        // The pane's background shows through, at 0.94.
        assert_eq!(behind, theme.colors.bg);
        assert_eq!(opacity, 0.94);
    }
}
