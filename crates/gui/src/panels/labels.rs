//! The authors box and the tags boxes of a form: their names, the caption over a box, and the
//! lists to and from the text a person types; the tokens and cost of a run; the words of a failure.

use std::borrow::Borrow;

use eframe::egui;

use crate::contract::{Failure, Usage};
use crate::theme::{TextRole, space};

pub(super) const AUTHORS: &str = "Authors";
pub(super) const TAGS: &str = "Tags";
pub(super) const LIST_PLACEHOLDER: &str = "Optional, with commas between them";
pub(super) const TRY_AGAIN: &str = "Try again";

pub(super) fn caption(ui: &mut egui::Ui, text: &str) {
    ui.add_space(space::SM);
    ui.label(TextRole::Label.rich(text));
}

pub(super) fn detail(ui: &mut egui::Ui, failure: &Failure) {
    ui.label(TextRole::Small.rich(&failure.detail));
}

/// One rule for every list a person types, authors and tags alike: commas between the items,
/// and a blank item is dropped.
pub(super) fn list_of(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn text_of<S: Borrow<str>>(items: &[S]) -> String {
    items.join(", ")
}

/// A cost in words: "≈ $0.42", "under $0.01" for a cost that would round to nothing, and
/// "unknown" when a model has no price.
// SMELL: the core crate keeps its own copy of this rule, and no panel may name that crate, so a
// change to one must be made in both.
pub(super) fn cost_text(usd: Option<f64>) -> String {
    match usd {
        None => "unknown".to_owned(),
        Some(usd) if usd > 0.0 && usd < 0.005 => "under $0.01".to_owned(),
        Some(usd) => format!("≈ ${usd:.2}"),
    }
}

/// A count of tokens in a few characters: "12", "48k", "1.2M".
pub(super) fn tokens_text(tokens: u64) -> String {
    let thousands = (tokens + 500) / 1_000;
    if tokens < 1_000 {
        tokens.to_string()
    } else if thousands < 1_000 {
        format!("{thousands}k")
    } else {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    }
}

/// What a run used, in words: "12k tokens, about $0.03", "12 tokens, under $0.01", or the tokens
/// alone ("12k tokens") for a model that has no price.
pub(super) fn spent_text(usage: &Usage) -> String {
    let tokens = format!("{} tokens", tokens_text(usage.tokens()));
    if usage.cost_usd.is_none() {
        return tokens;
    }
    let cost = cost_text(usage.cost_usd);
    match cost.strip_prefix("≈ ") {
        Some(price) => format!("{tokens}, about {price}"),
        None => format!("{tokens}, {cost}"),
    }
}

/// "claude-sonnet-5-5 192k, gemini-embedding-2 9k (estimated)".
pub(super) fn tokens_by_model(usage: &Usage) -> String {
    let models: Vec<String> = usage
        .models
        .iter()
        .map(|model| {
            let estimated = if model.estimated { " (estimated)" } else { "" };
            format!("{} {}{estimated}", model.model, tokens_text(model.total()))
        })
        .collect();
    models.join(", ")
}
