//! The Share menu of the answer exports the written answer as a PDF, a Markdown file or an HTML
//! file at the place the save dialog gives, tells the person how it ended, and offers nothing
//! before the answer is written.

use super::recording::{self, Seen};
use super::{Window, click, has, is_enabled, node, says, shared};
use contract::{Command, NoticeKind};
use eframe::egui::accesskit::Role;
use export::Format;
use gui::app::layout::DEFAULT_WINDOW;
use gui::testkit;

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
