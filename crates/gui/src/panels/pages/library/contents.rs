//! The Code, Formulas, Figures and Tables views of a document. They say that they are not shown
//! yet.

use eframe::egui;

use crate::contract::DocumentView;
use crate::panels::PanelCx;
use crate::theme::Icon;
use crate::widgets::Placeholder;

const VIEW_ON_ITS_WAY: &str = "This view comes in a later version of quanty.";

pub(super) fn show(ui: &mut egui::Ui, view: DocumentView, _cx: &mut PanelCx<'_>) {
    let icon = match view {
        DocumentView::Code => Icon::CODE,
        DocumentView::Formulas => Icon::FORMULA,
        DocumentView::Figures => Icon::FIGURE,
        DocumentView::Tables => Icon::TABLE,
        DocumentView::Overview | DocumentView::Insights | DocumentView::Concepts => Icon::DOCUMENT,
    };
    Placeholder::empty(icon, view.label())
        .hint(VIEW_ON_ITS_WAY)
        .show(ui);
}
