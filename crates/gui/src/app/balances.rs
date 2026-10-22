//! The footer: what is known of each paid account, and a link to the page that shows its credit.
//! No provider gives a balance for an API key, so no entry shows an amount. It draws static text
//! with no spinner, so a window at rest asks for no repaint.

use eframe::egui;

use crate::contract::{Balance, FailureKind, Intent, Provider, ReadFailure};
use crate::panels::PanelCx;
use crate::theme::{Icon, TextRole, Tone, color, hairline, space};
use crate::widgets::{Button, ControlSize, dot};

/// The room kept at the right end of an entry for its link, so that a long hint is cut short
/// before the link would pass the end of the entry.
const LINK_ROOM: f32 = 104.0;

pub(super) fn show(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    let footer = ui.max_rect();
    let painter = ui.painter();
    let line = hairline(painter, color::HAIRLINE);
    painter.hline(footer.x_range(), footer.top() + line.width / 2.0, line);

    let row = footer.shrink2(egui::vec2(space::LG, 0.0));
    let width = (row.width() - space::LG) / 2.0;
    for (place, provider) in Provider::ALL.into_iter().enumerate() {
        let left = row.left() + place as f32 * (width + space::LG);
        let rect =
            egui::Rect::from_min_size(egui::pos2(left, row.top()), egui::vec2(width, row.height()));
        entry(ui, provider, rect, cx);
    }
}

fn entry(ui: &mut egui::Ui, provider: Provider, rect: egui::Rect, cx: &mut PanelCx<'_>) {
    let shown = cx.shared.balances.shown(provider);
    let builder = egui::UiBuilder::new()
        .id_salt(name(provider))
        .max_rect(rect)
        .layout(egui::Layout::left_to_right(egui::Align::Center));
    ui.scope_builder(builder, |ui| {
        ui.spacing_mut().item_spacing.x = space::SM;
        dot(ui, tone(shown));
        let label = format!("{}: {}", name(provider), state(shown));
        if ui
            .add(Button::ghost(&label).size(ControlSize::Small))
            .clicked()
        {
            cx.intents.push(Intent::ReadBalances);
        }
        ui.scope(|ui| {
            ui.set_max_width((ui.available_width() - LINK_ROOM).max(0.0));
            ui.add(egui::Label::new(TextRole::Small.rich(hint(provider, shown))).truncate());
        });
        let (link, opens) = match provider {
            Provider::Google => ("Billing", "Open the Google billing page"),
            Provider::Jev => ("Console", "Open the Jev console"),
        };
        let button = Button::ghost(link)
            .icon(Icon::OPEN)
            .size(ControlSize::Small)
            .accessible_name(opens);
        if ui.add(button).clicked() {
            cx.intents.push(Intent::OpenBillingPage(provider));
        }
    });
}

fn name(provider: Provider) -> &'static str {
    match provider {
        Provider::Google => "Google Gemini",
        Provider::Jev => "Jev",
    }
}

/// `None` is the time before the first read has ended.
fn state(shown: Option<Balance>) -> &'static str {
    match shown {
        None => "checking…",
        Some(Balance::NotOffered) => "balance not offered",
        Some(Balance::KeyMissing) => "key missing",
        Some(Balance::KeyRefused) => "key refused",
        Some(Balance::OutOfCredit) => "out of credit",
        Some(Balance::Unread(_)) => "could not read",
    }
}

fn tone(shown: Option<Balance>) -> Tone {
    match shown {
        Some(Balance::NotOffered) => Tone::Success,
        Some(Balance::KeyRefused | Balance::OutOfCredit) => Tone::Danger,
        None | Some(Balance::KeyMissing | Balance::Unread(_)) => Tone::Warning,
    }
}

/// What the person can do next. The settings are read once, before the window opens, and a free
/// read cannot see credit, so no hint promises that a click or a new key is enough.
fn hint(provider: Provider, shown: Option<Balance>) -> String {
    let Some(shown) = shown else {
        return String::new();
    };
    let words = match (provider, shown) {
        (Provider::Google, Balance::NotOffered) => "See it on the billing page",
        (Provider::Jev, Balance::NotOffered) => "See it in the console",
        (Provider::Google, Balance::KeyMissing) => FailureKind::EmbeddingKeyMissing.hint(),
        (Provider::Jev, Balance::KeyMissing) => FailureKind::ConverterKeyMissing.hint(),
        (_, Balance::KeyRefused) => "Make a new key, put it in .env, then start quanty again",
        (Provider::Google, Balance::OutOfCredit) => {
            "Google said so. Add credit; the next ask that works clears this"
        }
        (Provider::Jev, Balance::OutOfCredit) => {
            "Jev said so. Add credit; the next ingest that works clears this"
        }
        (_, Balance::Unread(failure)) => return unread_hint(failure),
    };
    // A failure's hint ends with a full stop, and the footer's hints end with none.
    words.trim_end_matches('.').to_owned()
}

fn unread_hint(failure: ReadFailure) -> String {
    let cause = match failure {
        ReadFailure::Unreachable => "Could not be reached".to_owned(),
        ReadFailure::TimedOut => "No answer in time".to_owned(),
        ReadFailure::Status(status) => format!("Answered HTTP {status}"),
        ReadFailure::Internal => "Something went wrong inside quanty".to_owned(),
    };
    format!("{cause}. Click to try again")
}
