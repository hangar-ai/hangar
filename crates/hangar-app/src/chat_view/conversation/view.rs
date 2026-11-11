//! The conversation, drawn: a header with the orchestrator and what it does,
//! the messages newest at the bottom (the scroll anchored there), and the
//! composer, or in its place the card of the question or permission it waits
//! on.
//!
//! Laid out as a chat reads: the person's messages in bubbles on the right,
//! Claude's words open on the page, its calls quiet lines of the work log —
//! an icon for what they do, what they did, folded in runs — and, while it
//! works, a line that says so at the bottom.

use std::hash::{DefaultHasher, Hash, Hasher};

use hangar_core::chat::{Answer, Body, CallResult, CallState, ChatItem, Question, Sender};
use hangar_i18n::{Key, args};
use hangar_theme::{AgentState, Theme, fonts, style, tokens};
use hangar_ui::{Icon, button as ui_button, empty, icon};
use iced::keyboard::{self, key::Named};
use iced::widget::text_editor::{Binding, KeyPress, Status};
use iced::widget::{
    Space, button, column, container, row, rule, scrollable, text, text_editor, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::ctx::Ctx;
use crate::message::Message;
use crate::mobile::chat_live::PLAN_TOOL;
use crate::mobile::contract::{ChatPending, ChatReason};
use crate::orchestration_ui::actions::can_create_orchestrator;
use crate::state::roles::orchestrator_of;
use crate::state::{AgentActivity, Pane, Workspace};

use super::calls::{self, Kind, Piece};
use super::hangar;
use super::settled::settled;
use super::thread::{Outgoing, Sent};
use super::{Blocked, Chat, Msg, State, blocked, msg, plan_key, rich, titles};
use crate::orchestration::events::DeliveryOutcome;
use crate::orchestration::heard::Heard;

/// The widest the column of messages gets: lines longer than this read badly.
const COLUMN_WIDTH: f32 = 760.0;
/// How wide a bubble of the person's gets, at most.
const BUBBLE_WIDTH: f32 = 580.0;
/// The roundness of a bubble and of the composer.
const SOFT: f32 = 16.0;
/// The lines of a call's detail or result shown expanded, at most.
const DETAIL_LINES: usize = 40;
/// A call's icon, and how far its words sit from the row's edge.
const CALL_ICON: f32 = 13.0;
const CALL_INDENT: f32 = tokens::space::SM + CALL_ICON + tokens::space::MD;

/// The input's widget id.
pub fn input_id() -> iced::widget::Id {
    iced::widget::Id::new("chat-view-conversation-input")
}

pub fn view<'a>(st: &'a State, ctx: Ctx<'a>, ws: &'a Workspace) -> Element<'a, Message> {
    let t = ctx.theme;
    let Some(pane) = orchestrator_of(ws) else {
        return no_orchestrator(ctx, ws);
    };
    let chat = st.chat(&ws.id).filter(|chat| chat.pane == pane.id);
    let Some(chat) = chat.filter(|chat| chat.loaded) else {
        return loading(t, ctx.t(Key::ChatConvLoading).into_owned());
    };
    let body = column![
        header(ctx, pane),
        rule::horizontal(tokens::BORDER_WIDTH).style(style::rule::sep(t)),
        messages(ctx, ws, pane, chat),
        composer(ctx, ws, pane, chat),
    ]
    .height(Length::Fill);
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(style::container::plain(t))
        .into()
}

/// The column, centred, no wider than reads well.
fn centred<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(container(content).max_width(COLUMN_WIDTH))
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

fn loading<'a>(t: &'a Theme, words: String) -> Element<'a, Message> {
    container(hangar_ui::text::hint(t, words))
        .center(Length::Fill)
        .style(style::container::plain(t))
        .into()
}

fn no_orchestrator<'a>(ctx: Ctx<'a>, ws: &'a Workspace) -> Element<'a, Message> {
    let t = ctx.theme;
    let state = empty::empty(t, Icon::Orchestrator, ctx.t(Key::ChatConvEmptyTitle))
        .description(ctx.t(Key::ChatConvEmptyBody));
    if can_create_orchestrator(ws) {
        state
            .action(
                ui_button::primary(t, ctx.t(Key::ChatConvEmptyStart))
                    .icon(Icon::Play)
                    .on_press(msg(Msg::Start(ws.id.clone()))),
            )
            .into()
    } else {
        state.description(ctx.t(Key::ChatConvEmptyCannot)).into()
    }
}

/// `color` at `alpha`.
fn tint(color: Color, alpha: f32) -> Color {
    Color {
        a: color.a * alpha,
        ..color
    }
}

/// A round mark of `color`, `size` across.
fn dot<'a>(color: Color, size: f32) -> Element<'a, Message> {
    container(Space::new())
        .width(size)
        .height(size)
        .style(move |_| container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                radius: (size / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// The orchestrator, how it is doing, and on the right the model it runs and
/// how full its context is.
fn header<'a>(ctx: Ctx<'a>, pane: &'a Pane) -> Element<'a, Message> {
    let t = ctx.theme;
    let logic = &ctx.model.live.panes_logic;
    let activity = logic.activity_or_idle(&pane.id);
    let label = match activity {
        AgentActivity::Working => Key::PaneStateWorking,
        AgentActivity::Waiting => Key::PaneStateWaiting,
        AgentActivity::Yours => Key::PaneStateYoursShort,
        AgentActivity::Idle => Key::PaneStateIdle,
    };
    let state = AgentState::from(activity);
    let pill = container(
        row![
            dot(t.colors.state(state).fill, 7.0),
            text(ctx.t(label))
                .size(tokens::font_size::CAPTION)
                .style(style::text::state(t, state)),
        ]
        .spacing(tokens::space::SM)
        .align_y(Alignment::Center),
    )
    .padding([tokens::space::XXS, tokens::space::MD])
    .style(move |_| container::Style {
        background: Some(Background::Color(tint(t.colors.state(state).fill, 0.12))),
        border: Border {
            radius: tokens::radius::FULL.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let mut line = row![
        icon(Icon::Orchestrator, 15.0).color(t.colors.fg_muted),
        text(pane.name.as_str())
            .size(tokens::font_size::UI_LG)
            .font(fonts::ui_semibold())
            .style(style::text::primary(t)),
        pill,
        Space::new().width(Length::Fill),
    ]
    .spacing(tokens::space::MD)
    .align_y(Alignment::Center);
    let insight = logic.insight(&pane.id);
    let mut meta: Vec<String> = Vec::new();
    if let Some(model) = insight.model.as_deref().filter(|m| !m.trim().is_empty()) {
        meta.push(crate::stats::model::model_label(model));
    }
    if let Some(context) = &insight.context {
        let pct = context.pct.round().clamp(0.0, 100.0) as u32;
        meta.push(
            ctx.i18n
                .t_with(Key::ChatConvContext, &args!("pct" => pct.to_string()))
                .into_owned(),
        );
    }
    if !meta.is_empty() {
        line = line.push(
            text(meta.join(" · "))
                .size(tokens::font_size::CAPTION)
                .style(style::text::subtle(t)),
        );
    }
    container(line)
        .padding([tokens::space::MD, tokens::space::XXL])
        .width(Length::Fill)
        .into()
}

fn messages<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    pane: &'a Pane,
    chat: &'a Chat,
) -> Element<'a, Message> {
    let t = ctx.theme;
    let mut list = column![].spacing(tokens::space::XL).width(Length::Fill);
    if chat.reading_older {
        list = list.push(centre_line(hangar_ui::text::hint(
            t,
            ctx.t(Key::ChatConvOlderLoading),
        )));
    } else if chat.thread.older {
        list = list.push(centre_line(
            ui_button::ghost(t, ctx.t(Key::ChatConvOlder))
                .small()
                .on_press(msg(Msg::LoadOlder(ws.id.clone()))),
        ));
    }
    let tasks = ctx.model.live.board.tasks(&ws.cwd);
    let task_title = move |id: &str| {
        tasks?
            .iter()
            .find(|task| task.id == id)
            .map(|task| task.title.clone())
    };
    let pending_question = match &chat.thread.pending {
        Some(ChatPending::Question { tool_use_id, .. }) => Some(tool_use_id.as_str()),
        _ => None,
    };
    // The question the pane waits on is the card below, not an item.
    let items = chat.thread.items.iter().filter(|item| {
        !matches!(&item.body, Body::Question { tool_use_id, .. }
            if Some(tool_use_id.as_str()) == pending_question)
    });
    for piece in calls::pieces(items) {
        list = list.push(match piece {
            Piece::One(item) => item_view(ctx, ws, chat, item, &task_title),
            Piece::Run(run) => run_view(ctx, ws, chat, &run, &task_title),
        });
    }
    for out in &chat.thread.outgoing {
        list = list.push(outgoing(ctx, pane, out));
    }
    if chat.thread.items.is_empty()
        && chat.thread.outgoing.is_empty()
        && let Some(reason) = chat.thread.reason
    {
        let key = match reason {
            ChatReason::NotClaude => Key::ChatConvReasonNotClaude,
            ChatReason::NoSession => Key::ChatConvReasonNoSession,
            ChatReason::NoTranscript => Key::ChatConvReasonNoTranscript,
        };
        list = list.push(notice(
            t,
            ctx.i18n
                .t_with(key, &args!("name" => pane.name.as_str()))
                .into_owned(),
        ));
    }
    if let Some(error) = &chat.error {
        list = list.push(
            hangar_ui::text::danger(
                t,
                ctx.i18n
                    .t_with(Key::ChatConvReadFailed, &args!("error" => error.as_str()))
                    .into_owned(),
            )
            .size(tokens::font_size::CAPTION),
        );
    }
    let ws_id = ws.id.clone();
    let content = centred(container(list).padding(Padding {
        top: tokens::space::XXXL,
        right: tokens::space::XXL,
        bottom: tokens::space::XL,
        left: tokens::space::XXL,
    }));
    // The working line moves on its own clock: outside the laid-out messages.
    let mut page = column![settled(layout_key(ctx, pane, chat, tasks), content)];
    if let Some(working) = working(ctx, pane, chat) {
        page = page.push(centred(container(working).width(Length::Fill).padding(
            Padding {
                top: 0.0,
                right: tokens::space::XXL,
                bottom: tokens::space::XL,
                left: tokens::space::XXL,
            },
        )));
    }
    scrollable(page)
        .anchor_bottom()
        .on_scroll(move |viewport| msg(Msg::Scrolled(ws_id.clone(), viewport)))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(style::scrollable::default(t))
        .into()
}

/// What the messages' words come from: the chat's own state, and the
/// names, titles and language they are written with, and the theme (a
/// Markdown span's colour is set as it is laid out). Laid out again only
/// when it changes.
fn layout_key(
    ctx: Ctx<'_>,
    pane: &Pane,
    chat: &Chat,
    tasks: Option<&[crate::board::model::Task]>,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    chat.revision.hash(&mut hasher);
    ctx.model.locale().hash(&mut hasher);
    ctx.theme.id.hash(&mut hasher);
    pane.name.hash(&mut hasher);
    for task in tasks.unwrap_or_default() {
        task.id.hash(&mut hasher);
        task.title.hash(&mut hasher);
    }
    hasher.finish()
}

/// `ms` as a short span of time: 8s, 2m 05s, 1h 04m.
pub(super) fn span(ms: u64) -> String {
    let seconds = ms / 1000;
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h {:02}m", seconds / 3600, (seconds % 3600) / 60),
    }
}

/// While the orchestrator works and nothing waits on the person: a line that
/// says so, with how long its turn has run.
fn working<'a>(ctx: Ctx<'a>, pane: &'a Pane, chat: &'a Chat) -> Option<Element<'a, Message>> {
    let t = ctx.theme;
    let logic = &ctx.model.live.panes_logic;
    if logic.activity_or_idle(&pane.id) != AgentActivity::Working || chat.thread.pending.is_some() {
        return None;
    }
    let since = logic.insight(&pane.id).turn_started_at;
    let words = match since {
        Some(since) => ctx
            .i18n
            .t_with(
                Key::ChatConvWorkingFor,
                &args!("time" => span(crate::control::types::now_ms().saturating_sub(since))),
            )
            .into_owned(),
        None => ctx.t(Key::ChatConvWorking).into_owned(),
    };
    Some(
        row![
            dot(t.colors.state_working_fill, 7.0),
            text(words)
                .size(tokens::font_size::CAPTION)
                .style(style::text::state(t, AgentState::Working)),
        ]
        .spacing(tokens::space::MD)
        .align_y(Alignment::Center)
        .padding([0.0, tokens::space::SM])
        .into(),
    )
}

fn centre_line<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).center_x(Length::Fill).into()
}

/// A line across the conversation with words in its middle: compacted,
/// interrupted, resumed.
fn notice<'a>(t: &'a Theme, words: String) -> Element<'a, Message> {
    let line = || {
        container(rule::horizontal(tokens::BORDER_WIDTH).style(style::rule::sep(t)))
            .width(Length::Fill)
    };
    row![
        line(),
        text(words)
            .size(tokens::font_size::CAPTION)
            .style(style::text::subtle(t))
            .align_x(iced::alignment::Horizontal::Center),
        line(),
    ]
    .spacing(tokens::space::LG)
    .align_y(Alignment::Center)
    .into()
}

fn item_view<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    chat: &'a Chat,
    item: &'a ChatItem,
    task_title: &dyn Fn(&str) -> Option<String>,
) -> Element<'a, Message> {
    let t = ctx.theme;
    match &item.body {
        Body::User {
            text: words,
            from: Sender::Person,
            images,
        } => {
            let mut bubble = column![
                text(words.as_str())
                    .size(tokens::font_size::BODY)
                    .line_height(tokens::line_height::BODY)
                    .style(style::text::primary(t))
            ]
            .spacing(tokens::space::XS);
            if *images > 0 {
                bubble = bubble.push(
                    text(ctx.i18n.tn(Key::ChatConvImages, *images, &[]).into_owned())
                        .size(tokens::font_size::CAPTION)
                        .style(style::text::muted(t)),
                );
            }
            mine(t, bubble, false)
        }
        Body::User {
            text: words,
            from: Sender::Hangar,
            ..
        } => hangar_message(ctx, ws, chat, item, words),
        Body::Assistant { .. } => {
            let Some((_, content)) = chat.prose.get(&item.id) else {
                return Space::new().into();
            };
            container(rich::view(t, content, &chat.code))
                .padding([0.0, tokens::space::SM])
                .width(Length::Fill)
                .into()
        }
        Body::Action {
            tool,
            title,
            detail,
            result,
            state,
        } => action(
            ctx,
            ws,
            chat,
            item,
            Action {
                tool,
                title: &titles::title(ctx.i18n, tool, title, detail.as_deref(), task_title),
                detail: detail.as_deref(),
                result: result.as_ref(),
                state: *state,
            },
        ),
        Body::Question {
            questions, answer, ..
        } => asked(ctx, questions, answer.as_deref()),
        Body::Notice { text: words } => notice(t, words.clone()),
    }
}

/// The icon of a call: what kind of thing it does.
fn call_icon(tool: &str) -> Icon {
    match calls::kind(tool) {
        Kind::Command => Icon::Terminal,
        Kind::Edit => Icon::Pencil,
        Kind::Read => Icon::Folder,
        Kind::Search => Icon::Search,
        Kind::Task => Icon::Board,
        Kind::Memory => Icon::Bookmark,
        Kind::Message => Icon::Bubble,
        Kind::Other if tool.starts_with("mcp__hangar__") => Icon::Orchestrator,
        Kind::Other if tool.starts_with("mcp__") => Icon::Plug,
        Kind::Other => Icon::Bolt,
    }
}

/// A run of calls, folded into one quiet line saying what they did; the
/// calls, each its own line as alone, once pressed.
fn run_view<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    chat: &'a Chat,
    run: &[&'a ChatItem],
    task_title: &dyn Fn(&str) -> Option<String>,
) -> Element<'a, Message> {
    let t = ctx.theme;
    let Some(first) = run.first() else {
        return Space::new().into();
    };
    let key = Piece::key(first);
    let open = chat.expanded.contains(&key);
    let tally = calls::tally(run);
    let mut line = row![
        container(
            icon(
                if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                11.0,
            )
            .color(t.colors.fg_subtle)
        )
        .width(CALL_ICON)
        .center_x(CALL_ICON),
        text(calls::summary(ctx.i18n, &tally))
            .size(tokens::font_size::CAPTION)
            .style(style::text::muted(t)),
    ]
    .spacing(tokens::space::MD)
    .align_y(Alignment::Center);
    if tally.failed > 0 {
        line = line.push(
            text(format!(
                "· {}",
                ctx.i18n.tn(Key::ChatConvCallsFailed, tally.failed, &[])
            ))
            .size(tokens::font_size::CAPTION)
            .color(t.colors.warning),
        );
    }
    if tally.running > 0 {
        line = line.push(
            text(format!(
                "· {}",
                ctx.i18n.tn(Key::ChatConvCallsRunning, tally.running, &[])
            ))
            .size(tokens::font_size::CAPTION)
            .color(t.colors.state_working),
        );
    }
    let head = button(line)
        .padding([tokens::space::XS, tokens::space::SM])
        .width(Length::Fill)
        .style(style::button::row(t, false))
        .on_press(msg(Msg::Toggle(ws.id.clone(), key)));
    if !open {
        return head.into();
    }
    let mut list = column![].spacing(tokens::space::XXS);
    for item in run {
        list = list.push(item_view(ctx, ws, chat, item, task_title));
    }
    column![
        head,
        container(list).padding(Padding {
            left: tokens::space::XL,
            ..Padding::ZERO
        })
    ]
    .spacing(tokens::space::XXS)
    .into()
}

/// A bubble of the person's, on the right.
fn mine<'a>(
    t: &'a Theme,
    content: impl Into<Element<'a, Message>>,
    faded: bool,
) -> Element<'a, Message> {
    let bubble = container(content)
        .padding([tokens::space::LG, tokens::space::XL + tokens::space::XXS])
        .max_width(BUBBLE_WIDTH)
        .style(move |_| container::Style {
            background: Some(Background::Color(tint(
                t.colors.raised,
                if faded { 0.6 } else { 1.0 },
            ))),
            border: Border {
                color: t.colors.line,
                width: tokens::BORDER_WIDTH,
                radius: SOFT.into(),
            },
            text_color: Some(t.colors.fg),
            ..container::Style::default()
        });
    row![Space::new().width(Length::Fill), bubble].into()
}

/// How long the first line of a message gets in its one line.
const NOTE_WORDS: usize = 140;

/// A line of the workspace's news in the person's words: its icon and its
/// colour, by what it says.
fn heard_line(ctx: Ctx<'_>, heard: &Heard) -> (Icon, Color, String) {
    let t = ctx.theme;
    let c = &t.colors;
    let say = |key: Key, args: &[(&str, &str)]| {
        let list: Vec<(&str, hangar_i18n::Arg)> = args
            .iter()
            .map(|(name, value)| (*name, hangar_i18n::Arg::from(*value)))
            .collect();
        ctx.i18n.t_with(key, &list).into_owned()
    };
    let column = |id: &str| {
        crate::board::model::Column::from_id(id)
            .map(|column| ctx.t(column.label_key()).into_owned())
            .unwrap_or_else(|| id.to_string())
    };
    match heard {
        Heard::Delivery {
            task,
            worker,
            outcome,
        } => {
            let (glyph, colour, key) = match outcome {
                DeliveryOutcome::Landed => (Icon::Check, c.success, Key::ChatConvHeardLanded),
                DeliveryOutcome::Done => (Icon::Check, c.success, Key::ChatConvHeardDone),
                DeliveryOutcome::Ready => (Icon::Merge, c.accent_hi, Key::ChatConvHeardReady),
                DeliveryOutcome::Conflict => (Icon::Alert, c.warning, Key::ChatConvHeardConflict),
                DeliveryOutcome::CheckFailed => {
                    (Icon::Alert, c.warning, Key::ChatConvHeardCheckFailed)
                }
                DeliveryOutcome::ChangesRequested => {
                    (Icon::Alert, c.warning, Key::ChatConvHeardChangesRequested)
                }
                DeliveryOutcome::Dirty => (Icon::Alert, c.warning, Key::ChatConvHeardDirty),
                DeliveryOutcome::Stale => (Icon::Alert, c.warning, Key::ChatConvHeardStale),
                DeliveryOutcome::Error => (Icon::Alert, c.danger, Key::ChatConvHeardFailed),
            };
            (
                glyph,
                colour,
                say(key, &[("worker", worker.as_str()), ("task", task.as_str())]),
            )
        }
        Heard::NeedsYou { task, reason } => (
            Icon::Bell,
            c.state_waiting,
            say(
                Key::ChatConvHeardNeedsYou,
                &[("task", task.as_str()), ("reason", reason.as_str())],
            ),
        ),
        Heard::Blocked { task, by, reason } => {
            let words = match by {
                Some(who) => say(
                    Key::ChatConvHeardBlocked,
                    &[
                        ("who", who.as_str()),
                        ("task", task.as_str()),
                        ("reason", reason.as_str()),
                    ],
                ),
                None => say(
                    Key::ChatConvHeardBlockedByYou,
                    &[("task", task.as_str()), ("reason", reason.as_str())],
                ),
            };
            (Icon::Lock, c.warning, words)
        }
        Heard::Unblocked { task } => (
            Icon::Lock,
            c.fg_muted,
            say(Key::ChatConvHeardUnblocked, &[("task", task.as_str())]),
        ),
        Heard::RecheckDue { task } => (
            Icon::Clock,
            c.warning,
            say(Key::ChatConvHeardRecheck, &[("task", task.as_str())]),
        ),
        Heard::Commented { task, by, text } => {
            let words = match by {
                Some(who) => say(
                    Key::ChatConvHeardCommented,
                    &[
                        ("who", who.as_str()),
                        ("task", task.as_str()),
                        ("text", text.as_str()),
                    ],
                ),
                None => say(
                    Key::ChatConvHeardCommentedByYou,
                    &[("task", task.as_str()), ("text", text.as_str())],
                ),
            };
            (Icon::Bubble, c.fg_muted, words)
        }
        Heard::Moved { task, from, to } => (
            Icon::ArrowRight,
            c.fg_muted,
            say(
                Key::ChatConvHeardMoved,
                &[
                    ("task", task.as_str()),
                    ("from", &column(from)),
                    ("to", &column(to)),
                ],
            ),
        ),
        Heard::Started { worker, task } => {
            let words = match task {
                Some(task) => say(
                    Key::ChatConvHeardStartedOn,
                    &[("worker", worker.as_str()), ("task", task.as_str())],
                ),
                None => say(Key::ChatConvHeardStarted, &[("worker", worker.as_str())]),
            };
            (Icon::Play, c.state_working, words)
        }
        Heard::Stopped { worker, .. } => (
            Icon::Stop,
            c.fg_muted,
            say(Key::ChatConvHeardStopped, &[("worker", worker.as_str())]),
        ),
        Heard::Exited { worker, .. } => (
            Icon::Close,
            c.fg_muted,
            say(Key::ChatConvHeardExited, &[("worker", worker.as_str())]),
        ),
        Heard::Waiting {
            worker, minutes, ..
        } => (
            Icon::Bell,
            c.state_waiting,
            say(
                Key::ChatConvHeardWaiting,
                &[
                    ("worker", worker.as_str()),
                    ("minutes", &minutes.to_string()),
                ],
            ),
        ),
        Heard::Idle { worker, .. } => (
            Icon::Pause,
            c.warning,
            say(Key::ChatConvHeardIdle, &[("worker", worker.as_str())]),
        ),
    }
}

/// One line of what Hangar said: an icon, the words, and when there is more
/// to read, the chevron that opens it.
fn note_line<'a>(
    t: &'a Theme,
    glyph: Icon,
    colour: Color,
    words: String,
    toggle: Option<(bool, Message)>,
) -> Element<'a, Message> {
    let mut line = row![
        container(icon(glyph, CALL_ICON).color(colour))
            .width(CALL_ICON)
            .center_x(CALL_ICON),
        text(words)
            .size(tokens::font_size::CAPTION)
            .width(Length::Fill)
            .style(style::text::secondary(t)),
    ]
    .spacing(tokens::space::MD)
    .align_y(Alignment::Center);
    let Some((open, press)) = toggle else {
        return container(line)
            .padding([tokens::space::XS, tokens::space::SM])
            .into();
    };
    line = line.push(
        icon(
            if open {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            },
            11.0,
        )
        .color(t.colors.fg_subtle),
    );
    button(line)
        .padding([tokens::space::XS, tokens::space::SM])
        .width(Length::Fill)
        .style(style::button::row(t, false))
        .on_press(press)
        .into()
}

/// A message Hangar put in the orchestrator's conversation, compact: a line
/// per thing that happened in the workspace, in the person's words; or one
/// line for a message from another agent, the state of the work handed to a
/// conversation that starts, or anything else, the whole of it a click away.
fn hangar_message<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    chat: &'a Chat,
    item: &'a ChatItem,
    words: &'a str,
) -> Element<'a, Message> {
    let t = ctx.theme;
    let open = chat.expanded.contains(&item.id);
    let toggle = || (open, msg(Msg::Toggle(ws.id.clone(), item.id.clone())));
    let (head, whole): (Element<'a, Message>, Option<String>) = match hangar::note(words) {
        hangar::Note::News(lines) => {
            let mut list = column![].spacing(tokens::space::XXS);
            for line in &lines {
                let (glyph, colour, words) = match line {
                    hangar::Line::Heard(heard) => heard_line(ctx, heard),
                    hangar::Line::Said(words) => {
                        (Icon::Broadcast, t.colors.fg_muted, words.clone())
                    }
                };
                list = list.push(note_line(t, glyph, colour, words, None));
            }
            return list.into();
        }
        hangar::Note::Agent { from, text: said } => {
            let line = hangar::first_line(&said, NOTE_WORDS);
            let more = said.trim() != line;
            let words = ctx
                .i18n
                .t_with(
                    Key::ChatConvHeardMessage,
                    &args!("from" => from.as_str(), "text" => line.as_str()),
                )
                .into_owned();
            (
                note_line(t, Icon::Bubble, t.colors.fg_muted, words, more.then(toggle)),
                more.then_some(said),
            )
        }
        hangar::Note::Briefing(state) => {
            let name = orchestrator_of(ws).map_or("", |pane| pane.name.as_str());
            let words = ctx
                .i18n
                .t_with(Key::ChatConvHeardBriefing, &args!("name" => name))
                .into_owned();
            let more = !state.trim().is_empty();
            (
                note_line(t, Icon::Board, t.colors.fg_muted, words, more.then(toggle)),
                more.then_some(state),
            )
        }
        hangar::Note::Other(said) => {
            let line = hangar::first_line(&said, NOTE_WORDS);
            let more = said.trim() != line;
            (
                note_line(
                    t,
                    Icon::Broadcast,
                    t.colors.fg_muted,
                    line,
                    more.then(toggle),
                ),
                more.then_some(said),
            )
        }
    };
    match whole.filter(|_| open) {
        Some(whole) => column![
            head,
            container(
                text(whole)
                    .size(tokens::font_size::CAPTION)
                    .line_height(tokens::line_height::BODY)
                    .style(style::text::muted(t)),
            )
            .padding(Padding {
                top: 0.0,
                right: tokens::space::SM,
                bottom: tokens::space::SM,
                left: CALL_INDENT,
            })
        ]
        .into(),
        None => head,
    }
}

struct Action<'b> {
    tool: &'b str,
    title: &'b str,
    detail: Option<&'b str>,
    result: Option<&'b CallResult>,
    state: CallState,
}

/// The name of a tool as a chip shows it: an MCP tool's own name.
fn tool_name(tool: &str) -> &str {
    tool.strip_prefix("mcp__")
        .and_then(|rest| rest.split_once("__"))
        .map_or(tool, |(_, name)| name)
}

/// Lines `at_most` of `words`, the rest said to be there.
fn clipped(words: &str, at_most: usize) -> String {
    let count = words.lines().count();
    if count <= at_most {
        return words.to_string();
    }
    let head: Vec<&str> = words.lines().take(at_most).collect();
    format!("{}\n…", head.join("\n"))
}

/// A call, a line of the work log: what kind of thing it does, what it did,
/// how it went; its detail below once pressed.
fn action<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    chat: &'a Chat,
    item: &'a ChatItem,
    call: Action<'_>,
) -> Element<'a, Message> {
    let t = ctx.theme;
    let (colour, hint) = match call.state {
        CallState::Running => (
            t.colors.state_working,
            Some(ctx.t(Key::ChatConvActionRunning)),
        ),
        CallState::Done => (t.colors.fg_muted, None),
        CallState::Error => (t.colors.danger, Some(ctx.t(Key::ChatConvActionFailed))),
    };
    let more = call.detail.is_some() || call.result.is_some_and(|r| !r.text.trim().is_empty());
    let open = more && chat.expanded.contains(&item.id);
    let mut line = row![
        container(icon(call_icon(call.tool), CALL_ICON).color(colour))
            .width(CALL_ICON)
            .center_x(CALL_ICON),
        container(
            text(call.title.to_string())
                .size(tokens::font_size::UI)
                .style(style::text::secondary(t)),
        )
        .width(Length::Fill),
    ]
    .spacing(tokens::space::MD)
    .align_y(Alignment::Center);
    if let Some(hint) = hint {
        line = line.push(text(hint).size(tokens::font_size::MICRO).color(colour));
    }
    if more {
        line = line.push(
            icon(
                if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                11.0,
            )
            .color(t.colors.fg_subtle),
        );
    }
    let head = button(line)
        .padding([tokens::space::XS, tokens::space::SM])
        .width(Length::Fill)
        .style(style::button::row(t, false))
        .on_press_maybe(more.then(|| msg(Msg::Toggle(ws.id.clone(), item.id.clone()))));
    if !open {
        return head.into();
    }
    let mut detail = column![].spacing(tokens::space::SM);
    if let Some(words) = call.detail {
        detail = detail.push(mono_block(
            t,
            clipped(&titles::detail(call.tool, words), DETAIL_LINES),
        ));
    }
    if let Some(result) = call.result.filter(|r| !r.text.trim().is_empty()) {
        detail = detail.push(
            text(ctx.t(Key::ChatConvActionResult))
                .size(tokens::font_size::MICRO)
                .font(fonts::ui_semibold())
                .style(style::text::subtle(t)),
        );
        let block = mono_block(t, clipped(&result.text, DETAIL_LINES));
        detail = detail.push(block);
        if !result.ok {
            detail = detail.push(
                text(ctx.t(Key::ChatConvActionFailed))
                    .size(tokens::font_size::MICRO)
                    .style(style::text::danger(t)),
            );
        }
    }
    column![
        head,
        container(detail).padding(Padding {
            top: tokens::space::XXS,
            right: tokens::space::SM,
            bottom: tokens::space::SM,
            left: CALL_INDENT,
        })
    ]
    .into()
}

fn mono_block<'a>(t: &'a Theme, words: String) -> Element<'a, Message> {
    container(
        text(words)
            .size(tokens::font_size::LABEL)
            .font(fonts::mono())
            .style(style::text::secondary(t)),
    )
    .padding([tokens::space::MD, tokens::space::LG])
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(t.colors.bg_sunken)),
        border: Border {
            color: t.colors.line,
            width: tokens::BORDER_WIDTH,
            radius: tokens::radius::LG.into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// A question asked and answered, or left without an answer.
fn asked<'a>(
    ctx: Ctx<'a>,
    questions: &'a [Question],
    answers: Option<&'a [Answer]>,
) -> Element<'a, Message> {
    let t = ctx.theme;
    let mut stack = column![].spacing(tokens::space::XS);
    for (at, question) in questions.iter().enumerate() {
        stack = stack.push(
            text(
                ctx.i18n
                    .t_with(
                        Key::ChatConvQuestionAsked,
                        &args!("question" => question.question.as_str()),
                    )
                    .into_owned(),
            )
            .size(tokens::font_size::UI)
            .style(style::text::secondary(t)),
        );
        let answer = answers.and_then(|answers| answers.get(at));
        let words: Vec<String> = answer
            .map(|answer| {
                answer
                    .selected
                    .iter()
                    .cloned()
                    .chain(answer.other.clone())
                    .collect()
            })
            .unwrap_or_default();
        let line = if words.is_empty() {
            text(ctx.t(Key::ChatConvQuestionNoAnswer))
                .size(tokens::font_size::CAPTION)
                .style(style::text::subtle(t))
        } else {
            text(format!("→ {}", words.join(", ")))
                .size(tokens::font_size::CAPTION)
                .style(style::text::accent(t))
        };
        stack = stack.push(line);
    }
    row![
        container(icon(Icon::Bubble, CALL_ICON).color(t.colors.fg_muted))
            .width(CALL_ICON)
            .center_x(CALL_ICON),
        container(stack).width(Length::Fill),
    ]
    .spacing(tokens::space::MD)
    .padding([tokens::space::XS, tokens::space::SM])
    .into()
}

/// A message sent from here the transcript does not have yet.
fn outgoing<'a>(ctx: Ctx<'a>, pane: &'a Pane, out: &'a Outgoing) -> Element<'a, Message> {
    let t = ctx.theme;
    let state = match &out.state {
        Sent::Sending => Some(
            text(ctx.t(Key::ChatConvSending))
                .size(tokens::font_size::MICRO)
                .style(style::text::muted(t)),
        ),
        Sent::Typed => None,
        Sent::Queued => Some(
            text(
                ctx.i18n
                    .t_with(Key::ChatConvQueued, &args!("name" => pane.name.as_str()))
                    .into_owned(),
            )
            .size(tokens::font_size::MICRO)
            .style(style::text::muted(t)),
        ),
        Sent::Failed(error) => Some(
            text(
                ctx.i18n
                    .t_with(Key::ChatConvNotSent, &args!("error" => error.as_str()))
                    .into_owned(),
            )
            .size(tokens::font_size::MICRO)
            .style(style::text::danger(t)),
        ),
    };
    let mut bubble = column![
        text(out.text.as_str())
            .size(tokens::font_size::BODY)
            .line_height(tokens::line_height::BODY)
            .style(style::text::primary(t))
    ]
    .spacing(tokens::space::XS);
    if let Some(state) = state {
        bubble = bubble.push(state);
    }
    mine(t, bubble, !matches!(out.state, Sent::Typed))
}

/// The tallest the body of a card gets: past it, it scrolls, and its
/// title and buttons stay in sight.
const CARD_BODY_HEIGHT: f32 = 340.0;

/// A plan is read before it is approved: its card gets more room.
const PLAN_BODY_HEIGHT: f32 = 480.0;

/// A card's parts: its title row, what it shows (scrolled when tall) and
/// its buttons (always shown).
struct CardParts<'a> {
    title: Element<'a, Message>,
    body: Element<'a, Message>,
    actions: Element<'a, Message>,
    /// The body's scroll, per question shown: a new one starts at its top.
    scroll: iced::widget::Id,
}

/// The card of what the pane waits on the person for, in the composer's
/// place, tinted with the colour of waiting.
fn card<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    pane: &'a Pane,
    chat: &'a Chat,
) -> Option<Element<'a, Message>> {
    let t = ctx.theme;
    let parts = match chat.thread.pending.as_ref()? {
        ChatPending::Question { questions, .. } => question_card(ctx, ws, pane, chat, questions),
        ChatPending::Permission {
            tool,
            plan: Some(_),
            prompt,
            ..
        } if tool == PLAN_TOOL => plan_card(ctx, ws, pane, chat, prompt),
        ChatPending::Permission {
            tool,
            title,
            detail,
            ..
        } => permission_card(ctx, ws, pane, chat, tool, title, detail.as_deref()),
    };
    let tall = matches!(
        chat.thread.pending,
        Some(ChatPending::Permission { ref tool, plan: Some(_), .. }) if tool == PLAN_TOOL
    );
    let body = container(
        scrollable(container(parts.body).padding(Padding {
            right: tokens::space::LG,
            ..Padding::ZERO
        }))
        .id(parts.scroll)
        .height(Length::Shrink)
        .style(style::scrollable::default(t)),
    )
    .max_height(if tall {
        PLAN_BODY_HEIGHT
    } else {
        CARD_BODY_HEIGHT
    });
    let mut stack = column![parts.title, body].spacing(tokens::space::LG);
    // What happened to the last answer, right above the buttons: never out
    // of sight.
    if chat.deciding {
        stack = stack.push(hangar_ui::text::hint(t, ctx.t(Key::ChatConvBusy)));
    }
    if let Some(refusal) = &chat.refusal {
        stack = stack.push(
            hangar_ui::text::danger(
                t,
                ctx.i18n
                    .t_with(Key::ChatConvRefused, &args!("error" => refusal.as_str()))
                    .into_owned(),
            )
            .size(tokens::font_size::CAPTION),
        );
    }
    stack = stack.push(parts.actions);
    Some(
        container(stack)
            .padding([tokens::space::XL, tokens::space::XL])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(tint(t.colors.state_waiting_fill, 0.07))),
                border: Border {
                    radius: SOFT.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into(),
    )
}

fn card_title<'a>(t: &'a Theme, words: String) -> Element<'a, Message> {
    row![
        icon(Icon::Bell, 14.0).color(t.colors.state_waiting),
        text(words)
            .size(tokens::font_size::UI_LG)
            .font(fonts::ui_semibold())
            .style(style::text::primary(t)),
    ]
    .spacing(tokens::space::MD)
    .align_y(Alignment::Center)
    .into()
}

/// A question's header, as a chip.
fn header_chip<'a>(t: &'a Theme, header: &'a str) -> Element<'a, Message> {
    container(
        text(header)
            .size(tokens::font_size::MICRO)
            .style(style::text::muted(t)),
    )
    .padding([0.0, tokens::space::SM])
    .style(style::container::chip(t))
    .into()
}

/// An option of a question, chosen or not.
fn choice<'a>(
    t: &'a Theme,
    label: String,
    description: Option<&'a str>,
    multi: bool,
    on: bool,
    press: Option<Message>,
) -> Element<'a, Message> {
    let mark = match (multi, on) {
        (_, true) => Icon::Check,
        (true, false) => Icon::Minus,
        (false, false) => Icon::ChevronRight,
    };
    let mut words = column![
        text(label)
            .size(tokens::font_size::UI)
            .style(style::text::primary(t))
    ]
    .width(Length::Fill);
    if let Some(description) = description.filter(|d| !d.trim().is_empty()) {
        words = words.push(
            text(description)
                .size(tokens::font_size::CAPTION)
                .style(style::text::muted(t)),
        );
    }
    button(
        row![
            icon(mark, 12.0).color(if on {
                t.colors.accent
            } else {
                t.colors.fg_subtle
            }),
            words
        ]
        .spacing(tokens::space::MD)
        .align_y(Alignment::Center),
    )
    .padding([tokens::space::MD, tokens::space::LG])
    .width(Length::Fill)
    .style(style::button::toggle(t, on))
    .on_press_maybe(press)
    .into()
}

/// The question card: one question at a time, as Claude Code asks them —
/// its header and words, its options and "Other", the step it is (1/3),
/// Back and Next, and Send on the last one. Nothing of the answers given
/// before: Back goes to them.
fn question_card<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    pane: &'a Pane,
    chat: &'a Chat,
    questions: &'a [Question],
) -> CardParts<'a> {
    let t = ctx.theme;
    let form = &chat.form;
    let free = !chat.deciding;
    let count = questions.len();
    let at = form.step();
    let last = at + 1 >= count;
    let mut title = row![
        container(card_title(
            t,
            ctx.i18n
                .t_with(
                    Key::ChatConvQuestionTitle,
                    &args!("name" => pane.name.as_str())
                )
                .into_owned(),
        ))
        .width(Length::Fill)
    ]
    .align_y(Alignment::Center);
    if count > 1 {
        title = title.push(
            text(format!("{}/{count}", at + 1))
                .size(tokens::font_size::CAPTION)
                .font(fonts::mono())
                .style(style::text::muted(t)),
        );
    }
    let mut block = column![].spacing(tokens::space::SM);
    if let Some(question) = questions.get(at) {
        if !question.header.trim().is_empty() {
            block = block.push(header_chip(t, question.header.as_str()));
        }
        block = block.push(
            text(question.question.as_str())
                .size(tokens::font_size::BODY)
                .width(Length::Fill)
                .style(style::text::primary(t)),
        );
        if question.multi_select {
            block = block.push(hangar_ui::text::hint(t, ctx.t(Key::ChatConvQuestionMulti)));
        }
        let picked = form.picked.get(at).map(Vec::as_slice).unwrap_or(&[]);
        for (option_at, option) in question.options.iter().enumerate() {
            block = block.push(choice(
                t,
                option.label.clone(),
                Some(option.description.as_str()),
                question.multi_select,
                picked.contains(&option_at),
                free.then(|| {
                    msg(Msg::Pick {
                        ws: ws.id.clone(),
                        question: at,
                        option: option_at,
                    })
                }),
            ));
        }
        let other_on = form.other_on.get(at).copied().unwrap_or(false);
        block = block.push(choice(
            t,
            ctx.t(Key::ChatConvQuestionOther).into_owned(),
            None,
            question.multi_select,
            other_on,
            free.then(|| {
                msg(Msg::Other {
                    ws: ws.id.clone(),
                    question: at,
                })
            }),
        ));
        if other_on {
            let words = form.other.get(at).map(String::as_str).unwrap_or("");
            let ws_id = ws.id.clone();
            let field = text_input(&ctx.t(Key::ChatConvQuestionOtherPlaceholder), words)
                .size(tokens::font_size::UI)
                .padding([tokens::space::SM, tokens::space::MD])
                .style(style::text_input::field(t))
                .on_input_maybe(free.then_some(move |text| {
                    msg(Msg::OtherText {
                        ws: ws_id.clone(),
                        question: at,
                        text,
                    })
                }));
            block = block.push(crate::keymap::guard(ctx, field));
        }
    }
    let answered = form.answered(questions, at);
    let mut actions = row![].spacing(tokens::space::MD).align_y(Alignment::Center);
    if at > 0 {
        actions = actions.push(
            ui_button::secondary(t, ctx.t(Key::ChatConvQuestionBack)).on_press_maybe(free.then(
                || {
                    msg(Msg::Step {
                        ws: ws.id.clone(),
                        to: at - 1,
                    })
                },
            )),
        );
    }
    // Why the button waits, said rather than a button that does nothing.
    let waits: Element<'a, Message> = if answered {
        Space::new().width(Length::Fill).into()
    } else {
        container(hangar_ui::text::hint(
            t,
            ctx.t(Key::ChatConvQuestionNeedsAnswer),
        ))
        .width(Length::Fill)
        .into()
    };
    actions = actions.push(waits);
    actions = actions.push(if last {
        let ready = form.answers(questions).is_some();
        ui_button::primary(t, ctx.t(Key::ChatConvQuestionSubmit))
            .icon(Icon::Send)
            .on_press_maybe((ready && free).then(|| msg(Msg::SubmitAnswer(ws.id.clone()))))
    } else {
        ui_button::primary(t, ctx.t(Key::ChatConvQuestionNext))
            .icon_after(Icon::ChevronRight)
            .on_press_maybe((answered && free).then(|| {
                msg(Msg::Step {
                    ws: ws.id.clone(),
                    to: at + 1,
                })
            }))
    });
    CardParts {
        title: title.into(),
        body: block.into(),
        actions: actions.into(),
        scroll: iced::widget::Id::from(format!("chat-view-question-{}-{at}", form.tool_use_id)),
    }
}

/// The plan the pane presents, read as Claude's messages are, then
/// approved, or sent back to keep planning with what to change in it.
fn plan_card<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    pane: &'a Pane,
    chat: &'a Chat,
    prompt: &'a str,
) -> CardParts<'a> {
    let t = ctx.theme;
    let free = !chat.deciding;
    let body: Element<'a, Message> = match chat.prose.get(&plan_key(prompt)) {
        Some((_, content)) => rich::view(t, content, &chat.code),
        None => Space::new().into(),
    };
    let ws_id = ws.id.clone();
    let feedback = text_input(&ctx.t(Key::ChatConvPlanFeedback), &chat.plan_feedback)
        .size(tokens::font_size::UI)
        .padding([tokens::space::SM, tokens::space::MD])
        .width(Length::Fill)
        .style(style::text_input::field(t))
        .on_input_maybe(free.then_some(move |words| msg(Msg::PlanFeedback(ws_id.clone(), words))))
        .on_submit_maybe(free.then(|| msg(Msg::Plan(ws.id.clone(), false))));
    let actions = column![
        crate::keymap::guard(ctx, feedback),
        row![
            Space::new().width(Length::Fill),
            ui_button::secondary(t, ctx.t(Key::ChatConvPlanKeep))
                .on_press_maybe(free.then(|| msg(Msg::Plan(ws.id.clone(), false)))),
            ui_button::primary(t, ctx.t(Key::ChatConvPlanApprove))
                .icon(Icon::Check)
                .on_press_maybe(free.then(|| msg(Msg::Plan(ws.id.clone(), true)))),
        ]
        .spacing(tokens::space::MD)
        .align_y(Alignment::Center),
    ]
    .spacing(tokens::space::MD);
    CardParts {
        title: card_title(
            t,
            ctx.i18n
                .t_with(Key::ChatConvPlanTitle, &args!("name" => pane.name.as_str()))
                .into_owned(),
        ),
        body,
        actions: actions.into(),
        scroll: iced::widget::Id::from(format!("chat-view-plan-{prompt}")),
    }
}

fn permission_card<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    pane: &'a Pane,
    chat: &'a Chat,
    tool: &'a str,
    title: &'a str,
    detail: Option<&'a str>,
) -> CardParts<'a> {
    let t = ctx.theme;
    let free = !chat.deciding;
    let tasks = ctx.model.live.board.tasks(&ws.cwd);
    let task_title = move |id: &str| {
        tasks?
            .iter()
            .find(|task| task.id == id)
            .map(|task| task.title.clone())
    };
    let what = titles::title(ctx.i18n, tool, title, detail, &task_title);
    let mut body = column![
        row![
            container(icon(call_icon(tool), CALL_ICON).color(t.colors.fg_muted))
                .width(CALL_ICON)
                .center_x(CALL_ICON),
            text(what)
                .size(tokens::font_size::UI_LG)
                .width(Length::Fill)
                .style(style::text::secondary(t)),
        ]
        .spacing(tokens::space::MD)
        .align_y(Alignment::Center),
    ]
    .spacing(tokens::space::MD);
    if let Some(detail) = detail {
        body = body.push(mono_block(
            t,
            clipped(&titles::detail(tool, detail), DETAIL_LINES),
        ));
    }
    let actions = row![
        Space::new().width(Length::Fill),
        ui_button::secondary(t, ctx.t(Key::ChatConvPermissionDeny))
            .on_press_maybe(free.then(|| msg(Msg::Permit(ws.id.clone(), false)))),
        ui_button::primary(t, ctx.t(Key::ChatConvPermissionAllow))
            .icon(Icon::Check)
            .on_press_maybe(free.then(|| msg(Msg::Permit(ws.id.clone(), true)))),
    ]
    .spacing(tokens::space::MD)
    .align_y(Alignment::Center);
    CardParts {
        title: card_title(
            t,
            ctx.i18n
                .t_with(
                    Key::ChatConvPermissionTitle,
                    &args!("name" => pane.name.as_str(), "tool" => tool_name(tool)),
                )
                .into_owned(),
        ),
        body: body.into(),
        actions: actions.into(),
        scroll: iced::widget::Id::new("chat-view-permission"),
    }
}

/// The words that say why the input is disabled.
pub fn blocked_words(ctx: Ctx<'_>, pane: &Pane, why: Blocked) -> String {
    let name = args!("name" => pane.name.as_str());
    match why {
        Blocked::Loading => ctx.t(Key::ChatConvDisabledLoading).into_owned(),
        Blocked::NotClaude => ctx
            .i18n
            .t_with(Key::ChatConvDisabledNotClaude, &name)
            .into_owned(),
        Blocked::Answering => ctx
            .i18n
            .t_with(Key::ChatConvDisabledAnswering, &name)
            .into_owned(),
        Blocked::Resuming => ctx
            .i18n
            .t_with(Key::ChatConvDisabledResuming, &name)
            .into_owned(),
        Blocked::Question => ctx.t(Key::ChatConvDisabledQuestion).into_owned(),
        Blocked::Permission => ctx.t(Key::ChatConvDisabledPermission).into_owned(),
    }
}

/// Enter sends, Shift+Enter breaks the line; everything else as the editor
/// does it.
fn keys(ws: String) -> impl Fn(KeyPress) -> Option<Binding<Message>> {
    move |press: KeyPress| {
        let enter = matches!(press.key, keyboard::Key::Named(Named::Enter));
        if enter && matches!(press.status, Status::Focused { .. }) && !press.modifiers.shift() {
            return Some(Binding::Custom(msg(Msg::Send(ws.clone()))));
        }
        Binding::from_key_press(press)
    }
}

/// The editor inside the composer: no field of its own, the composer is.
fn bare_editor(t: &Theme) -> impl Fn(&iced::Theme, Status) -> text_editor::Style + '_ {
    move |_, status| {
        let c = &t.colors;
        let disabled = matches!(status, Status::Disabled);
        let fade = |color: Color| tint(color, if disabled { 0.6 } else { 1.0 });
        text_editor::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: fade(c.fg_subtle),
            value: fade(c.fg),
            selection: c.accent_ring,
        }
    }
}

/// What a round button shows: an icon, or Stop's square.
enum Glyph {
    Icon(Icon),
    Square,
}

/// A round button with just a glyph: Send, Stop.
fn round<'a>(
    t: &'a Theme,
    glyph: Glyph,
    primary: bool,
    press: Option<Message>,
) -> Element<'a, Message> {
    let on = press.is_some();
    let colour = match (primary, on) {
        (true, true) => t.colors.accent_fg,
        (true, false) => tint(t.colors.accent_fg, 0.7),
        (false, true) => t.colors.fg,
        (false, false) => t.colors.fg_subtle,
    };
    let mark: Element<'a, Message> = match glyph {
        Glyph::Icon(glyph) => icon(glyph, 14.0).color(colour).into(),
        Glyph::Square => container(Space::new())
            .width(10.0)
            .height(10.0)
            .style(move |_| container::Style {
                background: Some(Background::Color(colour)),
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into(),
    };
    button(container(mark).center_x(30.0).center_y(30.0))
        .padding(0)
        .style(move |theme, status| {
            let mut look = if primary {
                style::button::primary(t)(theme, status)
            } else {
                style::button::secondary(t)(theme, status)
            };
            look.border.radius = tokens::radius::FULL.into();
            if primary && !on {
                look.background = Some(Background::Color(tint(t.colors.accent, 0.35)));
            }
            look
        })
        .on_press_maybe(press)
        .into()
}

/// The composer: the input, and under it what to know before sending, Stop
/// while the orchestrator works, Send; or, while a question or a permission
/// waits, its card instead.
fn composer<'a>(
    ctx: Ctx<'a>,
    ws: &'a Workspace,
    pane: &'a Pane,
    chat: &'a Chat,
) -> Element<'a, Message> {
    let t = ctx.theme;
    let why = blocked(chat, pane);
    let placeholder = ctx
        .i18n
        .t_with(
            Key::ChatConvPlaceholder,
            &args!("name" => pane.name.as_str()),
        )
        .into_owned();
    let mut editor = text_editor(&chat.draft)
        .id(input_id())
        .placeholder(placeholder)
        .size(tokens::font_size::BODY)
        .padding(Padding {
            top: tokens::space::XL,
            right: tokens::space::XL,
            bottom: tokens::space::XS,
            left: tokens::space::XL,
        })
        .min_height(40.0)
        .max_height(220.0)
        .style(bare_editor(t));
    if why.is_none() {
        let ws_id = ws.id.clone();
        editor = editor
            .on_action(move |action| msg(Msg::Edit(ws_id.clone(), action)))
            .key_binding(keys(ws.id.clone()));
    }
    let can_send = why.is_none() && !chat.draft.text().trim().is_empty();
    let under = match (&chat.stop_error, why) {
        (Some(error), _) => text(
            ctx.i18n
                .t_with(Key::ChatConvStopFailed, &args!("error" => error.as_str()))
                .into_owned(),
        )
        .size(tokens::font_size::CAPTION)
        .style(style::text::danger(t)),
        (None, Some(why)) => text(blocked_words(ctx, pane, why))
            .size(tokens::font_size::CAPTION)
            .style(style::text::warning(t)),
        (None, None) => text(ctx.t(Key::ChatConvInputHint))
            .size(tokens::font_size::CAPTION)
            .style(style::text::subtle(t)),
    };
    let mut footer = row![container(under).width(Length::Fill)]
        .spacing(tokens::space::MD)
        .align_y(Alignment::Center);
    let works = ctx.model.live.panes_logic.activity_or_idle(&pane.id) == AgentActivity::Working;
    if works && chat.thread.pending.is_none() {
        let stop = round(
            t,
            Glyph::Square,
            false,
            (!chat.stopping).then(|| msg(Msg::Stop(ws.id.clone()))),
        );
        footer = footer.push(hangar_ui::tip::over(
            t,
            stop,
            ctx.t(Key::ChatConvStopHint).into_owned(),
        ));
    }
    footer = footer.push(round(
        t,
        Glyph::Icon(Icon::Send),
        true,
        can_send.then(|| msg(Msg::Send(ws.id.clone()))),
    ));
    // What waits on the person takes the input's place: nothing can be sent
    // until it is answered, and the card says what to do.
    let waiting = chat.thread.pending.is_some();
    let surface: Element<'a, Message> = match card(ctx, ws, pane, chat) {
        Some(card) => card,
        None => column![
            crate::keymap::guard(ctx, editor),
            container(footer).padding(Padding {
                top: 0.0,
                right: tokens::space::MD,
                bottom: tokens::space::MD,
                left: tokens::space::XL,
            })
        ]
        .into(),
    };
    centred(
        container(
            container(surface)
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(t.colors.card)),
                    border: Border {
                        color: if waiting {
                            tint(t.colors.state_waiting_fill, 0.7)
                        } else {
                            t.colors.line_strong
                        },
                        width: tokens::BORDER_WIDTH,
                        radius: SOFT.into(),
                    },
                    shadow: iced::Shadow {
                        color: tint(t.colors.shadow, 0.6),
                        offset: iced::Vector::new(0.0, 2.0),
                        blur_radius: 12.0,
                    },
                    ..container::Style::default()
                }),
        )
        .padding(Padding {
            top: tokens::space::SM,
            right: tokens::space::XXL,
            bottom: tokens::space::XL,
            left: tokens::space::XXL,
        }),
    )
}
