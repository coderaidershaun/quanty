//! The Share menu of the answer exports the written answer as a PDF, a Markdown file or an HTML
//! file at the place the save dialog gives, tells the person how it ended, and offers nothing
//! before the answer is written. On a phone Share stands beside the tabs of the answer, which can
//! all be reached.

use crate::flows::fit;
use crate::flows::recording::{self, Seen};
use crate::flows::{
    Window, click, copied_by, has, is_enabled, node, node_in, panels, says, shared,
};
use contract::{Command, Intent, NoticeKind};
use eframe::egui::Rect;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::NodeT as _;
use export::Format;
use gui::app::layout::{DEFAULT_WINDOW, PHONE_WINDOW};
use gui::testkit;
use gui::theme::{self, Scheme, size};

pub(super) fn downloaded_file(harness: &Window, seen: &Seen) -> std::path::PathBuf {
    let download = shared(harness)
        .export
        .as_ref()
        .unwrap()
        .download
        .as_ref()
        .unwrap();
    let relative = download.url.strip_prefix("/api/download/").unwrap();
    let id = relative.split('/').next().unwrap();
    seen.home()
        .join("data/exports")
        .join(id)
        .join(&download.file_name)
}

const QUESTION: &str = "How is the Black–Scholes formula derived and what assumptions does it make";

#[test]
fn each_export_writes_the_download_that_the_browser_opens() {
    let (mut harness, seen) = recording::open("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);

    click(&mut harness, Role::Button, "Share");
    for item in [
        "Copy as Markdown",
        "Export as PDF…",
        "Export as Markdown…",
        "Export as HTML…",
    ] {
        assert!(has(&harness, Role::Button, item), "the menu has {item}");
    }

    let starts = [
        (Format::Pdf, "Export as PDF…", "%PDF-"),
        (Format::Markdown, "Export as Markdown…", "# Black–Scholes"),
        (Format::Html, "Export as HTML…", "<!DOCTYPE html>"),
    ];
    let mut dark_exports = Vec::new();
    for (at, (format, item, beginning)) in starts.into_iter().enumerate() {
        if at > 0 {
            click(&mut harness, Role::Button, "Share");
        }
        click(&mut harness, Role::Button, item);

        let name = format!("{QUESTION}.{}", format.extension());
        let file = shared(&harness)
            .export
            .as_ref()
            .unwrap()
            .download
            .as_ref()
            .unwrap();
        assert_eq!(file.file_name, name);
        let written = std::fs::read(downloaded_file(&harness, &seen)).expect("the file is written");
        assert!(
            written.starts_with(beginning.as_bytes()),
            "the {} file starts with {beginning}",
            format.name()
        );
        dark_exports.push(written);
        let notice = shared(&harness)
            .notices
            .first()
            .expect("the export left a notice");
        assert_eq!(notice.kind, NoticeKind::Done);
        assert!(
            notice
                .detail
                .contains(&format!("Your browser is downloading {name}"))
        );
        assert!(
            says(
                &harness,
                &format!("Exported the answer as {}", format.name())
            ),
            "the answer pane shows how it ended"
        );
    }
    assert_eq!(
        seen.count(|command| matches!(command, Command::Export { .. })),
        3
    );

    harness.state_mut().push(Intent::SwitchScheme);
    testkit::settle(&mut harness);
    assert_eq!(shared(&harness).scheme, Scheme::Light);
    assert_eq!(theme::scheme(&harness.ctx), Scheme::Light);
    for ((format, item, _), dark) in starts.into_iter().zip(&dark_exports) {
        click(&mut harness, Role::Button, "Share");
        click(&mut harness, Role::Button, item);
        let light = std::fs::read(downloaded_file(&harness, &seen)).expect("the file is written");
        assert!(
            light == *dark,
            "the {} file written while the window is light ({} bytes) differs from the one written while it was dark ({} bytes)",
            format.name(),
            light.len(),
            dark.len()
        );
    }
    assert_eq!(
        seen.count(|command| matches!(command, Command::Export { .. })),
        6,
        "each format was exported again"
    );
}

#[test]
fn a_write_that_fails_says_where_and_why() {
    let (mut harness, seen) = recording::open("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    std::fs::write(seen.home().join("data"), "exports cannot be made here").unwrap();

    click(&mut harness, Role::Button, "Share");
    click(&mut harness, Role::Button, "Export as PDF…");

    let notice = shared(&harness)
        .notices
        .first()
        .expect("the failed export left a notice");
    assert_eq!(notice.kind, NoticeKind::Failed);
    assert!(
        notice.detail.contains("Not a directory"),
        "{}",
        notice.detail
    );
    assert!(says(&harness, "Could not export the answer as PDF"));
    assert!(says(&harness, "Not a directory"));

    click(&mut harness, Role::Button, "Dismiss");
    assert!(
        !says(&harness, "Could not export the answer as PDF"),
        "Dismiss takes the strip away"
    );
    assert!(shared(&harness).notices.is_empty());
}

#[test]
fn nothing_is_exported_before_the_answer_is_written() {
    let (mut harness, seen): (Window, Seen) = recording::open("answering", DEFAULT_WINDOW);
    testkit::run_until(&mut harness, "the results", |shared| {
        shared.ask.search.ready().is_some()
    });
    harness.run_ok();

    assert!(shared(&harness).ask.answer.is_loading());
    assert!(!is_enabled(&harness, Role::Button, "Share"));
    node(&harness, Role::Button, "Share").click();
    harness.run_ok();
    assert!(!has(&harness, Role::Button, "Export as PDF…"));
    assert!(shared(&harness).ask.export_request(Format::Pdf).is_none());
    assert_eq!(
        seen.count(|command| matches!(command, Command::Export { .. })),
        0
    );
}

#[test]
fn a_phone_reaches_every_answer_tab_and_shares_from_beside_them() {
    let (mut harness, seen) = recording::open("black-scholes", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let answer = panels(PHONE_WINDOW).answer;
    let screen = fit::window_rect(PHONE_WINDOW);
    let share = node_in(&harness, Role::Button, "Share", answer)
        .expect("Share is in the answer pane")
        .rect();
    assert!(
        screen.contains_rect(share),
        "Share {share:?} is on the screen"
    );
    fit::assert_finger_sized("Share", share, size::CONTROL_MD);

    // A tab that has slid out of the strip stands past the pane, so a tab is found at any place
    // along the row.
    let row = Rect::from_x_y_ranges(f32::NEG_INFINITY..=f32::INFINITY, answer.y_range());
    for tab in ["Answer", "Results", "Formulas", "Figures", "Tables", "Code"] {
        tab_in(&harness, tab, row).scroll_to_me();
        testkit::settle(&mut harness);
        let rect = tab_in(&harness, tab, row).rect();
        assert!(
            answer.expand(fit::SLACK).contains_rect(rect)
                && rect.right() <= share.left() + fit::SLACK,
            "`{tab}` {rect:?} is not in the pane and before Share {share:?}"
        );
        tab_in(&harness, tab, row).click();
        testkit::settle(&mut harness);
        let is_open = tab_in(&harness, tab, row).accesskit_node().is_selected();
        assert_eq!(is_open, Some(true), "`{tab}` opens");
    }
    testkit::save_png(&mut harness, "phone-answer-header");

    click(&mut harness, Role::Button, "Share");
    for item in ["Copy as Markdown", "Export as PDF…"] {
        assert!(has(&harness, Role::Button, item), "the menu has {item}");
        fit::assert_finger_sized(
            item,
            node(&harness, Role::Button, item).rect(),
            size::CONTROL_MD,
        );
    }
    testkit::save_png(&mut harness, "phone-share-menu");
    let copies = copied_by(&mut harness, |harness| {
        node(harness, Role::Button, "Copy as Markdown").click()
    });
    let markdown = shared(&harness)
        .ask
        .answer_markdown()
        .expect("an answer to share");
    assert_eq!(copies, [markdown], "Share copies the answer as Markdown");

    // The menu stays open after a copy, so its Export is pressed with no second press on Share,
    // which would close the menu.
    std::fs::write(seen.home().join("data"), "exports cannot be made here").unwrap();
    click(&mut harness, Role::Button, "Export as PDF…");
    assert!(says(&harness, "Could not export the answer as PDF"));
    let dismiss = node_in(&harness, Role::Button, "Dismiss", answer)
        .expect("the strip of a failed export has a Dismiss in the answer pane")
        .rect();
    assert!(
        screen.contains_rect(dismiss),
        "Dismiss {dismiss:?} is on the screen"
    );
    fit::assert_finger_sized("Dismiss", dismiss, size::CONTROL_MD);
}

fn tab_in<'a>(harness: &'a Window, name: &'a str, row: Rect) -> egui_kittest::Node<'a> {
    node_in(harness, Role::Tab, name, row)
        .unwrap_or_else(|| panic!("no tab `{name}` in the header"))
}
