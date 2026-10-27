//! What the footer shows of each paid account, and a link to the page that shows its credit: one
//! line for each in a wide window, a slim row of short names in a smaller one, and the full lines
//! that the row opens. No provider gives a balance for an API key, so no entry shows an amount,
//! and an account whose key was taken is its billing link alone. It draws static text with no
//! spinner, so a window at rest asks for no repaint.

use eframe::egui;

use crate::panels::PanelCx;
use crate::theme::{Icon, TextRole, Tone, space};
use crate::widgets::{Button, ControlSize, dot};
use contract::{Balance, FailureKind, Intent, Provider, ReadFailure};

/// The room kept at the right end of an entry for its link, so that a long hint is cut short
/// before the link would pass the end of the entry.
const LINK_ROOM: f32 = 128.0;

pub(super) fn show(ui: &mut egui::Ui, row: egui::Rect, cx: &mut PanelCx<'_>) {
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
        if has_state(shown) {
            state_button(ui, provider, shown, ControlSize::Small, cx);
            ui.scope(|ui| {
                ui.set_max_width((ui.available_width() - LINK_ROOM).max(0.0));
                let words = TextRole::Small.rich(ui, hint(provider, shown));
                ui.add(egui::Label::new(words).truncate());
            });
        }
        link_button(ui, provider, ControlSize::Small, cx);
    });
}

/// One dot and one short name for each account, in a row that sends nothing. Returns whether the
/// person tapped either account.
pub(super) fn slim(ui: &mut egui::Ui, cx: &PanelCx<'_>) -> bool {
    let mut is_tapped = false;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = space::LG;
        for provider in Provider::ALL {
            let shown = cx.shared.balances.shown(provider);
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x = space::SM;
                dot(ui, tone(shown));
                let full_name = account_label(provider, shown);
                let button = Button::ghost(short_name(provider))
                    .size(ControlSize::Small)
                    .accessible_name(&full_name);
                is_tapped |= ui.add(button).clicked();
            });
        }
    });
    is_tapped
}

/// The full lines of both accounts, for a room that is too narrow for them to stand on one row:
/// what they hold wraps, and the hint is never cut short.
pub(super) fn lines(ui: &mut egui::Ui, cx: &mut PanelCx<'_>) {
    ui.scope(|ui| {
        // A wrapped row centres each item on the height the row has when the item is added, so
        // the dot needs the height of the buttons from the start.
        ui.spacing_mut().interact_size.y = ControlSize::Medium.height();
        ui.spacing_mut().item_spacing.x = space::SM;
        for provider in Provider::ALL {
            let shown = cx.shared.balances.shown(provider);
            ui.horizontal_wrapped(|ui| {
                dot(ui, tone(shown));
                if has_state(shown) {
                    state_button(ui, provider, shown, ControlSize::Medium, cx);
                }
                link_button(ui, provider, ControlSize::Medium, cx);
            });
            let words = hint(provider, shown);
            if !words.is_empty() {
                ui.add(egui::Label::new(TextRole::Small.rich(ui, words)).wrap());
            }
        }
    });
}

fn state_button(
    ui: &mut egui::Ui,
    provider: Provider,
    shown: Option<Balance>,
    size: ControlSize,
    cx: &mut PanelCx<'_>,
) {
    let label = account_label(provider, shown);
    if ui.add(Button::ghost(&label).size(size)).clicked() {
        cx.intents.push(Intent::ReadBalances);
    }
}

fn link_button(ui: &mut egui::Ui, provider: Provider, size: ControlSize, cx: &mut PanelCx<'_>) {
    let (link, opens) = match provider {
        Provider::Google => ("Google billing", "Open the Google billing page"),
        Provider::Jev => ("Jev billing", "Open Jev billing in the console"),
    };
    let button = Button::ghost(link)
        .icon(Icon::OPEN)
        .size(size)
        .accessible_name(opens);
    if ui.add(button).clicked() {
        cx.intents.push(Intent::OpenBillingPage(provider));
    }
}

fn account_label(provider: Provider, shown: Option<Balance>) -> String {
    match state(shown) {
        Some(state) => format!("{}: {state}", name(provider)),
        None => name(provider).to_owned(),
    }
}

/// A key that the provider took has nothing to say in words: the dot is green, and the credit is
/// behind the link.
fn has_state(shown: Option<Balance>) -> bool {
    state(shown).is_some()
}

/// The slim row of a narrow window has room only for a short name.
fn short_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Google => "Gemini",
        Provider::Jev => "Jev",
    }
}

fn name(provider: Provider) -> &'static str {
    match provider {
        Provider::Google => "Google Gemini",
        Provider::Jev => "Jev",
    }
}

/// A shown `None` is the time before the first read has ended.
fn state(shown: Option<Balance>) -> Option<&'static str> {
    let words = match shown {
        None => "checking…",
        Some(Balance::NotOffered) => return None,
        Some(Balance::KeyMissing) => "key missing",
        Some(Balance::KeyRefused) => "key refused",
        Some(Balance::OutOfCredit) => "out of credit",
        Some(Balance::LocationRefused) => "location refused",
        Some(Balance::Unread(_)) => "could not read",
    };
    Some(words)
}

fn tone(shown: Option<Balance>) -> Tone {
    match shown {
        Some(Balance::NotOffered) => Tone::Success,
        Some(Balance::KeyRefused | Balance::OutOfCredit | Balance::LocationRefused) => Tone::Danger,
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
        (_, Balance::NotOffered) => return String::new(),
        (Provider::Google, Balance::KeyMissing) => FailureKind::EmbeddingKeyMissing.hint(),
        (Provider::Jev, Balance::KeyMissing) => FailureKind::ConverterKeyMissing.hint(),
        (_, Balance::KeyRefused) => "Make a new key, put it in .env, then start quanty again",
        (_, Balance::LocationRefused) => FailureKind::EmbeddingLocationRefused.hint(),

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
