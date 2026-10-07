//! Turns the keys a person presses into intents.

use eframe::egui;

use crate::contract::Intent;

/// Reads this frame's keys and pushes the intents they stand for. It binds no key yet.
pub(super) fn read(_ctx: &egui::Context, _intents: &mut Vec<Intent>) {}
