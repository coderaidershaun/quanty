//! The app that owns the window's state, and the drawing of the whole window: the top bar and
//! the panels of the open tab, each in the rectangle the layout gives it.

use std::time::Duration;
use web_time::Instant;

use eframe::egui;

use super::layout::{self, AskRegion, Form, ShellRects};
use super::region::region;
use super::{ask_nav, footer, shortcuts, top_bar};
use crate::media::Media;
use crate::panels::{
    Locals, PanelCx, answer, ask_bar, concept_graph, datasets, follow_up, ingest, library,
    retrieval_path, source,
};
use crate::state::Shared;
use crate::theme::{self, Scheme, space};
use crate::transport::Transport;
use crate::widgets::{self, gallery};
use contract::{self, AskView, Effect, Intent, Panel, StartupFacts};

/// How long `logic` may spend on backend events before it hands the frame back.
const DRAIN_BUDGET: Duration = Duration::from_millis(4);

const GALLERY_SCENE: &str = "gallery";

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
    /// The rectangles of the frame drawn last. They are those of a wide window until the first
    /// frame, and no input comes before that frame.
    rects: ShellRects,
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
            rects: layout::shell(
                egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::from(layout::DEFAULT_WINDOW),
                ),
                ask_bar::NoteRow::Absent,
            ),
        }
    }

    pub fn shared(&self) -> &Shared {
        &self.shared
    }

    /// The rectangles of every part of the window in the frame drawn last.
    pub fn shell_rects(&self) -> ShellRects {
        self.rects
    }

    pub fn push(&mut self, intent: Intent) {
        self.intents.push(intent);
    }

    /// Chooses the palette at once, for the host before the first frame and for tests between
    /// frames. A person's choice during a frame goes through `Intent::SwitchScheme`.
    pub fn set_scheme(&mut self, scheme: Scheme) {
        self.shared.scheme = scheme;
        theme::set_scheme(&self.ctx, scheme);
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

    /// Whether a Back would change what the window shows. An open sheet or popup is closed first,
    /// in every window. Only a compact window shows one view of the Ask tab at a time; a wider one
    /// keeps the Answer beside the others, so a Back that only returns to the Answer changes
    /// nothing there.
    pub fn can_go_back(&self) -> bool {
        widgets::is_sheet_or_popup_open(&self.ctx)
            || (self.shared.can_go_back()
                && (self.rects.form == Form::Compact || !self.shared.goes_back_to_the_answer()))
    }

    /// An event from the backend can ask for something too, so this runs once even when no
    /// intent is queued.
    fn apply(&mut self, ctx: &egui::Context) {
        loop {
            let mut intents = std::mem::take(&mut self.intents);
            for intent in intents.drain(..) {
                if matches!(intent, Intent::Back) {
                    if widgets::is_sheet_or_popup_open(ctx) {
                        widgets::close_the_top_sheet(ctx);
                        continue;
                    }
                    if !self.can_go_back() {
                        continue;
                    }
                }
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
        ui.painter()
            .rect_filled(window, 0.0, theme::palette(ui.ctx()).desk);
        if self.shared.health.facts.fixture.as_deref() == Some(GALLERY_SCENE) {
            egui::CentralPanel::no_frame().show(ui, |ui| {
                region(ui, "gallery", window.shrink(space::LG), |ui| {
                    gallery::show(ui, &mut self.gallery);
                });
            });
            return;
        }
        let rects = layout::shell(window, ask_bar::note_row(&self.shared));
        self.rects = rects;
        let mut cx = PanelCx {
            shared: &self.shared,
            media: &mut self.media,
            intents: &mut self.intents,
        };
        let places = top_bar::places(rects.top_bar, rects.form);
        egui::Panel::top("top_bar")
            .exact_size(rects.top_bar.height())
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                top_bar::draw(ui, &places, &mut cx);
                self.model_picker.draw(
                    ui,
                    places.models,
                    self.backend.is_idle_but_for_balance_reads(),
                    &mut cx,
                );
            });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
            draw_tab(ui, &rects, &mut self.locals, &mut cx);
            region(ui, "footer", rects.footer, |ui| {
                footer::draw(ui, &rects, self.backend.connection(), &mut cx);
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
        widgets::forget_the_close_request(ui.ctx());
        // egui may lay a frame out twice, and only the first pass gets the input. So the intents
        // are applied after every pass. Never clear them when a pass is thrown away: that would
        // lose the click.
        if !self.intents.is_empty() {
            self.apply(ui.ctx());
            ui.ctx().request_repaint();
        }
        // A palette that changes in the middle of a pass would draw one frame in two palettes. So
        // the choice reaches the window here, at the end of a pass, whoever queued it.
        if theme::scheme(ui.ctx()) != self.shared.scheme {
            theme::set_scheme(ui.ctx(), self.shared.scheme);
            ui.ctx().request_repaint();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::palette(&self.ctx).desk.to_normalized_gamma_f32()
    }
}

fn draw_tab(ui: &mut egui::Ui, rects: &ShellRects, locals: &mut Locals, cx: &mut PanelCx<'_>) {
    match cx.shared.tab {
        contract::Tab::Ask => match cx.shared.maximised {
            Some(Panel::Source) => draw_view(ui, AskView::Source, rects.page, locals, cx),
            Some(Panel::ConceptGraph) => {
                draw_view(ui, AskView::ConceptGraph, rects.page, locals, cx);
            }
            None => {
                let regions = rects.ask_regions(cx.shared.ask_view, cx.shared.side_view());
                for (part, rect) in regions {
                    draw_part(ui, part, rect, locals, cx);
                }
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

fn draw_part(
    ui: &mut egui::Ui,
    part: AskRegion,
    rect: egui::Rect,
    locals: &mut Locals,
    cx: &mut PanelCx<'_>,
) {
    match part {
        AskRegion::AskBar => region(ui, "ask_bar", rect, |ui| {
            ask_bar::show(ui, &mut locals.ask_bar, cx);
        }),
        AskRegion::View(view) => draw_view(ui, view, rect, locals, cx),
        AskRegion::SideTabs => region(ui, "side_tabs", rect, |ui| ask_nav::side_tabs(ui, cx)),
        AskRegion::BottomNav => region(ui, "bottom_nav", rect, |ui| ask_nav::bottom_nav(ui, cx)),
    }
}

/// A view keeps the name of its region in every form and while it is maximised, so what egui
/// remembers of it, such as how far a list is scrolled, is the same before and after.
fn draw_view(
    ui: &mut egui::Ui,
    view: AskView,
    rect: egui::Rect,
    locals: &mut Locals,
    cx: &mut PanelCx<'_>,
) {
    match view {
        AskView::Answer => region(ui, "answer", rect, |ui| {
            answer::show(ui, &mut locals.answer, cx);
        }),
        AskView::Source => region(ui, "source", rect, |ui| {
            source::show(ui, &mut locals.source, cx);
        }),
        AskView::ConceptGraph => region(ui, "concept_graph", rect, |ui| {
            concept_graph::show(ui, &mut locals.concept_graph, cx);
        }),
        AskView::RetrievalPath => region(ui, "retrieval_path", rect, |ui| {
            retrieval_path::show(ui, &mut locals.retrieval_path, cx);
        }),
        AskView::FollowUp => region(ui, "follow_up", rect, |ui| {
            follow_up::show(ui, &mut locals.follow_up, cx);
        }),
    }
}
