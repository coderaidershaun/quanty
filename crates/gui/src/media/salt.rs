//! Salts made from what a formula or a table shows, so that its scroll place stays when things
//! above it come and go.

use eframe::egui::Ui;

/// How many widgets with the same content a parent has shown so far in one pass.
#[derive(Clone, Copy)]
struct Shown {
    pass: u64,
    count: u32,
}

/// Equal contents in one parent are told apart by how many of them the parent showed before in
/// this pass, so that two equal widgets never share a scroll place.
pub(super) fn content_salt(ui: &Ui, content: u64) -> (u64, u32) {
    let pass = ui.ctx().cumulative_pass_nr();
    let key = ui.id().with(content);
    let earlier = ui.data_mut(|data| {
        let shown = data.get_temp_mut_or_insert_with(key, || Shown { pass, count: 0 });
        if shown.pass != pass {
            *shown = Shown { pass, count: 0 };
        }
        shown.count += 1;
        shown.count - 1
    });
    (content, earlier)
}
