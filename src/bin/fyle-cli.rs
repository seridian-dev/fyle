//! Headless tools; builds without the GUI using --no-default-features.
use fyle::core::{get_system_terminal, is_application_available};
fn main() -> anyhow::Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("doctor") => {
            println!("Fyle {}", env!("CARGO_PKG_VERSION"));
            for key in [
                "XDG_CURRENT_DESKTOP",
                "XDG_SESSION_TYPE",
                "WAYLAND_DISPLAY",
                "DISPLAY",
                "DBUS_SESSION_BUS_ADDRESS",
                "TERMINAL",
            ] {
                // Report presence of the bus rather than its address.
                let value = std::env::var(key).unwrap_or_default();
                println!(
                    "{key}: {}",
                    if key == "DBUS_SESSION_BUS_ADDRESS" && !value.is_empty() {
                        "set"
                    } else {
                        &value
                    }
                );
            }
            println!("Terminal: {}", get_system_terminal());
            println!(
                "xdg-open available: {}",
                is_application_available("xdg-open")
            );
            println!("Init system: no dependency (OpenRC and systemd supported)");
        }
        _ => println!("Usage: fyle-cli doctor"),
    }
    Ok(())
}
