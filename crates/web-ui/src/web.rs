//! Browser callbacks feed the app without keeping a borrowed intent list across an await. The
//! browser's back button, the phone back gesture and Alt+Left step back inside the app here.

use std::sync::mpsc::{Receiver, channel};

use contract::{Intent, StartupFacts};
use eframe::egui;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;

use crate::back::{BackHistory, MoveStep};
use crate::fetch::EhttpFetch;
use crate::transport::SocketTransport;
use crate::upload;

/// The state value of the one history entry the app adds.
const OWN_ENTRY: &str = "quanty-back";

pub(crate) fn start() {
    let mut request = ehttp::Request::get("api/facts");
    request.mode = ehttp::Mode::SameOrigin;
    ehttp::fetch(request, |result| match result {
        Ok(response) if response.status == 401 => {
            if let Some(window) = web_sys::window()
                && let Err(error) = window.location().set_href("login")
            {
                failed(&format!("could not open the login page: {error:?}"));
            }
        }
        Ok(response) if response.status == 200 => {
            match serde_json::from_slice::<StartupFacts>(&response.bytes) {
                Ok(facts) => wasm_bindgen_futures::spawn_local(run(facts)),
                Err(error) => failed(&format!("could not read server facts: {error}")),
            }
        }
        Ok(response) => failed(&format!(
            "could not load server facts: HTTP {}",
            response.status
        )),
        Err(error) => failed(&format!("could not load server facts: {error}")),
    });
}

async fn run(facts: StartupFacts) {
    let window = web_sys::window().expect("the browser provides a window");
    let document = window.document().expect("the browser provides a document");
    let Some(canvas) = document
        .get_element_by_id("the_canvas_id")
        .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
    else {
        failed("could not find the app canvas");
        return;
    };
    let result = eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |creation| {
                let (sender, intents) = channel();
                let ctx = creation.egui_ctx.clone();
                let transport = SocketTransport::connect("api/ws", ctx.clone(), sender.clone())
                    .map_err(|error| {
                        std::io::Error::other(format!("could not connect: {error:?}"))
                    })?;
                if let Err(error) = keep_page_turn_keys(&window) {
                    log::warn!("could not keep Cmd/Ctrl+[ and ] for the app: {error:?}");
                }
                let browser_back = BrowserBack::install(&window, &ctx);
                let app = gui::app::App::new(creation, Box::new(transport), facts, vec![])
                    .with_fetch(Box::new(EhttpFetch))
                    .with_file_picker(move |_| upload::pick(sender.clone(), ctx.clone()));
                Ok(Box::new(BrowserApp {
                    app,
                    intents,
                    browser_back,
                }))
            }),
        )
        .await;
    match result {
        Ok(()) => {
            if let Some(loading) = document.get_element_by_id("loading_text") {
                loading.remove();
            }
        }
        Err(error) => failed(&format!("could not start the app: {error:?}")),
    }
}

fn failed(message: &str) {
    log::error!("{message}");
    if let Some(loading) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("loading_text"))
    {
        loading.set_text_content(Some(message));
    }
}

/// Runs `callback` for every `event_name` event of the window, before the page's own elements see
/// it, because the canvas stops a key from travelling on to the window. The page needs the
/// callback for as long as it lives, so the closure is leaked on purpose instead of being stored
/// and dropped.
fn listen_on_window(
    window: &web_sys::Window,
    event_name: &str,
    callback: impl FnMut(web_sys::Event) + 'static,
) -> Result<(), JsValue> {
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(callback);
    let in_capture_phase = true;
    window.add_event_listener_with_callback_and_bool(
        event_name,
        closure.as_ref().unchecked_ref(),
        in_capture_phase,
    )?;
    closure.forget();
    Ok(())
}

/// The app turns the source page with Cmd or Ctrl and `[` or `]`, and a Mac browser takes the same
/// chords as back and forward, so the browser must not also move in its history.
fn keep_page_turn_keys(window: &web_sys::Window) -> Result<(), JsValue> {
    listen_on_window(window, "keydown", |event| {
        // On Windows the AltGr key, which many keyboards need to type a bracket, reports Ctrl
        // and Alt together, so a press with Alt is left alone or the bracket could not be typed.
        if let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>()
            && (key.meta_key() || key.ctrl_key())
            && !key.alt_key()
            && matches!(key.key().as_str(), "[" | "]")
        {
            key.prevent_default();
        }
    })
}

fn is_own_entry(browser_history: &web_sys::History) -> bool {
    match browser_history.state() {
        Ok(state) => state.as_string().as_deref() == Some(OWN_ENTRY),
        Err(error) => {
            log::warn!("could not read the browser history state: {error:?}");
            false
        }
    }
}

/// The browser's back button and the phone back gesture, as far as the app takes them.
struct BrowserBack {
    back: BackHistory,
    browser_history: web_sys::History,
    /// For each move of the browser in its history, whether it landed on the app's entry.
    moves: Receiver<bool>,
}

impl BrowserBack {
    /// `None` when the browser gives no history, or no way to hear it move: back then stays with
    /// the browser.
    fn install(window: &web_sys::Window, ctx: &egui::Context) -> Option<Self> {
        let browser_history = window
            .history()
            .inspect_err(|error| log::warn!("the browser gives no history: {error:?}"))
            .ok()?;
        let (sender, moves) = channel();
        let ctx = ctx.clone();
        let history_for_moves = browser_history.clone();
        listen_on_window(window, "popstate", move |_| {
            if sender.send(is_own_entry(&history_for_moves)).is_ok() {
                ctx.request_repaint();
            }
        })
        .inspect_err(|error| log::warn!("could not listen to the browser's back: {error:?}"))
        .ok()?;
        Some(Self {
            back: BackHistory::new(is_own_entry(&browser_history)),
            browser_history,
            moves,
        })
    }

    fn before_frame(&mut self, app: &mut gui::app::App) {
        for landed_on_own_entry in self.moves.try_iter() {
            match self.back.after_move(landed_on_own_entry, app.can_go_back()) {
                MoveStep::SendBack => app.push(Intent::Back),
                MoveStep::LeavePage => {
                    if let Err(error) = self.browser_history.back() {
                        log::warn!("could not leave the page: {error:?}");
                    }
                }
                MoveStep::Keep => {}
            }
        }
    }

    fn after_frame(&mut self, app: &gui::app::App) {
        if self.back.after_frame(app.can_go_back())
            && let Err(error) = self
                .browser_history
                .push_state(&JsValue::from_str(OWN_ENTRY), "")
        {
            log::warn!("could not add the app's entry to the browser history: {error:?}");
        }
    }
}

struct BrowserApp {
    app: gui::app::App,
    intents: Receiver<Intent>,
    browser_back: Option<BrowserBack>,
}

impl BrowserApp {
    /// Takes Alt+Left out of the input when it steps back inside the app, because eframe keeps
    /// arrow keys from the browser while the canvas has focus. A text box keeps the key, because
    /// there it moves the cursor by a word.
    fn consume_alt_left(&self, ctx: &egui::Context) -> bool {
        self.app.can_go_back()
            && !ctx.text_edit_focused()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft))
    }
}

impl eframe::App for BrowserApp {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        for intent in self.intents.try_iter() {
            self.app.push(intent);
        }
        if let Some(browser_back) = &mut self.browser_back {
            browser_back.before_frame(&mut self.app);
        }
        if self.consume_alt_left(ctx) {
            self.app.push(Intent::Back);
        }
        self.app.logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        if let Some(browser_back) = &mut self.browser_back {
            browser_back.after_frame(&self.app);
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.app.clear_color(visuals)
    }
}
