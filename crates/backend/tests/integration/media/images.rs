//! Fetches real sample bytes through content URLs and checks cached, missing and broken pictures.

use std::path::{Path, PathBuf};
use std::time::Duration;

use contract::ImageRef;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use gui::media::images::{self, ImageState, Images, Picture};
use gui::media::{Media, Offload};
use gui::testkit::{self, FolderFetch, Host};

fn chapter_file(page: u32, file: &str) -> ImageRef {
    ImageRef {
        url: format!("/content/option-volatility-and-pricing/chapter-1/page-num-{page}/{file}"),
    }
}

fn page(number: u32) -> ImageRef {
    chapter_file(number, "page.png")
}

fn figure() -> ImageRef {
    chapter_file(5, "01-figure.png")
}

/// The stored sizes of one picture, widest first, as the texture manager holds them.
fn stored_sizes(ctx: &egui::Context, image: &ImageRef) -> Vec<[usize; 2]> {
    let prefix = format!("image:{}#", image.url);
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
        other => panic!("{} is {other:?}, not ready", image.url),
    }
}

fn size_in_file(root: &Path, image: &ImageRef) -> egui::Vec2 {
    let relative =
        percent_encoding::percent_decode_str(image.url.strip_prefix("/content/").unwrap())
            .decode_utf8()
            .unwrap();
    let path = root.join(relative.as_ref());
    let (width, height) = image::image_dimensions(path).expect("the file has a size");
    egui::vec2(width as f32, height as f32)
}

/// The widest stored size: no screen is wider than this.
fn widest(picture: &Picture) -> egui::TextureId {
    picture.texture(100_000.0, 1.0)
}

#[test]
fn committed_pictures_load_once_and_keep_their_textures() {
    let ctx = egui::Context::default();
    let folder = tempfile::tempdir().expect("a temporary folder");
    for image in [page(2), figure(), page(3)] {
        let relative = image.url.strip_prefix("/content/").unwrap();
        let to = folder.path().join(relative);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(testkit::samples_folder().join(relative), to).unwrap();
    }
    let mut images = Images::new(
        &ctx,
        Offload::Manual,
        Box::new(FolderFetch::new(folder.path().into())),
    );
    let jpeg = ImageRef {
        url: "/content/lone%20picture.jpg".into(),
    };
    image::RgbImage::from_fn(300, 200, |x, y| image::Rgb([x as u8, y as u8, 128]))
        .save(folder.path().join("lone picture.jpg"))
        .expect("the JPEG is written");
    let asked = [(page(2), 5), (figure(), 5), (jpeg, 3)];

    for (image, _) in &asked {
        assert_eq!(images.get(image), ImageState::Loading);
    }
    images.run_for(Duration::ZERO);
    for (image, _) in &asked {
        assert!(
            stored_sizes(&ctx, image).is_empty(),
            "a zero frame budget decodes nothing"
        );
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
        assert_eq!(picture.size(), size_in_file(folder.path(), image));
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

    let mut media = Media::with_fetch(&ctx, Box::new(FolderFetch::new(folder.path().into())));
    let third = page(3);
    assert_eq!(media.images.get(&third), ImageState::Loading);
    media.poll(&ctx);
    let picture = ready(&mut media.images, &third);
    assert_eq!(picture.size(), size_in_file(folder.path(), &third));
    assert!(media.is_idle());
}

#[test]
fn a_missing_or_broken_file_fails_and_retry_reads_it_again() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let later = ImageRef {
        url: "/content/figure.png".into(),
    };
    let shown = later.clone();
    let root = folder.path().to_path_buf();
    let mut installed = false;
    let mut harness = testkit::panel([520.0, 260.0], testkit::asked("q"), move |ui, cx| {
        if !installed {
            cx.media.images = Images::new(
                ui.ctx(),
                Offload::Manual,
                Box::new(FolderFetch::new(root.clone())),
            );
            installed = true;
        }
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

    std::fs::copy(
        testkit::samples_folder().join(figure().url.strip_prefix("/content/").unwrap()),
        folder.path().join("figure.png"),
    )
    .expect("the picture appears on disk");
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
        url: "/content/notes.png".into(),
    };
    std::fs::write(folder.path().join("notes.png"), "this is not a picture")
        .expect("the text file is written");
    let mut harness = picture_panel(
        folder.path().into(),
        notes,
        "Notes",
        [520.0, 260.0],
        egui::vec2(480.0, 220.0),
    );
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
        url: "/content/gone.png".into(),
    };
    let mut harness = picture_panel(
        folder.path().into(),
        gone,
        "Small figure",
        [400.0, 120.0],
        egui::vec2(160.0, 64.0),
    );
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

fn picture_panel(
    root: PathBuf,
    image: ImageRef,
    alt: &'static str,
    panel: [f32; 2],
    size: egui::Vec2,
) -> Harness<'static, Host> {
    let mut installed = false;
    testkit::panel(panel, testkit::asked("q"), move |ui, cx| {
        if !installed {
            cx.media.images = Images::new(
                ui.ctx(),
                Offload::Manual,
                Box::new(FolderFetch::new(root.clone())),
            );
            installed = true;
        }
        images::show(ui, cx.media, &image, alt, size);
    })
}
