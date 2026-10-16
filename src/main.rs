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
    fyle::diagnostics::init_gui();

    // Parse command line arguments
    let automation = std::env::var("FYLE_AUTOMATION").as_deref() == Ok("1")
        || fyle::startup::automation_requested(std::env::args_os().skip(1));
    let launch_target = parse_args();

    log::info!(
        "Starting Fyle v{} ({})",
        VERSION,
        fyle::diagnostics::revision()
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

    let result = eframe::run_native(
        "Fyle",
        options,
        Box::new(move |cc| {
            setup_cyberpunk_theme(&cc.egui_ctx);
            let mut app = FileManager::new_with_target(launch_target);
            app.initialize_appearance(&cc.egui_ctx, cc.gl.as_deref());
            if automation {
                if let Err(error) = app.enable_automation(&cc.egui_ctx) {
                    log::error!("Harness startup failed: {error:#}");
                    app.harness_error(error.to_string());
                }
            }
            Box::new(app)
        }),
    );
    if let Err(error) = &result {
        log::error!("GUI startup failed: {error}");
        let _ = fyle::diagnostics::DiagnosticsStore::default().record_crash(
            "startup",
            &error.to_string(),
            "",
        );
    }
    result
}
