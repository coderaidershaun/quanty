//! What every panel that exports a document draws alike: the Export button, the items of its menu,
//! and the strip that says how the last export from it ended.

use eframe::egui;
use export::Format;

use super::{PanelCx, PanelWidth};
use crate::theme::{Icon, size, space};
use crate::widgets::{Button, ControlSize, Notice};
use contract::{ExportPlace, ExportRequest, Intent, NoticeKind};

/// Every item of a menu with Export items is this wide, so the menu does not change width when
/// an item changes its words.
pub(crate) const ITEM_WIDTH: f32 = 6.0 * size::CONTROL_LG;

const BUSY: &str = "A file is being exported.";
const EXPORT: &str = "Export";

/// A strip narrower than this makes its buttons tall enough for a finger.
const NARROW_BELOW: f32 = 480.0;

/// The Export button and the menu of its items. `request` is as for `items`.
pub(crate) fn menu(
    ui: &mut egui::Ui,
    cx: &mut PanelCx<'_>,
    size: ControlSize,
    request: impl Fn(Format) -> Option<ExportRequest>,
) {
    let button = Button::secondary(EXPORT).icon(Icon::DOCUMENT).size(size);
    let response = ui.add(button);
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            if items(ui, cx, size, request) {
                ui.close();
            }
        });
}

/// One item for each kind of file, `size` tall. `request` builds what the panel asks to export,
/// and gives `None` when there is nothing to export. Gives back whether an item was chosen, so
/// that the caller can close its menu.
pub(crate) fn items(
    ui: &mut egui::Ui,
    cx: &mut PanelCx<'_>,
    size: ControlSize,
    request: impl Fn(Format) -> Option<ExportRequest>,
) -> bool {
    let is_busy = cx.shared.is_exporting();
    let mut is_chosen = false;
    for format in Format::ALL {
        let label = format!("Export as {}\u{2026}", format.name());
        let item = Button::ghost(&label)
            .icon(Icon::DOCUMENT)
            .size(size)
            .min_width(ITEM_WIDTH)
            .start_aligned();
        let response = ui.add_enabled(!is_busy, item).on_disabled_hover_text(BUSY);
        if response.clicked()
            && let Some(export) = request(format)
        {
            cx.intents.push(Intent::Export(export));
            is_chosen = true;
        }
    }
    is_chosen
}

/// How the last export from `place` ended, until it is dismissed. Nothing when there is none.
pub(crate) fn strip(ui: &mut egui::Ui, cx: &mut PanelCx<'_>, place: ExportPlace) {
    let Some(notice) = cx.shared.export_notice(place) else {
        return;
    };
    ui.add_space(space::SM);
    let strip = match notice.kind {
        NoticeKind::Done => Notice::success(&notice.title),
        NoticeKind::Failed => Notice::error(&notice.title),
    };
    let size = PanelWidth::of(ui, NARROW_BELOW).control_size();
    let shown = strip
        .body(&notice.detail)
        .dismissable()
        .control_size(size)
        .show(ui);
    if shown.dismissed {
        cx.intents.push(Intent::DismissNotice(notice.id));
    }
}
