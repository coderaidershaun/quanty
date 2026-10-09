//! The shapes saved for a figure: the rectangle it was cut from, what its picture shows, and
//! the name of the picture file.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A rectangle in the picture of the page, in thousandths of the picture's width and height.
///
/// The numbers are whole and are not limited by the schema, so that any number the model writes
/// reaches the reply check instead of failing the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
// Do not remove: without it the doc comment above is sent to the model as the description.
#[schemars(
    description = "A rectangle in the picture of the page, in thousandths of the picture's width and height counted from the picture's top-left corner."
)]
pub struct PageBox {
    #[schemars(
        description = "The left edge: thousandths of the picture's width from the picture's left edge, 0 to 1000."
    )]
    pub left: i32,
    #[schemars(
        description = "The top edge: thousandths of the picture's height from the picture's top edge, 0 to 1000."
    )]
    pub top: i32,
    #[schemars(
        description = "The right edge: thousandths of the picture's width from the picture's left edge, 0 to 1000, more than left."
    )]
    pub right: i32,
    #[schemars(
        description = "The bottom edge: thousandths of the picture's height from the picture's top edge, 0 to 1000, more than top."
    )]
    pub bottom: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "shows",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case"
)]
pub enum ImageShows {
    /// The figure, cut out of the page.
    Figure {
        /// The rectangle the picture was drawn from, padding included.
        #[serde(default)]
        cut: Option<PageBox>,
        /// A line of another piece of the page is still wholly inside the picture, so the
        /// picture may show body text.
        #[serde(default)]
        holds_body_text: bool,
        /// The model's rectangle could not be checked: none of the figure's own printed lines
        /// was found in the page's text layer, the layer was unreadable or the page is turned,
        /// or the refined rectangle came out unusable. The picture is cut from the model's
        /// rectangle as given, plus padding, and may hold a paragraph or miss part of the
        /// figure.
        #[serde(default)]
        unchecked: bool,
    },
    /// The whole page, because the figure could not be cut out.
    WholePage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FigureImage {
    /// A file name in the page's folder.
    pub file: String,
    #[serde(flatten)]
    pub shows: ImageShows,
}

pub(crate) fn figure_image_file_name(number: u32) -> String {
    format!("{number:02}-figure.png")
}

#[cfg(test)]
mod tests {
    use super::{FigureImage, ImageShows, PageBox};

    #[test]
    fn saved_pictures_read_with_their_figure_marks_or_without_them() {
        let whole_page = r#"{"file": "page.png", "shows": "whole-page", "cut": null,
            "holds-body-text": false, "unchecked": false}"#;
        let figure = r#"{"file": "01-figure.png", "shows": "figure",
            "cut": {"left": 1, "top": 2, "right": 3, "bottom": 4}, "holds-body-text": true}"#;

        let read = |text| serde_json::from_str::<FigureImage>(text).unwrap().shows;
        assert_eq!(read(whole_page), ImageShows::WholePage);
        assert_eq!(
            read(figure),
            ImageShows::Figure {
                cut: Some(PageBox {
                    left: 1,
                    top: 2,
                    right: 3,
                    bottom: 4
                }),
                holds_body_text: true,
                unchecked: false,
            }
        );
    }
}
