//! The app that owns the window's state, and the drawing of the whole window: the top bar and
//! the panels of the open tab, each in the rectangle the layout gives it.

use std::time::Duration;
use web_time::Instant;

use eframe::egui;

use super::layout::{self, ShellRects};
use super::region::region;
use super::{balances, shortcuts, top_bar};
use crate::media::Media;
use crate::panels::{
    Locals, PanelCx, answer, ask_bar, concept_graph, datasets, follow_up, ingest, library,
    retrieval_path, source,
};
use crate::state::Shared;
use crate::theme::{self, color, space};
use crate::transport::Transport;
use crate::widgets::gallery;
use contract::{self, Effect, Intent, Panel, StartupFacts};

/// How long `logic` may spend on backend events before it hands the frame back.
const DRAIN_BUDGET: Duration = Duration::from_millis(4);

const GALLERY_SCENE: &str = "gallery";

/// A panel keeps the name of its region while it is maximised, so what egui remembers of it,
/// such as how far a list is scrolled, is the same before and after.
const SOURCE_REGION: &str = "source";
const GRAPH_REGION: &str = "concept_graph";

type FilePicker = Box<dyn FnMut(&mut Vec<Intent>)>;

pub struct App {
    ctx: egui::Context,
    shared: Shared,
    locals: Locals,
    gallery: gallery::State,
    media: Media,
    backend: Box<dyn Transport>,
    intents: Vec<Intent>,
    effects: Vec<Effect>,
    file_picker: FilePicker,
    model_picker: super::model_picker::ModelPicker,
}

impl App {
    /// Queues the initial health, catalogue and balance reads before the opening actions.
    pub fn new(
        creation: &eframe::CreationContext<'_>,
        transport: Box<dyn Transport>,
        facts: StartupFacts,
        opening: Vec<Intent>,
    ) -> App {
        let ctx = creation.egui_ctx.clone();
        // Fonts set during a pass exist only from the next pass, so the theme goes first.
        theme::install(&ctx);
        let media = Media::new(&ctx);

        let model_picker = super::model_picker::ModelPicker::new(&facts);
        let shared = Shared::new(facts);
        let mut intents = vec![
            Intent::RecheckHealth,
            Intent::RefreshCatalogue,
            Intent::ReadBalances,
        ];
        intents.extend(opening);
        App {
            ctx,
            shared,
            locals: Locals::default(),
            gallery: gallery::State::default(),
            media,
            backend: transport,
            intents,
            effects: Vec::new(),
            file_picker: Box::new(|_| {}),
            model_picker,
        }
    }

    pub fn shared(&self) -> &Shared {
        &self.shared
    }

    pub fn push(&mut self, intent: Intent) {
        self.intents.push(intent);
    }

    pub fn with_file_picker(mut self, pick: impl FnMut(&mut Vec<Intent>) + 'static) -> App {
        self.file_picker = Box::new(pick);
        self
    }

    pub fn with_fetch(mut self, fetch: Box<dyn crate::media::Fetch>) -> App {
        self.media = Media::with_fetch(&self.ctx, fetch);
        self
    }

    pub fn is_idle(&self) -> bool {
        self.backend.is_idle() && self.shared.is_at_rest() && self.media.is_idle()
    }

    /// An event from the backend can ask for something too, so this runs once even when no
    /// intent is queued.
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
                    Effect::OpenUrl(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
                    Effect::PickFile => (self.file_picker)(&mut self.intents),
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
        ui.painter().rect_filled(window, 0.0, color::DESK);
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
            .show(ui, |ui| {
                top_bar::draw(ui, rects.top_bar, &mut cx);
                self.model_picker.draw(
                    ui,
                    rects.top_bar,
                    self.backend.is_idle_but_for_balance_reads(),
                    cx.shared,
                    cx.intents,
                );
            });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            draw_tab(ui, &rects, &mut self.locals, &mut cx);
            region(ui, "footer", rects.footer, |ui| {
                let footer = rects.footer;
                let painter = ui.painter();
                let line = theme::hairline(painter, color::HAIRLINE);
                painter.hline(footer.x_range(), footer.top() + line.width / 2.0, line);
                let parts = layout::footer(footer);
                balances::show(ui, parts.accounts, &mut cx);
                let text = match self.backend.connection() {
                    contract::Connection::Connected => "Connected".to_owned(),
                    contract::Connection::Reconnecting { attempt } => {
                        format!("Reconnecting · attempt {attempt}")
                    }
                    contract::Connection::Lost => "Connection lost".to_owned(),
                };
                ui.put(
                    parts.connection,
                    egui::Label::new(crate::theme::TextRole::Small.rich(text)),
                );
            });
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

fn draw_tab(ui: &mut egui::Ui, rects: &ShellRects, locals: &mut Locals, cx: &mut PanelCx<'_>) {
    match cx.shared.tab {
        contract::Tab::Ask => match cx.shared.maximised {
            Some(Panel::Source) => region(ui, SOURCE_REGION, rects.page, |ui| {
                source::show(ui, &mut locals.source, cx);
            }),
            Some(Panel::ConceptGraph) => region(ui, GRAPH_REGION, rects.page, |ui| {
                concept_graph::show(ui, &mut locals.concept_graph, cx);
            }),
            None => {
                region(ui, "ask_bar", rects.ask_bar, |ui| {
                    ask_bar::show(ui, &mut locals.ask_bar, cx);
                });
                region(ui, "answer", rects.answer, |ui| {
                    answer::show(ui, &mut locals.answer, cx);
                });
                region(ui, SOURCE_REGION, rects.source, |ui| {
                    source::show(ui, &mut locals.source, cx);
                });
                region(ui, GRAPH_REGION, rects.concept_graph, |ui| {
                    concept_graph::show(ui, &mut locals.concept_graph, cx);
                });
                region(ui, "retrieval_path", rects.retrieval_path, |ui| {
                    retrieval_path::show(ui, &mut locals.retrieval_path, cx);
                });
                region(ui, "follow_up", rects.follow_up, |ui| {
                    follow_up::show(ui, &mut locals.follow_up, cx);
                });
            }
        },
        contract::Tab::Library => region(ui, "library", rects.page, |ui| {
            library::show(ui, &mut locals.library, cx);
        }),
        contract::Tab::Ingest => region(ui, "ingest", rects.page, |ui| {
            ingest::show(ui, &mut locals.ingest, cx);
        }),
        contract::Tab::Datasets => region(ui, "datasets", rects.page, |ui| {
            datasets::show(ui, &mut locals.datasets, cx);
        }),
    }
}
