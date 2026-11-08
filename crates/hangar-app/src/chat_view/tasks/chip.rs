//! A delivery's stage as a chip, for a worker's pane header and its task's
//! row in the tasks panel: the same words and colours in both.

use hangar_i18n::Key;
use hangar_theme::{style, tokens};
use hangar_ui::tip;
use iced::widget::{Space, container, mouse_area, row, text};
use iced::{Alignment, Element, Padding};

use crate::ctx::Ctx;
use crate::message::Message;

/// A delivery's stage in a capsule: a dot, then the words
/// (`delivery_stage::label`). Under way it is in the accent, and the dot of a
/// check beats with the second its time moves on; over, it is green or red,
/// and with `clickable` a click opens the task, whose delivery comment says
/// what the check printed. `short` where the room is tight: the words in
/// their short form, the full ones in the tooltip.
pub fn delivery_chip<'a>(
    ctx: Ctx<'a>,
    cwd: &str,
    task: &str,
    stage: crate::panes_logic::delivery_stage::DeliveryStage,
    now: u64,
    clickable: bool,
    short: bool,
) -> Element<'a, Message> {
    use crate::panes_logic::delivery_stage::{DeliveryStage, label};
    use hangar_theme::style::Tone;
    let t = ctx.theme;
    let tone = if stage.running() {
        Tone::Accent
    } else if stage.failed() {
        Tone::Danger
    } else {
        Tone::Success
    };
    let ink = style::container::tone_text(t, tone);
    // The dot beats only with the check's own tick: nothing else redraws it.
    let beat = matches!(stage, DeliveryStage::Checking { .. }) && (now / 1000) % 2 == 1;
    let dot_color = if beat {
        iced::Color { a: 0.4, ..ink }
    } else {
        ink
    };
    let mut content = row![].spacing(tokens::space::XS).align_y(Alignment::Center);
    if stage.running() {
        content = content.push(container(Space::new().width(6).height(6)).style(move |_| {
            container::Style {
                background: Some(iced::Background::Color(dot_color)),
                border: iced::border::rounded(3),
                ..container::Style::default()
            }
        }));
    }
    content = content.push(
        text(label(stage, now, ctx.i18n, short))
            .size(tokens::font_size::LABEL)
            .font(hangar_theme::fonts::ui_medium())
            .wrapping(text::Wrapping::None)
            .color(ink),
    );
    let chip = container(content)
        .padding(Padding::from([1.0, 6.0]))
        .style(style::container::badge(t, tone));
    if stage.running() {
        let tip_words = ctx.t(Key::PaneDeliveryTip);
        let words = if short {
            format!("{} — {tip_words}", label(stage, now, ctx.i18n, false))
        } else {
            tip_words.into_owned()
        };
        return tip::bottom(t, chip, words);
    }
    if !clickable {
        return chip.into();
    }
    let open = Message::Intent(crate::message::Intent::OpenTask {
        cwd: cwd.to_string(),
        task: Some(task.to_string()),
    });
    tip::bottom(
        t,
        mouse_area(chip)
            .interaction(iced::mouse::Interaction::Pointer)
            .on_press(open),
        ctx.t(Key::PaneDeliveryTipOutcome).into_owned(),
    )
}
