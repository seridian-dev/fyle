mod app;
mod theme;
mod ui;

use app::FileManager;
use fyle::startup::parse_args;
use fyle::{core, types};
use theme::setup_cyberpunk_theme;
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<(), eframe::Error> {
    // Initialize logging
    env_logger::init();

    // Parse command line arguments
    let initial_path = parse_args();

    log::info!(
        "Starting Fyle v{} with initial path: {}",
        VERSION,
        initial_path.display()
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([600.0, 400.0])
            .with_resizable(true)
            .with_title("Fyle - File Manager")
            .with_app_id("fyle"),
        ..Default::default()
    };

    eframe::run_native(
        "Fyle",
        options,
        Box::new(move |cc| {
            setup_cyberpunk_theme(&cc.egui_ctx);
            Box::new(FileManager::new(initial_path))
        }),
    )
}
