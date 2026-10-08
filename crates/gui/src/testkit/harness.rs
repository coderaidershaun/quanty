//! Puts one panel, or the whole app, in a test window: drawn as the app draws it, with the theme
//! installed, the window background, and no file dialog.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use egui_kittest::Harness;

use crate::app::App;
use crate::backend::Handler;
use crate::backend::fake::Fake;
use crate::contract::{Intent, StartupFacts};
use crate::media::Media;
use crate::panels::PanelCx;
use crate::state::Shared;
use crate::theme::{self, color};

const PIXELS_PER_POINT: f32 = 2.0;
const SETTLE_LIMIT: Duration = Duration::from_secs(5);

type Draw = Box<dyn FnMut(&mut egui::Ui, &mut PanelCx<'_>)>;

/// Look at `shared` and `intents` through `harness.state()`.
pub struct Host {
    pub shared: Shared,
    pub media: Media,
    pub intents: Vec<Intent>,
    ctx: egui::Context,
    panel_size: egui::Vec2,
    draw: Draw,
}

impl eframe::App for Host {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.media.poll(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let window = ui.max_rect();
        ui.painter().rect_filled(window, 0.0, color::CANVAS);
        let gutter = egui::Vec2::splat(theme::space::LG);
        let rect = egui::Rect::from_min_size(window.min + gutter, self.panel_size);
        let builder = egui::UiBuilder::new()
            .id_salt("panel")
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min));
        let mut cx = PanelCx {
            shared: &self.shared,
            media: &mut self.media,
            intents: &mut self.intents,
        };
        let draw = &mut self.draw;
        ui.scope_builder(builder, |ui| {
            ui.set_clip_rect(rect);
            draw(ui, &mut cx);
        });
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        // egui panics in a debug build when a textures change is still waiting at the end.
        self.ctx.tex_manager().write().take_delta().clear();
    }
}

/// `size` is the panel's outer rectangle. The window is that plus the gutter on every side, so
/// the panel sits where the shell would put it.
///
/// The media is manual, and the host polls it at the start of every frame. To see pictures,
/// call `harness.state_mut().media.run_pending()`, then `harness.run()`.
pub fn panel(
    size: [f32; 2],
    shared: Shared,
    draw: impl FnMut(&mut egui::Ui, &mut PanelCx<'_>) + 'static,
) -> Harness<'static, Host> {
    let panel_size = egui::Vec2::from(size);
    let window = panel_size + egui::Vec2::splat(2.0 * theme::space::LG);
    Harness::builder()
        .with_size(window)
        .with_pixels_per_point(PIXELS_PER_POINT)
        .build_eframe(move |creation| {
            theme::install(&creation.egui_ctx);
            Host {
                shared,
                media: Media::manual(&creation.egui_ctx),
                intents: Vec::new(),
                ctx: creation.egui_ctx.clone(),
                panel_size,
                draw: Box::new(draw),
            }
        })
}

/// The whole app on the fake backend, with no waits and no file dialog.
///
/// # Panics
/// When the scene does not exist or the backend cannot start.
pub fn app(scene: &str, size: [f32; 2]) -> Harness<'static, App> {
    app_at(scene, size, PIXELS_PER_POINT)
}

/// A picture that is kept on disk is drawn at 1.0, so that it stays small.
///
/// # Panics
/// When the scene does not exist or the backend cannot start.
pub fn app_at(scene: &str, size: [f32; 2], pixels_per_point: f32) -> Harness<'static, App> {
    let home = repository_root();
    let fake = Fake::scene(scene, &home)
        .unwrap_or_else(|error| panic!("the test cannot start the scene `{scene}`: {error}"))
        .instant();
    let opening = fake.opening();
    let facts = StartupFacts {
        home,
        env_file: None,
        fixture: Some(scene.to_owned()),
        anthropic_api_key_set: false,
    };
    build_app(fake, facts, opening, size, pixels_per_point)
}

/// The whole app on any backend, with no file dialog.
///
/// # Panics
/// When the backend's threads cannot start.
pub fn app_on(
    handler: impl Handler,
    facts: StartupFacts,
    opening: Vec<Intent>,
    size: [f32; 2],
) -> Harness<'static, App> {
    build_app(handler, facts, opening, size, PIXELS_PER_POINT)
}

fn build_app(
    handler: impl Handler,
    facts: StartupFacts,
    opening: Vec<Intent>,
    size: [f32; 2],
    pixels_per_point: f32,
) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::Vec2::from(size))
        .with_pixels_per_point(pixels_per_point)
        .build_eframe(move |creation| {
            App::new(creation, handler, facts, opening)
                .expect("the backend's threads start")
                .with_file_picker(|| None)
        })
}

/// # Panics
/// When the app is still busy after five seconds.
pub fn settle(harness: &mut Harness<'_, App>) {
    settle_within(harness, SETTLE_LIMIT);
}

/// As `settle`, with the limit a slow backend needs.
pub fn settle_within(harness: &mut Harness<'_, App>, limit: Duration) {
    let started = Instant::now();
    loop {
        // A run that is cut short is not a failure here: the check below says whether the app
        // is idle.
        harness.run_ok();
        if harness.state().is_idle() {
            return;
        }
        assert!(
            started.elapsed() < limit,
            "the app was still busy after {limit:?}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

pub fn save_png<S>(harness: &mut Harness<'_, S>, name: &str) {
    let Some(folder) = std::env::var_os("QUANTY_PNG_DIR") else {
        return;
    };
    // The harness paints a pointer after a click. Take it away before the picture.
    harness.remove_cursor();
    harness.run_ok();
    let image = harness.render().expect("the test renderer draws the frame");
    let folder = PathBuf::from(folder);
    std::fs::create_dir_all(&folder).expect("the folder for the pictures can be made");
    image
        .save(folder.join(format!("{name}.png")))
        .expect("the picture is written");
}

pub(super) fn repository_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map_or(manifest.clone(), PathBuf::from)
}
