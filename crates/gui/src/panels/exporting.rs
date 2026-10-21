//! What every panel that exports a document draws alike: the three Export items of its menu, and
//! the strip that says how the last export from it ended.

use eframe::egui;
use export::Format;

use super::PanelCx;
use crate::contract::{ExportPlace, ExportRequest, Intent, NoticeKind};
use crate::theme::{Icon, size, space};
use crate::widgets::{Button, ControlSize, Notice};

/// Every item of a menu with Export items is this wide, so the menu does not change width when
/// an item changes its words.
pub(crate) const ITEM_WIDTH: f32 = 6.0 * size::CONTROL_LG;

const BUSY: &str = "A file is being exported.";

/// One item for each kind of file. `request` builds what the panel asks to export, and gives
/// `None` when there is nothing to export. Gives back whether an item was chosen, so that the
/// caller can close its menu.
pub(crate) fn items(
    ui: &mut egui::Ui,
    cx: &mut PanelCx<'_>,
    request: impl Fn(Format) -> Option<ExportRequest>,
) -> bool {
    let is_busy = cx.shared.is_exporting();
    let mut is_chosen = false;
    for format in Format::ALL {
        let label = format!("Export as {}\u{2026}", format.name());
        let item = Button::ghost(&label)
            .icon(Icon::DOCUMENT)
            .size(ControlSize::Small)
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
    if strip.body(&notice.detail).dismissable().show(ui).dismissed {
        cx.intents.push(Intent::DismissNotice(notice.id));
    }
}
