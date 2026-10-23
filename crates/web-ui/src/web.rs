//! Browser callbacks feed the app without keeping a borrowed intent list across an await.

use std::sync::mpsc::{Receiver, channel};

use contract::{Intent, StartupFacts};
use eframe::egui;
use wasm_bindgen::JsCast as _;

use crate::fetch::EhttpFetch;
use crate::transport::SocketTransport;
use crate::upload;

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
    let document = web_sys::window()
        .expect("the browser provides a window")
        .document()
        .expect("the browser provides a document");
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
                let app = gui::app::App::new(creation, Box::new(transport), facts, vec![])
                    .with_fetch(Box::new(EhttpFetch))
                    .with_file_picker(move |_| upload::pick(sender.clone(), ctx.clone()));
                Ok(Box::new(BrowserApp { app, intents }))
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

struct BrowserApp {
    app: gui::app::App,
    intents: Receiver<Intent>,
}

impl eframe::App for BrowserApp {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        for intent in self.intents.try_iter() {
            self.app.push(intent);
        }
        self.app.logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.app.clear_color(visuals)
    }
}
