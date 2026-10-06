//! Checks on the pictures cut for figures in the stubbed chapter run. There are no tests in this
//! file.

use std::path::Path;

use ocr::content::ImageShows;
use ocr::reader::{PieceId, read_chapter};

use crate::stubs::{Call, StubServices, page_folder, read_json};

/// The figures the run really cuts: the page, the figure's place in that page's `pieces`, the
/// rectangle the stub gives, and the rectangle `ocr` must draw. Page 2's figure matches
/// nothing printed on its page, so its rectangle is only padded. Page 6's is trimmed above the
/// body text under it. Page 7's grows to take in a label line far above its top edge.
const CUT_ROWS: [(u32, usize, [i64; 4], [i64; 4]); 3] = [
    (2, 4, [0, 200, 700, 500], [0, 185, 715, 515]),
    (6, 4, [115, 55, 950, 675], [100, 25, 965, 440]),
    (7, 2, [80, 578, 950, 945], [65, 510, 965, 960]),
];

fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert_eq!(bytes[..8], [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let number =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    (number(16), number(20))
}

fn sides(rectangle: &serde_json::Value) -> [i64; 4] {
    ["left", "top", "right", "bottom"].map(|side| rectangle[side].as_i64().unwrap())
}

/// Each figure of `CUT_ROWS` saved its rectangle as given, was drawn from the rectangle wanted
/// (two thousandths either way, because Poppler's boxes are floats) with no body text left in it,
/// and its picture has the pixel size that rectangle makes at 200 dpi.
pub fn assert_figures_cut(chapter: &Path) {
    for (position, index, given, wanted) in CUT_ROWS {
        let folder = page_folder(chapter, position);
        let page = read_json(&folder.join("page.json"));
        let piece = &page["pieces"][index];
        assert_eq!(sides(&piece["bounds"]), given, "page {position}");
        let image = &piece["image"];
        let file = format!("{:02}-figure.png", piece["number"].as_u64().unwrap());
        assert_eq!(
            (&image["file"], &image["shows"]),
            (&file.as_str().into(), &"figure".into())
        );
        assert_eq!(image["holds-body-text"], false, "page {position}");
        // Page 2's canned figure has none of its words on the page, so nothing could check the
        // rectangle the model gave. The other two found their own lines.
        assert_eq!(image["unchecked"], position == 2, "page {position}");
        let cut = sides(&image["cut"]);
        for (found, expected) in cut.iter().zip(wanted) {
            assert!(
                (found - expected).abs() <= 2,
                "page {position}: cut {cut:?}, wanted {wanted:?}"
            );
        }
        assert!(
            page["conversion"]["checks"]["whole-page-figures"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let (picture_width, picture_height) = png_size(&folder.join("page.png"));
        let (page_width, page_height) = (
            i64::from(picture_width) * 200 / 150,
            i64::from(picture_height) * 200 / 150,
        );
        let expected_width = (cut[2] * page_width + 999) / 1000 - cut[0] * page_width / 1000;
        let expected_height = (cut[3] * page_height + 999) / 1000 - cut[1] * page_height / 1000;
        let (width, height) = png_size(&folder.join(&file));
        assert!(
            (i64::from(width) - expected_width).abs() <= 2
                && (i64::from(height) - expected_height).abs() <= 2,
            "page {position}: picture {width}x{height}, wanted {expected_width}x{expected_height}"
        );
    }
}

/// Pages 4 and 5 each get two faulty replies, and the one whose only fault is its rectangle must
/// be the one saved. On page 4 that is the second reply. On page 5 it is the first, because the
/// second has a broken formula as well as a bad rectangle. Both pages fall back to the whole
/// page, are listed, and keep a formula that balances.
pub fn assert_hard_fallbacks(chapter: &Path, stubs: &StubServices) {
    for position in [4, 5] {
        let folder = page_folder(chapter, position);
        let page = read_json(&folder.join("page.json"));
        let formula = std::fs::read_to_string(folder.join("03-formula.tex")).unwrap();
        assert_eq!(formula.matches('{').count(), formula.matches('}').count());
        let image = &page["pieces"][4]["image"];
        assert_eq!(
            (&image["file"], &image["shows"]),
            (&"page.png".into(), &"whole-page".into())
        );
        assert!(image["cut"].is_null() && image["holds-body-text"] == false);
        let checks = &page["conversion"]["checks"];
        let fallbacks = checks["whole-page-figures"].as_array().unwrap();
        assert_eq!(fallbacks.len(), 1, "page {position}");
        assert_eq!(fallbacks[0]["piece"], 5);
        assert!(!fallbacks[0]["why"].as_str().unwrap().is_empty());
        assert_eq!(checks["reply-retried"], true);
        assert!(checks["retry-reason"].is_string());
        assert!(!folder.join("05-figure.png").exists());
        assert!(!folder.join("rejected-reply.json").exists());
    }
    let page_five = read_json(&page_folder(chapter, 5).join("page.json"));
    assert!(
        page_five["conversion"]["checks"]["retry-reason"]
            .as_str()
            .unwrap()
            .contains("bounds")
    );
    let corrections: Vec<Option<String>> = stubs
        .calls_for(5)
        .into_iter()
        .filter_map(|call| match call {
            Call::Transcribe { correction, .. } => Some(correction),
            _ => None,
        })
        .collect();
    assert_eq!(corrections.len(), 2);
    assert!(corrections[0].is_none());
    assert!(corrections[1].as_deref().unwrap().contains("bounds"));
}

/// `read_chapter` says which file is each figure's picture and what it shows.
pub fn assert_figure_pictures_read_back(chapter: &Path) {
    let read = read_chapter(chapter).unwrap();
    let picture = |page, number| {
        read.piece(PieceId { page, number })
            .unwrap()
            .figure_image
            .clone()
    };
    let cut = picture(2, 5).unwrap();
    assert_eq!(cut.path, chapter.join("page-num-2/05-figure.png"));
    assert_eq!(cut.shows, ImageShows::Figure);
    let whole = picture(5, 5).unwrap();
    assert_eq!(whole.path, chapter.join("page-num-5/page.png"));
    assert_eq!(whole.shows, ImageShows::WholePage);
    assert!(picture(2, 4).is_none());
}
