//! Settings › Nodes: the machines as rows, this computer first, each with
//! its kind's icon (which opens the picker), its name, where it is and how
//! its link is doing, a switch that keeps it connected or not, and a menu
//! for the rest; then the form that adds one.

use std::borrow::Cow;

use hangar_i18n::{Key, args};
use hangar_theme::tokens;
use hangar_ui::menu::{self as ui_menu, Entry};
use hangar_ui::{Icon, button as ui_button, form, icon, input, text as typography, tip};
use iced::widget::{column, container, row, text};
use iced::{Alignment, Element, Length, Padding};

use crate::ctx::Ctx;
use crate::message::Message;
use crate::state::types::{MachineKind, NodeConfig};

use super::failure;
use super::model::{NodeStatus, Step, TargetProblem, target_problem};
use super::presence::{self, Presence};
use super::{Msg, Open, State};

fn press(msg: Msg) -> Message {
    Message::Nodes(msg)
}

/// A dictionary string that takes no argument: always borrowed from the
/// dictionary, which a field's placeholder needs to be.
fn static_text(text: Cow<'static, str>) -> &'static str {
    match text {
        Cow::Borrowed(text) => text,
        Cow::Owned(_) => "",
    }
}

pub fn step_key(step: Step) -> Key {
    match step {
        Step::Probe => Key::NodesStepProbe,
        Step::Install => Key::NodesStepInstall,
        Step::Start => Key::NodesStepStart,
        Step::Config => Key::NodesStepConfig,
        Step::Tunnel => Key::NodesStepTunnel,
        Step::Handshake => Key::NodesStepHandshake,
    }
}

/// The width of the icon at the head of a row, which the lines under a
/// row are indented by to sit under its name.
const LEAD: f32 = tokens::size::CONTROL_HEIGHT;

pub fn settings_page<'a>(st: &'a State, ctx: Ctx<'a>) -> Element<'a, Message> {
    let t = ctx.theme;
    let nodes = &ctx.model.settings().nodes;
    let mut rows: Vec<Element<'a, Message>> = vec![local_row(ctx)];
    rows.extend(nodes.iter().map(|node| machine_row(st, ctx, node)));
    let mut body = column![form::group(t, rows)].spacing(tokens::space::XL);
    if nodes.is_empty() {
        body = body.push(typography::hint(t, ctx.t(Key::NodesSettingsEmpty)));
    }
    body = body.push(add_form(st, ctx));
    form::section(t, ctx.t(Key::SettingsCatNodes))
        .hint(ctx.t(Key::NodesSettingsIntro))
        .bare(body)
        .into()
}

/// A row's frame: what leads it, its name over a quieter line, and what
/// ends it.
fn row_frame<'a>(
    lead: Element<'a, Message>,
    lines: Element<'a, Message>,
    end: Option<Element<'a, Message>>,
) -> iced::widget::Row<'a, Message> {
    let mut head = row![lead, container(lines).width(Length::Fill)]
        .spacing(tokens::space::LG)
        .align_y(Alignment::Center);
    if let Some(end) = end {
        head = head.push(end);
    }
    head
}

fn name_text<'a>(ctx: Ctx<'a>, name: impl text::IntoFragment<'a>) -> text::Text<'a> {
    typography::body(ctx.theme, name).font(hangar_theme::fonts::ui_semibold())
}

/// The name this computer has on its network, read once.
pub(crate) fn local_hostname() -> Option<&'static str> {
    static NAME: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    NAME.get_or_init(hangar_core::paths::hostname).as_deref()
}

/// This computer, which heads the list: it is a machine like the others,
/// only never away. Under its name, the name it has on its network, which
/// is what the other rows say of theirs once connected.
fn local_row<'a>(ctx: Ctx<'a>) -> Element<'a, Message> {
    let t = ctx.theme;
    let lead =
        container(icon(presence::LOCAL_ICON, tokens::size::ICON_LG).color(t.colors.fg_muted))
            .center_x(LEAD)
            .center_y(LEAD);
    let mut lines =
        column![name_text(ctx, ctx.t(Key::NodesCreateLocal))].spacing(tokens::space::XXS);
    if let Some(hostname) = local_hostname() {
        lines = lines.push(typography::hint(t, hostname));
    }
    container(row_frame(lead.into(), lines.into(), None))
        .padding(Padding::from([tokens::space::LG, tokens::space::XL]))
        .width(Length::Fill)
        .into()
}

/// The kinds of machine as a menu's lines, the current one checked.
fn kind_entries(
    ctx: Ctx<'_>,
    current: MachineKind,
    pick: impl Fn(MachineKind) -> Msg,
) -> Vec<Entry<Message>> {
    MachineKind::ALL
        .into_iter()
        .map(|kind| {
            ui_menu::item(ctx.t(presence::kind_key(kind)))
                .icon(presence::kind_icon(kind))
                .checked(kind == current)
                .on_select(press(pick(kind)))
                .into()
        })
        .collect()
}

/// A line of a row's « … » menu: what it says, and what picking it sends
/// (`None` for one that cannot be picked now).
pub(crate) struct MoreLine {
    pub label: Key,
    pub icon: Icon,
    pub danger: bool,
    pub send: Option<Msg>,
}

/// What a row's « … » menu offers for `node`.
pub(crate) fn more_lines(ctx: Ctx<'_>, node: &NodeConfig) -> Vec<MoreLine> {
    let id = &node.id;
    // Only a link that is down has an attempt to bring forward.
    let waiting = node.is_on() && presence::exact(ctx.model, id).is_away();
    vec![
        MoreLine {
            label: Key::NodesRetry,
            icon: Icon::Restart,
            danger: false,
            send: waiting.then(|| Msg::Retry(id.clone())),
        },
        MoreLine {
            label: Key::NodesRowRename,
            icon: Icon::Pencil,
            danger: false,
            send: Some(Msg::Show(Some(Open::Rename {
                id: id.clone(),
                draft: node.label.clone(),
            }))),
        },
        MoreLine {
            label: Key::NodesRowRemove,
            icon: Icon::Trash,
            danger: true,
            send: Some(Msg::Show(Some(Open::Remove(id.clone())))),
        },
    ]
}

/// The button a machine's icon is: it opens the picker of its kind.
fn kind_button<'a>(
    ctx: Ctx<'a>,
    current: MachineKind,
    open: bool,
    toggle: Open,
    pick: impl Fn(MachineKind) -> Msg,
) -> Element<'a, Message> {
    let t = ctx.theme;
    let words = ctx
        .i18n
        .t_with(
            Key::NodesKindPick,
            &args!("kind" => ctx.t(presence::kind_key(current)).into_owned()),
        )
        .into_owned();
    let button = ui_button::icon_button(t, presence::kind_icon(current))
        .icon_size(tokens::size::ICON_LG)
        .tip(words)
        .toggled(open)
        .on_press(press(Msg::Show((!open).then_some(toggle))));
    let entries = open.then(|| kind_entries(ctx, current, pick));
    ui_menu::dropdown(t, button, entries)
        .on_dismiss(press(Msg::Show(None)))
        .into()
}

/// Where the machine is: its SSH target, and its config file when it has
/// one of its own.
fn target_words(node: &NodeConfig) -> String {
    match node.ssh_config.as_deref() {
        Some(config) => format!("{} · -F {config}", node.ssh_target),
        None => node.ssh_target.clone(),
    }
}

/// How the link is doing, in a sentence, and whether that is something to
/// look at. The settings say it as it happens, without the grace period
/// the other surfaces give a link.
pub(crate) fn state_words(ctx: Ctx<'_>, node: &NodeConfig) -> (String, bool) {
    let tr = ctx.i18n;
    let live = &ctx.model.live.nodes;
    let status = live.status(&node.id);
    let link = live.links.get(&node.id);
    let step_words =
        |key: Key, step: Step| tr.t_with(key, &args!("step" => tr.t(step_key(step)).into_owned()));
    let words = match presence::exact(ctx.model, &node.id) {
        Presence::Off => tr.t(Key::NodesRowOff),
        Presence::Connected | Presence::Settling => match &status {
            NodeStatus::Connected { info, .. } => tr.t_with(
                Key::NodesRowConnected,
                &args!("version" => info.version.as_str(), "hostname" => info.hostname.as_str()),
            ),
            _ => tr.t(Key::NodesRowStarting),
        },
        Presence::Offline(None) => tr.t(Key::NodesRowUnavailable),
        Presence::Offline(Some(failure)) => {
            let reason = failure::words(tr, &failure);
            match link.and_then(|link| link.retry_in(ctx.now)) {
                Some(seconds) if seconds > 0 => tr.t_with(
                    Key::NodesRowOfflineIn,
                    &args!("seconds" => seconds as i64, "reason" => reason),
                ),
                // An attempt is under way.
                _ => tr.t_with(Key::NodesRowOfflineNow, &args!("reason" => reason)),
            }
        }
        Presence::Connecting { again } => match &status {
            NodeStatus::Connecting(step) if again => step_words(Key::NodesRowReconnecting, *step),
            NodeStatus::Connecting(step) => step_words(Key::NodesRowConnecting, *step),
            NodeStatus::Unreachable { failure, retry_in } => {
                let seconds = link
                    .and_then(|link| link.retry_in(ctx.now))
                    .unwrap_or(*retry_in);
                tr.t_with(
                    Key::NodesRowReconnectingIn,
                    &args!("seconds" => seconds as i64, "reason" => failure::words(tr, failure)),
                )
            }
            _ => tr.t(Key::NodesRowStarting),
        },
    };
    let alert = matches!(
        presence::exact(ctx.model, &node.id),
        Presence::Offline(Some(_))
    ) || matches!(status, NodeStatus::Unreachable { .. });
    (words.into_owned(), alert)
}

/// The longest a tool's own words run in a row.
const DETAIL_MAX: usize = 200;

/// What the tool said of the failure the row of `node` reports, when the
/// row's sentence does not hold all of it.
pub(crate) fn failure_detail(ctx: Ctx<'_>, node: &NodeConfig) -> Option<String> {
    let live = &ctx.model.live.nodes;
    let failure = match presence::exact(ctx.model, &node.id) {
        Presence::Offline(Some(failure)) => failure,
        Presence::Connecting { .. } => match live.status(&node.id) {
            NodeStatus::Unreachable { failure, .. } => failure,
            _ => return None,
        },
        _ => return None,
    };
    let detail = failure::detail(ctx.i18n, &failure)?;
    Some(if detail.chars().count() > DETAIL_MAX {
        let cut: String = detail.chars().take(DETAIL_MAX).collect();
        format!("{cut}…")
    } else {
        detail
    })
}

fn machine_row<'a>(st: &'a State, ctx: Ctx<'a>, node: &'a NodeConfig) -> Element<'a, Message> {
    let t = ctx.theme;
    let id = &node.id;
    let label = super::away::label_of(node);
    let status = ctx.model.live.nodes.status(id);

    let lead = kind_button(
        ctx,
        node.kind,
        st.open == Some(Open::Kind(id.clone())),
        Open::Kind(id.clone()),
        {
            let id = id.clone();
            move |kind| Msg::SetKind(id.clone(), kind)
        },
    );

    let renaming = match &st.open {
        Some(Open::Rename { id: renamed, draft }) if renamed == id => Some(draft.as_str()),
        _ => None,
    };
    let (state, alert) = state_words(ctx, node);
    let mut lines = column![].spacing(tokens::space::XXS);
    lines = match renaming {
        Some(draft) => lines.push(
            input::text_field(t, draft)
                .placeholder(static_text(ctx.t(Key::NodesAddLabelPlaceholder)))
                .on_input(|value| press(Msg::RenameInput(value)))
                .on_submit(press(Msg::RenameSubmit))
                .width(260.0),
        ),
        // A machine that is switched off steps back, as its rows do.
        None if !node.is_on() => lines.push(name_text(ctx, label.clone()).color(t.colors.fg_muted)),
        None => lines.push(name_text(ctx, label.clone())),
    };
    if alert {
        // What needs a look gets a line of its own, in the warning colour.
        let warning = t.colors.warning;
        lines = lines
            .push(typography::hint(t, target_words(node)))
            .push(typography::hint(t, state).color(warning));
        // What the tool said in full, quieter, for whoever has to fix it.
        if let Some(detail) = failure_detail(ctx, node) {
            lines = lines.push(typography::subtle(t, detail));
        }
    } else {
        lines = lines.push(typography::hint(
            t,
            format!("{} · {state}", target_words(node)),
        ));
    }
    if let NodeStatus::Connected { info, .. } = &status
        && node.is_on()
    {
        lines = lines.push(tools_line(ctx, info.claude, info.git));
    }
    // What it runs itself is in the navigator, with every other workspace:
    // here, only how many, known whether or not it is connected.
    if node.is_on()
        && let Some(words) = own_words(ctx, id)
    {
        lines = lines.push(typography::subtle(t, words));
    }
    // Those this computer still drives there stop when it does: said, and
    // each is handed over from its own menu in the navigator.
    if node.is_on()
        && let Some(words) = driven_words(ctx, id)
    {
        lines = lines.push(typography::subtle(t, words));
    }

    let end: Element<'a, Message> = if renaming.is_some() {
        row![
            ui_button::primary(t, ctx.t(Key::NodesRowRenameSave))
                .small()
                .on_press(press(Msg::RenameSubmit)),
            ui_button::ghost(t, ctx.t(Key::NodesRowCancel))
                .small()
                .on_press(press(Msg::Show(None))),
        ]
        .spacing(tokens::space::SM)
        .align_y(Alignment::Center)
        .into()
    } else {
        let toggle = {
            let id = id.clone();
            input::switch(t, node.is_on())
                .on_toggle(move |on| press(Msg::SetEnabled(id.clone(), on)))
        };
        let words = |key: Key| {
            ctx.i18n
                .t_with(key, &args!("node" => label.as_str()))
                .into_owned()
        };
        row![
            tip::bottom(t, toggle, words(Key::NodesRowToggle)),
            more_menu(st, ctx, node, words(Key::NodesRowMore)),
        ]
        .spacing(tokens::space::MD)
        .align_y(Alignment::Center)
        .into()
    };

    let mut block = column![row_frame(lead, lines.into(), Some(end))].spacing(tokens::space::LG);
    if st.open == Some(Open::Remove(id.clone())) {
        block = block.push(under(remove_question(ctx, node, &label)));
    }
    container(block)
        .padding(Padding::from([tokens::space::LG, tokens::space::XL]))
        .width(Length::Fill)
        .into()
}

/// How many workspaces node `id` runs itself, in words; nothing when it
/// runs none.
fn own_words(ctx: Ctx<'_>, id: &str) -> Option<String> {
    match super::hosted::count(ctx.model, id) {
        0 => None,
        1 => Some(ctx.t(Key::NodesOwnOne).into_owned()),
        count => Some(
            ctx.i18n
                .t_with(Key::NodesOwnMany, &args!("count" => count))
                .into_owned(),
        ),
    }
}

/// How many workspaces on node `id` this computer still drives, in words;
/// nothing when there is none.
fn driven_words(ctx: Ctx<'_>, id: &str) -> Option<String> {
    match super::hosted::driven_here(ctx.model, id) {
        0 => None,
        1 => Some(ctx.t(Key::NodesStillHereOne).into_owned()),
        count => Some(
            ctx.i18n
                .t_with(Key::NodesStillHereMany, &args!("count" => count))
                .into_owned(),
        ),
    }
}

/// What sits under a row, in line with its name.
fn under<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(content)
        .padding(Padding {
            left: LEAD + tokens::space::LG,
            ..Padding::ZERO
        })
        .into()
}

/// What the machine has of what the agents need, in one quiet line; what
/// it lacks stands out.
fn tools_line<'a>(ctx: Ctx<'a>, claude: bool, git: bool) -> Element<'a, Message> {
    let t = ctx.theme;
    let note = |found: bool, yes: Key, no: Key| {
        if found {
            typography::subtle(t, ctx.t(yes))
        } else {
            typography::subtle(t, ctx.t(no)).color(t.colors.warning)
        }
    };
    row![
        note(claude, Key::NodesClaudeFound, Key::NodesClaudeMissing),
        typography::subtle(t, "·"),
        note(git, Key::NodesGitFound, Key::NodesGitMissing),
    ]
    .spacing(tokens::space::SM)
    .into()
}

/// The « … » at the end of a row, and its menu.
fn more_menu<'a>(
    st: &'a State,
    ctx: Ctx<'a>,
    node: &'a NodeConfig,
    words: String,
) -> Element<'a, Message> {
    let t = ctx.theme;
    let id = &node.id;
    let open = st.open == Some(Open::More(id.clone()));
    let button = ui_button::icon_button(t, Icon::More)
        .tip(words)
        .toggled(open)
        .on_press(press(Msg::Show((!open).then(|| Open::More(id.clone())))));
    let entries = open.then(|| {
        let mut entries = Vec::new();
        for line in more_lines(ctx, node) {
            let mut item = ui_menu::item(ctx.t(line.label)).icon(line.icon);
            if line.danger {
                // What takes something away sits apart from the rest.
                entries.push(ui_menu::separator());
                item = item.danger();
            }
            entries.push(match line.send {
                Some(msg) => item.on_select(press(msg)).into(),
                None => item.disabled().into(),
            });
        }
        entries
    });
    ui_menu::dropdown(t, button, entries)
        .align_end()
        .on_dismiss(press(Msg::Show(None)))
        .into()
}

/// The question a removal waits on, under the row it is about.
fn remove_question<'a>(ctx: Ctx<'a>, node: &NodeConfig, label: &str) -> Element<'a, Message> {
    let t = ctx.theme;
    let asked = ctx
        .i18n
        .t_with(Key::NodesRowRemoveAsk, &args!("node" => label))
        .into_owned();
    column![
        typography::hint(t, asked),
        row![
            ui_button::danger(t, ctx.t(Key::NodesRowRemoveConfirm))
                .small()
                .on_press(press(Msg::Remove(node.id.clone()))),
            ui_button::ghost(t, ctx.t(Key::NodesRowCancel))
                .small()
                .on_press(press(Msg::Show(None))),
        ]
        .spacing(tokens::space::SM)
        .align_y(Alignment::Center),
    ]
    .spacing(tokens::space::MD)
    .into()
}

fn add_form<'a>(st: &'a State, ctx: Ctx<'a>) -> Element<'a, Message> {
    let t = ctx.theme;
    let form_state = &st.form;
    let problem = form_state
        .touched
        .then(|| target_problem(&form_state.target))
        .flatten();
    let label = input::text_field(t, &form_state.label)
        .placeholder(static_text(ctx.t(Key::NodesAddLabelPlaceholder)))
        .on_input(|value| press(Msg::AddLabel(value)))
        .on_submit(press(Msg::AddSubmit));
    let target = input::text_field(t, &form_state.target)
        .placeholder(static_text(ctx.t(Key::NodesAddTargetPlaceholder)))
        .on_input(|value| press(Msg::AddTarget(value)))
        .on_submit(press(Msg::AddSubmit))
        .error(problem.map(|problem| {
            ctx.t(match problem {
                TargetProblem::Empty => Key::NodesAddTargetEmpty,
                TargetProblem::Invalid => Key::NodesAddTargetInvalid,
            })
            .into_owned()
        }));
    let config = input::text_field(t, &form_state.config)
        .placeholder(static_text(ctx.t(Key::NodesAddConfigPlaceholder)))
        .on_input(|value| press(Msg::AddConfig(value)))
        .on_submit(press(Msg::AddSubmit));
    let kind = kind_button(
        ctx,
        form_state.kind,
        st.open == Some(Open::AddKind),
        Open::AddKind,
        Msg::AddKind,
    );
    let named: Element<'a, Message> = row![kind, label]
        .spacing(tokens::space::MD)
        .align_y(Alignment::Center)
        .into();
    let field = |key: Key, input: Element<'a, Message>| -> Element<'a, Message> {
        column![typography::hint(t, ctx.t(key)), input]
            .spacing(tokens::space::SM)
            .into()
    };
    form::group(
        t,
        vec![
            container(
                column![
                    typography::body(t, ctx.t(Key::NodesAddTitle))
                        .font(hangar_theme::fonts::ui_semibold()),
                    field(Key::NodesAddLabel, named),
                    field(Key::NodesAddTarget, target.into()),
                    field(Key::NodesAddConfig, config.into()),
                    row![
                        ui_button::primary(t, ctx.t(Key::NodesAddSubmit))
                            .on_press(press(Msg::AddSubmit)),
                        typography::hint(t, ctx.t(Key::NodesAddHint)),
                    ]
                    .spacing(tokens::space::MD)
                    .align_y(Alignment::Center),
                ]
                .spacing(tokens::space::MD),
            )
            .padding(tokens::space::XL)
            .width(Length::Fill)
            .into(),
        ],
    )
}
