//! Reads the committed sample chapters and a folder with a broken chapter through `Catalogue`,
//! because only the real file system shows that a bad `chapter.json` does not hide the others.

use std::path::Path;

use ocr::{Catalogue, ChapterIndex, ContentError, DocumentName};

fn sample_content() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/content")
}

#[test]
fn the_catalogue_lists_every_chapter_under_a_root_and_names_what_it_cannot_read() {
    let root = sample_content();

    let catalogue = Catalogue::read(&root).unwrap();

    let listed: Vec<(&Path, u32)> = catalogue
        .chapters
        .iter()
        .map(|entry| {
            let relative = entry.folder.strip_prefix(&root).unwrap();
            (relative, entry.index.page_count)
        })
        .collect();
    assert_eq!(
        listed,
        [
            (Path::new("option-volatility-and-pricing/chapter-1"), 7),
            (Path::new("quanty-sample-notes/chapter-1"), 3),
            (Path::new("quanty-sample-notes/chapter-2"), 3),
        ]
    );
    assert!(catalogue.chapters.iter().all(|entry| entry.index.finished));
    assert!(
        catalogue.unreadable.is_empty(),
        "{:?}",
        catalogue.unreadable
    );

    let temporary = tempfile::tempdir().unwrap();
    let missing = Catalogue::read(&temporary.path().join("no-such-folder")).unwrap();
    assert!(missing.chapters.is_empty() && missing.unreadable.is_empty());

    // By folder name chapter 10 comes before chapter 2. By number it comes after.
    let broken_folder = temporary.path().join("a-book/chapter-1");
    let chapter_2 = temporary.path().join("b-book/chapter-2");
    let chapter_10 = temporary.path().join("b-book/chapter-10");
    let pictures_folder = temporary.path().join("images/0123456789abcdef");
    for folder in [&broken_folder, &chapter_2, &chapter_10, &pictures_folder] {
        std::fs::create_dir_all(folder).unwrap();
    }
    let broken_file = broken_folder.join("chapter.json");
    std::fs::write(&broken_file, "not json").unwrap();
    std::fs::write(pictures_folder.join("image.json"), "{}").unwrap();
    let sample = &catalogue.chapters[0].index;
    for (folder, number) in [(&chapter_2, 2), (&chapter_10, 10)] {
        let index = ChapterIndex {
            name: DocumentName::Chapter {
                number,
                name: "Sample Pages".to_owned(),
            },
            ..sample.clone()
        };
        index.write(folder).unwrap();
    }

    let partial = Catalogue::read(temporary.path()).unwrap();

    let listed: Vec<&Path> = partial
        .chapters
        .iter()
        .map(|entry| entry.folder.as_path())
        .collect();
    assert_eq!(
        listed,
        [chapter_2.as_path(), chapter_10.as_path()],
        "the chapters that can be read come back, in the order of their numbers"
    );
    assert!(
        matches!(
            &partial.unreadable[..],
            [ContentError::Parse { path, .. }] if path.as_path() == broken_file.as_path()
        ),
        "{:?}",
        partial.unreadable
    );
}
