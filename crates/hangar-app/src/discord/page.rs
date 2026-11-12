//! Settings › Discord, the connector's block: the bot's token and the
//! server's id until they are set; then whether the bot runs, the code that
//! pairs the person's account until one is, the invite link, Disconnect.

use std::borrow::Cow;

use hangar_i18n::{Key, args};
use hangar_theme::tokens;
use hangar_ui::{badge, button as ui_button, form, input, text as typography};
use iced::widget::{column, container, row};
use iced::{Alignment, Element, Length};

use crate::ctx::Ctx;
use crate::message::Message;

use super::{Msg, State};

fn press(msg: Msg) -> Message {
    Message::Discord(msg)
}

/// A dictionary string that takes no argument, borrowed as a placeholder
/// must be.
fn static_text(text: Cow<'static, str>) -> &'static str {
    match text {
        Cow::Borrowed(text) => text,
        Cow::Owned(_) => "",
    }
}

pub fn settings_page<'a>(st: &'a State, ctx: Ctx<'a>) -> Element<'a, Message> {
    let t = ctx.theme;
    let body: Element<'a, Message> = if st.saved.ready() {
        connected(st, ctx)
    } else {
        setup(st, ctx)
    };
    form::group(
        t,
        vec![
            container(body)
                .padding(tokens::space::XL)
                .width(Length::Fill)
                .into(),
        ],
    )
}

fn connected<'a>(st: &'a State, ctx: Ctx<'a>) -> Element<'a, Message> {
    let t = ctx.theme;
    let guild = st.saved.guild.as_str();
    let (state, tone) = if st.running() {
        (
            ctx.i18n
                .t_with(Key::DiscordBotRunning, &args!("guild" => guild))
                .into_owned(),
            badge::Tone::Success,
        )
    } else {
        (
            ctx.i18n
                .t_with(Key::DiscordBotStarting, &args!("guild" => guild))
                .into_owned(),
            badge::Tone::Accent,
        )
    };
    let paired = match (&st.saved.owner, &st.saved.pairing) {
        (Some(_), _) => typography::hint(t, ctx.t(Key::DiscordBotPaired)),
        (None, Some(code)) => typography::body(
            t,
            ctx.i18n
                .t_with(Key::DiscordBotPair, &args!("code" => code.as_str()))
                .into_owned(),
        ),
        (None, None) => typography::hint(t, ctx.t(Key::DiscordBotPaired)),
    };
    let mut actions = row![].spacing(tokens::space::MD).align_y(Alignment::Center);
    if hangar_discord::setup::bot_id(&st.saved.token).is_some() {
        actions = actions.push(
            ui_button::secondary(t, ctx.t(Key::DiscordBotInvite))
                .small()
                .on_press(press(Msg::Invite)),
        );
    }
    actions = actions.push(
        ui_button::ghost(t, ctx.t(Key::DiscordBotDisconnect))
            .small()
            .on_press(press(Msg::Disconnect)),
    );
    column![
        row![
            typography::body(t, ctx.t(Key::DiscordBotTitle))
                .font(hangar_theme::fonts::ui_semibold()),
            badge::badge(t, state, tone),
        ]
        .spacing(tokens::space::MD)
        .align_y(Alignment::Center),
        paired,
        typography::hint(t, ctx.t(Key::DiscordBotHowTo)),
        actions,
    ]
    .spacing(tokens::space::MD)
    .into()
}

fn setup<'a>(st: &'a State, ctx: Ctx<'a>) -> Element<'a, Message> {
    let t = ctx.theme;
    let token = input::text_field(t, &st.form.token)
        .placeholder(static_text(ctx.t(Key::DiscordBotTokenPlaceholder)))
        .secure()
        .on_input(|value| press(Msg::Token(value)))
        .on_submit(press(Msg::Connect));
    let guild = input::text_field(t, &st.form.guild)
        .placeholder(static_text(ctx.t(Key::DiscordBotGuildPlaceholder)))
        .on_input(|value| press(Msg::Guild(value)))
        .on_submit(press(Msg::Connect));
    let field = |label: Key, hint: Key, input: Element<'a, Message>| -> Element<'a, Message> {
        column![
            typography::body(t, ctx.t(label)),
            typography::hint(t, ctx.t(hint)),
            input
        ]
        .spacing(tokens::space::SM)
        .into()
    };
    let mut body = column![
        typography::hint(t, ctx.t(Key::DiscordBotIntro)),
        field(Key::DiscordBotToken, Key::DiscordBotTokenHint, token.into()),
        field(Key::DiscordBotGuild, Key::DiscordBotGuildHint, guild.into()),
    ]
    .spacing(tokens::space::MD);
    if let Some(error) = st.form.error {
        body = body.push(typography::danger(t, ctx.t(error)));
    }
    body.push(ui_button::primary(t, ctx.t(Key::DiscordBotConnect)).on_press(press(Msg::Connect)))
        .into()
}
