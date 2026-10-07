//! The window itself: it drains what the backend sent, draws the panels, and applies what they
//! asked for.

pub mod launch;
pub mod layout;
mod shell;
mod shortcuts;
mod top_bar;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::backend::{Backend, Handler};
use crate::contract::{Effect, Intent, StartupFacts};
use crate::media::Media;
use crate::panels::Locals;
use crate::state::{Quit, Shared};
use crate::theme;

/// How long `logic` may spend on backend events before it hands the frame back.
const DRAIN_BUDGET: Duration = Duration::from_millis(4);

type FilePicker = Box<dyn FnMut() -> Option<PathBuf>>;

pub struct App {
    shared: Shared,
    locals: Locals,
    media: Media,
    backend: Backend,
    intents: Vec<Intent>,
    effects: Vec<Effect>,
    pick_file: FilePicker,
}

impl App {
    /// Makes the app inside the eframe creation closure: installs the theme, starts the backend,
    /// and queues a load of the catalogue, then the intents of `opening`.
    ///
    /// # Errors
    /// Whatever the operating system says when the backend's threads cannot start.
    pub fn new(
        creation: &eframe::CreationContext<'_>,
        handler: impl Handler,
        facts: StartupFacts,
        opening: Vec<Intent>,
    ) -> std::io::Result<App> {
        let ctx = creation.egui_ctx.clone();
        theme::install(&ctx);
        let media = Media::new(&ctx);
        let backend = Backend::start(handler, {
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        })?;
        let mut intents = vec![Intent::RefreshCatalogue];
        intents.extend(opening);
        Ok(App {
            shared: Shared::new(facts),
            locals: Locals::default(),
            media,
            backend,
            intents,
            effects: Vec::new(),
            pick_file: Box::new(|| None),
        })
    }

    pub fn shared(&self) -> &Shared {
        &self.shared
    }

    /// Queues an intent. It is applied at the end of the next frame.
    pub fn push(&mut self, intent: Intent) {
        self.intents.push(intent);
    }

    /// Replaces the file dialog. A test installs one that returns `None`.
    pub fn with_file_picker(mut self, pick: impl FnMut() -> Option<PathBuf> + 'static) -> App {
        self.pick_file = Box::new(pick);
        self
    }

    /// True when the backend, the shared state and the media have nothing left to do.
    pub fn is_idle(&self) -> bool {
        self.backend.is_idle() && self.shared.is_at_rest() && self.media.is_idle()
    }

    /// Applies every queued intent, then does what the shared state asked for.
    fn apply(&mut self, ctx: &egui::Context) {
        while !self.intents.is_empty() {
            let mut intents = std::mem::take(&mut self.intents);
            for intent in intents.drain(..) {
                self.shared.apply_intent(intent, &mut self.effects);
            }
            self.intents = intents;
            let mut effects = std::mem::take(&mut self.effects);
            for effect in effects.drain(..) {
                match effect {
                    Effect::Send(command) => self.backend.send(command),
                    Effect::CopyText(text) => ctx.copy_text(text),
                    Effect::CloseWindow => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Effect::PickFile => {
                        if let Some(path) = (self.pick_file)() {
                            self.intents.push(Intent::PdfPicked(path));
                        }
                    }
                }
            }
            self.effects = effects;
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let started = Instant::now();
        while let Some(event) = self.backend.try_next() {
            self.shared.apply_event(event, &mut self.effects);
            if started.elapsed() > DRAIN_BUDGET {
                ctx.request_repaint();
                break;
            }
        }
        self.media.poll(ctx);
        shortcuts::read(ctx, &mut self.intents);
        let closing = ctx.input(|input| input.viewport().close_requested());
        if closing && self.shared.quit == Quit::No && self.shared.ingest.is_running() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.intents.push(Intent::RequestQuit);
        }
        self.apply(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        shell::draw(
            ui,
            &self.shared,
            &mut self.locals,
            &mut self.media,
            &mut self.intents,
        );
        if !self.intents.is_empty() {
            self.apply(ui.ctx());
            ui.ctx().request_repaint();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::clear_color()
    }
}

/// Opens the window and runs the app until it closes.
///
/// # Errors
/// Whatever eframe reports when the window cannot be made.
pub fn run(handler: impl Handler, facts: StartupFacts, opening: Vec<Intent>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("quanty")
            .with_inner_size(layout::DEFAULT_WINDOW)
            .with_min_inner_size(layout::MIN_WINDOW)
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "quanty",
        options,
        Box::new(move |creation| Ok(Box::new(App::new(creation, handler, facts, opening)?))),
    )
}
