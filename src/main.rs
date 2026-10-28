mod app;
mod theme;
mod ui;

use app::FileManager;
use fyle::startup::parse_args;
use fyle::{core, types};
use theme::setup_cyberpunk_theme;
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn app_icon() -> Option<egui::IconData> {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
        "../assets/fyle-256.png"
    )));
    let mut reader = decoder.read_info().ok()?;
    if reader.output_color_type().0 != png::ColorType::Rgba {
        return None;
    }
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).ok()?;
    rgba.truncate(info.buffer_size());
    Some(egui::IconData {
        rgba,
        width: info.width,
        height: info.height,
    })
}

#[cfg(target_os = "macos")]
fn configure_macos_viewport(viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
    viewport
        .with_fullsize_content_view(true)
        .with_titlebar_shown(false)
        .with_title_shown(false)
}

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

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1024.0, 768.0])
        .with_min_inner_size([600.0, 400.0])
        .with_resizable(true)
        .with_title("Fyle")
        .with_app_id("dev.seridian.fyle");
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    #[cfg(target_os = "macos")]
    let viewport = configure_macos_viewport(viewport);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let result = eframe::run_native(
        "Fyle",
        options,
        Box::new(move |cc| {
            setup_cyberpunk_theme(&cc.egui_ctx);
            app::register_terminal_font(&cc.egui_ctx);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_app_icon_decodes_to_a_square_rgba_image() {
        let icon = app_icon().expect("bundled Fyle PNG icon should decode");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_titlebar_is_transparent_and_keeps_native_controls() {
        let viewport = configure_macos_viewport(egui::ViewportBuilder::default());
        assert_eq!(viewport.fullsize_content_view, Some(true));
        assert_eq!(viewport.titlebar_shown, Some(false));
        assert_eq!(viewport.title_shown, Some(false));
        assert_ne!(viewport.titlebar_buttons_shown, Some(false));
    }
}
