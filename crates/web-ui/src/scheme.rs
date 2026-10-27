//! The palette the person chose is kept in the browser, and the page around the canvas takes its
//! colour, because a phone shows the page in the margins of its safe area.

use gui::theme::Scheme;
use wasm_bindgen::JsCast as _;

const KEY: &str = "quanty-scheme";

/// Both `theme-color` tags: the browser picks one by the person's system setting, and the page
/// must carry the app's colour whichever one it picks.
const THEME_COLOR_TAGS: &str = r#"meta[name="theme-color"]"#;

/// What the person chose on an earlier visit. It is dark when nothing was kept, and when the
/// browser does not let the page read it, as in some private windows.
pub(crate) fn stored(window: &web_sys::Window) -> Scheme {
    let value = storage(window).and_then(|storage| {
        storage.get_item(KEY).unwrap_or_else(|error| {
            log::warn!("could not read the colour choice kept under {KEY}: {error:?}");
            None
        })
    });
    Scheme::from_stored(value.as_deref())
}

fn storage(window: &web_sys::Window) -> Option<web_sys::Storage> {
    match window.local_storage() {
        Ok(Some(storage)) => Some(storage),
        Ok(None) => {
            log::warn!("the browser gives no local storage, so the colour choice is not kept");
            None
        }
        Err(error) => {
            log::warn!("the browser does not let the page use its local storage: {error:?}");
            None
        }
    }
}

/// The scheme that the page around the canvas shows.
pub(crate) struct PageScheme {
    window: web_sys::Window,
    shown: Scheme,
}

impl PageScheme {
    pub(crate) fn new(window: &web_sys::Window, scheme: Scheme) -> Self {
        paint_page(window, scheme);
        Self {
            window: window.clone(),
            shown: scheme,
        }
    }

    /// Call after each frame with the scheme the person chose. A new choice paints the page and
    /// is kept for the next visit. A failure to keep it only warns, because the app still works.
    pub(crate) fn after_frame(&mut self, scheme: Scheme) {
        if scheme == self.shown {
            return;
        }
        self.shown = scheme;
        paint_page(&self.window, scheme);
        keep(&self.window, scheme);
    }
}

fn keep(window: &web_sys::Window, scheme: Scheme) {
    if let Some(storage) = storage(window)
        && let Err(error) = storage.set_item(KEY, scheme.as_stored())
    {
        log::warn!(
            "could not keep the {} choice under {KEY}: {error:?}",
            scheme.as_stored()
        );
    }
}

fn paint_page(window: &web_sys::Window, scheme: Scheme) {
    let desk = scheme.palette().desk;
    let color = format!("#{:02x}{:02x}{:02x}", desk.r(), desk.g(), desk.b());
    let Some(document) = window.document() else {
        log::warn!("the window has no document to paint {color} on");
        return;
    };
    match document.body() {
        Some(body) => {
            if let Err(error) = body.style().set_property("background", &color) {
                log::warn!("could not paint the page background {color}: {error:?}");
            }
        }
        None => log::warn!("the document has no body to paint {color} on"),
    }
    match document.query_selector_all(THEME_COLOR_TAGS) {
        Ok(tags) => {
            for index in 0..tags.length() {
                if let Some(tag) = tags
                    .get(index)
                    .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
                    && let Err(error) = tag.set_attribute("content", &color)
                {
                    log::warn!("could not paint a theme-color tag {color}: {error:?}");
                }
            }
        }
        Err(error) => log::warn!("could not find the theme-color tags to paint {color}: {error:?}"),
    }
}
