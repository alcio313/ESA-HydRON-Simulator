mod models;
mod config;
mod math;
mod physics;
mod simulation;
mod network;
mod app;

#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;
#[cfg(not(target_arch = "wasm32"))]
use config::load_config;
#[cfg(target_arch = "wasm32")]
use config::parse_config_from_str;
use app::{default_config, HydronGuiApp};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// Versione Desktop (Nativa)
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    println!("=== Starting HydRON-DT-Builder Interactive GUI Monitor ===");

    let config_path = "config.toml";
    let config = match load_config(config_path) {
        Ok(c) => c,
        Err(e) => {
            println!("Warning: config.toml could not be loaded: {}. Loading defaults.", e);
            default_config()
        }
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("HydRON Constellation Digital Twin Monitor")
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "HydRON Constellation Digital Twin Monitor",
        native_options,
        Box::new(|cc| Ok(Box::new(HydronGuiApp::new(cc, config)))),
    )
}

// Versione Web (WebAssembly)
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirige i panic sulla console degli strumenti sviluppatore del browser
    console_error_panic_hook::set_once();

    let web_options = eframe::WebOptions::default();
    let config_toml = include_str!("../config.toml");
    let config = match parse_config_from_str(config_toml) {
        Ok(c) => c,
        Err(_) => default_config(),
    };

    wasm_bindgen_futures::spawn_local(async {
        // eframe >= 0.28 requires the actual canvas element, not its id
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("the_canvas_id"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("Canvas element 'the_canvas_id' not found");

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(HydronGuiApp::new(cc, config)))),
            )
            .await
            .expect("Failed to start eframe");
    });
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = "
    export function download_file(filename, text) {
        const element = document.createElement('a');
        element.setAttribute('href', 'data:text/plain;charset=utf-8,' + encodeURIComponent(text));
        element.setAttribute('download', filename);
        element.style.display = 'none';
        document.body.appendChild(element);
        element.click();
        document.body.removeChild(element);
    }
")]
extern "C" {
    pub fn download_file(filename: &str, text: &str);
}
