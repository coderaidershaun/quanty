//! Checks that pictures load from real files, and that a missing or broken one is reported.

use std::time::{Duration, Instant};

use eframe::egui;
use egui_kittest::kittest::Queryable;
use gui::contract::ImageRef;
use gui::media::Offload;
use gui::media::images::{self, ImageState, Images, Picture};
use gui::testkit;

fn chapter_file(page: u32, file: &str) -> ImageRef {
    let path = testkit::samples_folder()
        .join("option-volatility-and-pricing")
        .join("chapter-1")
        .join(format!("page-num-{page}"))
        .join(file);
    ImageRef { path }
}

fn page(number: u32) -> ImageRef {
    chapter_file(number, "page.png")
}

fn figure() -> ImageRef {
    chapter_file(5, "01-figure.png")
}

/// The stored sizes of one picture, widest first, as the texture manager holds them.
fn stored_sizes(ctx: &egui::Context, image: &ImageRef) -> Vec<[usize; 2]> {
    let prefix = format!("image:{}#", image.path.display());
    let manager = ctx.tex_manager();
    let manager = manager.read();
    let mut found: Vec<(String, [usize; 2])> = manager
        .allocated()
        .filter(|(_, meta)| meta.name.starts_with(&prefix))
        .map(|(_, meta)| (meta.name.clone(), meta.size))
        .collect();
    found.sort();
    found.into_iter().map(|(_, size)| size).collect()
}

fn ready(images: &mut Images, image: &ImageRef) -> Picture {
    match images.get(image) {
        ImageState::Ready(picture) => picture,
        other => panic!("{} is {other:?}, not ready", image.path.display()),
    }
}

fn size_in_file(image: &ImageRef) -> egui::Vec2 {
    let (width, height) = image::image_dimensions(&image.path).expect("the file has a size");
    egui::vec2(width as f32, height as f32)
}

/// The widest stored size: no screen is wider than this.
fn widest(picture: &Picture) -> egui::TextureId {
    picture.texture(100_000.0, 1.0)
}

#[test]
fn committed_pictures_load_once_and_keep_their_textures() {
    let ctx = egui::Context::default();
    let mut images = Images::new(&ctx, Offload::Manual);
    let folder = tempfile::tempdir().expect("a temporary folder");
    let jpeg = ImageRef {
        path: folder.path().join("lone.jpg"),
    };
    image::RgbImage::from_fn(300, 200, |x, y| image::Rgb([x as u8, y as u8, 128]))
        .save(&jpeg.path)
        .expect("the JPEG is written");
    let asked = [(page(2), 5), (figure(), 5), (jpeg, 3)];

    for (image, _) in &asked {
        assert_eq!(images.get(image), ImageState::Loading);
    }
    images.run_pending();
    for (image, _) in &asked {
        assert_eq!(
            images.get(image),
            ImageState::Loading,
            "a finished picture is taken in by the next poll, not before"
        );
    }
    images.poll(&ctx);

    let mut first_textures = Vec::new();
    for (image, _) in &asked {
        let picture = ready(&mut images, image);
        assert_eq!(picture.size(), size_in_file(image));
        first_textures.push(widest(&picture));
    }

    for _ in 0..50 {
        images.poll(&ctx);
        images.run_pending();
        for (image, _) in &asked {
            ready(&mut images, image);
        }
    }

    for ((image, levels), first) in asked.iter().zip(first_textures) {
        let picture = ready(&mut images, image);
        assert_eq!(widest(&picture), first, "no second upload of the picture");
        let sizes = stored_sizes(&ctx, image);
        assert_eq!(sizes.len(), *levels, "one texture for each stored size");
        assert!(
            sizes.windows(2).all(|pair| pair[1][0] == pair[0][0] / 2),
            "each stored size is half of the one before: {sizes:?}"
        );
    }

    let mut threaded = Images::new(&ctx, Offload::Threads);
    let third = page(3);
    assert_eq!(threaded.get(&third), ImageState::Loading);
    let started = Instant::now();
    let picture = loop {
        threaded.poll(&ctx);
        if let ImageState::Ready(picture) = threaded.get(&third) {
            break picture;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "a worker thread decodes the picture with no run_pending"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(picture.size(), size_in_file(&third));
    assert!(threaded.is_idle());
}

#[test]
fn a_missing_or_broken_file_fails_and_retry_reads_it_again() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let later = ImageRef {
        path: folder.path().join("figure.png"),
    };
    let shown = later.clone();
    let mut harness = testkit::panel([520.0, 260.0], testkit::asked("q"), move |ui, cx| {
        images::show(
            ui,
            cx.media,
            &shown,
            "Payoff chart",
            egui::vec2(480.0, 220.0),
        );
    });

    harness.run();
    assert!(
        harness
            .query_all_by_label_contains("Loading picture: Payoff chart")
            .next()
            .is_some()
    );
    testkit::save_png(&mut harness, "images-loading");

    harness.state_mut().media.run_pending();
    harness.run();
    assert!(
        harness
            .query_all_by_label_contains("Picture not found")
            .next()
            .is_some()
    );
    testkit::save_png(&mut harness, "images-missing");

    std::fs::copy(figure().path, &later.path).expect("the picture appears on disk");
    harness.state_mut().media.run_pending();
    harness.run();
    assert!(
        harness
            .query_all_by_label_contains("Picture not found")
            .next()
            .is_some(),
        "a failure is kept: the file is not looked for again by itself"
    );

    harness.get_by_label("Retry").click();
    harness.run();
    assert!(
        harness
            .query_all_by_label_contains("Loading picture: Payoff chart")
            .next()
            .is_some()
    );
    harness.state_mut().media.run_pending();
    harness.run();
    assert!(harness.query_all_by_label("Payoff chart").next().is_some());
    testkit::save_png(&mut harness, "images-ready");

    let notes = ImageRef {
        path: folder.path().join("notes.png"),
    };
    std::fs::write(&notes.path, "this is not a picture").expect("the text file is written");
    let mut harness = testkit::panel([520.0, 260.0], testkit::asked("q"), move |ui, cx| {
        images::show(ui, cx.media, &notes, "Notes", egui::vec2(480.0, 220.0));
    });
    harness.run();
    harness.state_mut().media.run_pending();
    harness.run();
    assert!(
        harness
            .query_all_by_label_contains("Picture cannot be read")
            .next()
            .is_some()
    );
    testkit::save_png(&mut harness, "images-unreadable");

    let gone = ImageRef {
        path: folder.path().join("gone.png"),
    };
    let mut harness = testkit::panel([400.0, 120.0], testkit::asked("q"), move |ui, cx| {
        images::show(ui, cx.media, &gone, "Small figure", egui::vec2(160.0, 64.0));
    });
    harness.run();
    harness.state_mut().media.run_pending();
    harness.run();
    assert!(
        harness
            .query_all_by_label_contains("Picture not found")
            .next()
            .is_some()
    );
    testkit::save_png(&mut harness, "images-failed-small");
}
