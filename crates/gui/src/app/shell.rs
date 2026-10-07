//! The app that owns the window's state, and the drawing of the whole window: the top bar and
//! the panels of the open tab, each in the rectangle the layout gives it.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

use super::layout::{self, ShellRects};
use super::{shortcuts, top_bar};
use crate::backend::{Backend, Handler};
use crate::contract::{self, Effect, Intent, StartupFacts};
use crate::media::Media;
use crate::panels::{
    Locals, PanelCx, answer, ask_bar, concept_graph, follow_up, ingest, library, retrieval_path,
    source,
};
use crate::state::Shared;
use crate::theme::{self, color, space};
use crate::widgets::gallery::{self, GalleryState};

/// How long `logic` may spend on backend events before it hands the frame back.
const DRAIN_BUDGET: Duration = Duration::from_millis(4);

/// The scene that shows the widget kit instead of the app.
const GALLERY_SCENE: &str = "gallery";

type FilePicker = Box<dyn FnMut() -> Option<PathBuf>>;

pub struct App {
    shared: Shared,
    locals: Locals,
    gallery: GalleryState,
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
        // Fonts set during a pass exist only from the next pass, so the theme goes first.
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
            gallery: GalleryState::default(),
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

    /// Applies every queued intent, then does what the shared state asked for. An event from
    /// the backend can ask for something too, so this runs once even when no intent is queued.
    fn apply(&mut self, ctx: &egui::Context) {
        loop {
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
            // Picking a file queues one more intent.
            if self.intents.is_empty() {
                break;
            }
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let window = ui.max_rect();
        // The test renderer ignores the clear colour, so the window paints its own background.
        ui.painter().rect_filled(window, 0.0, color::CANVAS);
        if self.shared.health.facts.fixture.as_deref() == Some(GALLERY_SCENE) {
            egui::CentralPanel::no_frame().show(ui, |ui| {
                region(ui, "gallery", window.shrink(space::LG), |ui| {
                    gallery::show(ui, &mut self.gallery);
                });
            });
            return;
        }
        let rects = layout::shell(window);
        let mut cx = PanelCx {
            shared: &self.shared,
            media: &mut self.media,
            intents: &mut self.intents,
        };
        egui::Panel::top("top_bar")
            .exact_size(layout::TOP_BAR)
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| top_bar::draw(ui, rects.top_bar, &mut cx));
        egui::CentralPanel::no_frame().show(ui, |ui| {
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            draw_tab(ui, &rects, &mut self.locals, &mut cx);
        });
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
        self.apply(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
        // egui may lay a frame out twice, and only the first pass gets the input. So the intents
        // are applied after every pass. Never clear them when a pass is thrown away: that would
        // lose the click.
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

/// Gives `draw` a child ui whose area is exactly `rect`, clipped to it.
pub(super) fn region(
    ui: &mut egui::Ui,
    name: &str,
    rect: egui::Rect,
    draw: impl FnOnce(&mut egui::Ui),
) {
    let builder = egui::UiBuilder::new()
        .id_salt(name)
        .max_rect(rect)
        .layout(egui::Layout::top_down(egui::Align::Min));
    ui.scope_builder(builder, |ui| {
        ui.set_clip_rect(rect);
        draw(ui);
    });
}

fn draw_tab(ui: &mut egui::Ui, rects: &ShellRects, locals: &mut Locals, cx: &mut PanelCx<'_>) {
    match cx.shared.tab {
        contract::Tab::Ask => {
            region(ui, "ask_bar", rects.ask_bar, |ui| {
                ask_bar::show(ui, &mut locals.ask_bar, cx);
            });
            region(ui, "answer", rects.answer, |ui| {
                answer::show(ui, &mut locals.answer, cx);
            });
            region(ui, "source", rects.source, |ui| {
                source::show(ui, &mut locals.source, cx);
            });
            region(ui, "concept_graph", rects.concept_graph, |ui| {
                concept_graph::show(ui, &mut locals.concept_graph, cx);
            });
            region(ui, "retrieval_path", rects.retrieval_path, |ui| {
                retrieval_path::show(ui, &mut locals.retrieval_path, cx);
            });
            region(ui, "follow_up", rects.follow_up, |ui| {
                follow_up::show(ui, &mut locals.follow_up, cx);
            });
        }
        contract::Tab::Library => region(ui, "library", rects.page, |ui| {
            library::show(ui, &mut locals.library, cx);
        }),
        contract::Tab::Ingest => region(ui, "ingest", rects.page, |ui| {
            ingest::show(ui, &mut locals.ingest, cx);
        }),
    }
}
