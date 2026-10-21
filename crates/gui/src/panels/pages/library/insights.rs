//! The Insights view of a document. It says that it is not shown yet.

use eframe::egui;

use crate::contract::DocumentView;
use crate::panels::PanelCx;
use crate::theme::Icon;
use crate::widgets::Placeholder;

const VIEW_ON_ITS_WAY: &str = "This view comes in a later version of quanty.";

pub(super) fn show(ui: &mut egui::Ui, _cx: &mut PanelCx<'_>) {
    Placeholder::empty(Icon::INSIGHT, DocumentView::Insights.label())
        .hint(VIEW_ON_ITS_WAY)
        .show(ui);
}
