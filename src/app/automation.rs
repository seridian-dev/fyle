//! Integration of the headless control protocol with the actual egui event/render loop.
use super::*;
use anyhow::{bail, Context, Result};
use fyle::harness::{Reply, Request, Server};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) struct Automation {
    server: Server,
    epoch: u64,
    frame: u64,
    fingerprint: String,
    pending: Option<Request>,
    capture_state: Option<Value>,
    image: Option<std::sync::Arc<egui::ColorImage>>,
    close_after: Option<Instant>,
    capture_started: Option<Instant>,
}
impl FileManager {
    pub(crate) fn harness_error(&mut self, message: String) {
        self.error_message = Some(message);
    }
    pub(crate) fn enable_automation(&mut self, ctx: &egui::Context) -> Result<()> {
        let wakeup = ctx.clone();
        let server = Server::start(fyle::harness::directory(), move || wakeup.request_repaint())?;
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new(crate::ui::HARNESS_CONTROLS_ID),
                crate::ui::ControlMap::default(),
            )
        });
        self.automation = Some(Automation {
            server,
            epoch: 0,
            frame: 0,
            fingerprint: String::new(),
            pending: None,
            capture_state: None,
            image: None,
            close_after: None,
            capture_started: None,
        });
        Ok(())
    }
    fn describe_gui(&self, ctx: &egui::Context, offset: usize, limit: usize) -> Value {
        let controls = ctx
            .data(|data| {
                data.get_temp::<crate::ui::ControlMap>(egui::Id::new(
                    crate::ui::HARNESS_CONTROLS_ID,
                ))
            })
            .map(|controls| {
                controls
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .values()
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let entries: Vec<_> = self.entries.iter().skip(offset).take(limit.min(200)).map(|entry| json!({
            "name":entry.name, "path":entry.path.to_string_lossy(), "is_dir":entry.is_dir, "size":entry.size, "selected":entry.selected,
        })).collect();
        let rect = ctx.screen_rect();
        json!({
            "schema_version":1, "version":env!("CARGO_PKG_VERSION"), "revision":fyle::diagnostics::revision(),
            "path":self.current_path.to_string_lossy(), "view":format!("{:?}",self.view_mode).to_lowercase(),
            "query":self.search_query, "show_hidden":self.show_hidden,
            "inputs":{"new_folder_name":self.create_folder_state.name,"new_file_name":self.create_file_state.name,"rename_name":self.rename_state.as_ref().map(|state| &state.new_name)}, "loading":self.loading,
            "busy":self.operation.is_some(), "ready":!self.loading && self.operation.is_none(),
            "dialog": match &self.dialog { DialogType::None => "none", DialogType::ConfirmDelete(..) => "confirm_delete", DialogType::ConfirmCopy(..) => "confirm_copy", DialogType::ConfirmMove(..) => "confirm_move", DialogType::FileProperties(..) => "properties", DialogType::CreateFolder => "create_folder", DialogType::CreateFile => "create_file", DialogType::Rename(..) => "rename" }, "error":self.error_message,
            "total_entries":self.entries.len(), "entry_offset":offset, "entries":entries,
            "selected_count":self.selected_entries.len(), "can_go_back":!self.history.is_empty(), "can_go_forward":!self.forward_history.is_empty(),
            "viewport":{"width":rect.width(),"height":rect.height(),"pixels_per_point":ctx.pixels_per_point()},
            "context_menu":{"visible":self.context_menu.visible,"target":self.context_menu.target.as_ref().map(|entry| &entry.name)},
            "controls":controls,
        })
    }
    pub(super) fn automation_input(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        let Some(mut automation) = self.automation.take() else {
            return;
        };
        for event in &input.events {
            if let egui::Event::Screenshot { image, .. } = event {
                automation.image = Some(image.clone());
            } else {
                automation.epoch = automation.epoch.wrapping_add(1);
            }
        }
        self.poll_directory();
        self.poll_operation();
        let fingerprint = self.describe_gui(ctx, 0, 200).to_string();
        if fingerprint != automation.fingerprint {
            automation.epoch += 1;
            automation.fingerprint = fingerprint;
        }
        if automation.pending.is_none() {
            if let Some(request) = automation.server.try_receive() {
                let action = request.command["action"].as_str().unwrap_or("").to_owned();
                let result = self.apply_automation(ctx, input, &request.command, automation.epoch);
                match result {
                    Ok(()) => {
                        if action == "close" {
                            automation.close_after =
                                Some(Instant::now() + Duration::from_millis(150));
                        }
                        automation.pending = Some(request);
                        if !matches!(action.as_str(), "state" | "screenshot") {
                            automation.epoch += 1;
                        }
                    }
                    Err(error) => {
                        let _ = request.respond.send(Reply::error(error));
                    }
                }
            }
        }
        if let Some(controls) = ctx.data(|data| {
            data.get_temp::<crate::ui::ControlMap>(egui::Id::new(crate::ui::HARNESS_CONTROLS_ID))
        }) {
            controls
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clear();
        }
        self.automation = Some(automation);
    }
    fn apply_automation(
        &mut self,
        ctx: &egui::Context,
        input: &mut egui::RawInput,
        command: &Value,
        epoch: u64,
    ) -> Result<()> {
        let action = command["action"].as_str().context("Missing action")?;
        if !matches!(action, "state" | "screenshot") {
            anyhow::ensure!(self.operation.is_none(), "A file operation is running");
        }
        if let Some(expected) = command["snapshot_id"].as_u64() {
            anyhow::ensure!(
                expected == epoch,
                "Stale snapshot; capture fresh state/image before input"
            );
        }
        let text = |key| {
            command[key]
                .as_str()
                .with_context(|| format!("Missing {key}"))
        };
        match action {
            "state" | "screenshot" | "close" => {}
            "navigate" => {
                let path = fyle::startup::resolve_location(std::ffi::OsStr::new(text("path")?))?;
                anyhow::ensure!(path.is_dir(), "Expected a folder");
                self.navigate_to(path);
            }
            "back" => self.go_back(),
            "forward" => self.go_forward(),
            "up" => self.go_up(),
            "refresh" => self.load_entries(),
            "search" => {
                self.search_query = text("query")?.to_owned();
                self.apply_filter();
            }
            "view" => {
                self.view_mode = match text("mode")? {
                    "list" => ViewMode::List,
                    "grid" => ViewMode::Grid,
                    _ => bail!("View must be list or grid"),
                };
            }
            "toggle_hidden" => {
                self.show_hidden = !self.show_hidden;
                self.load_entries();
            }
            "select" | "open_folder" => {
                let name = text("name")?;
                let matches: Vec<_> = self
                    .entries
                    .iter()
                    .filter(|entry| entry.name == name)
                    .collect();
                anyhow::ensure!(matches.len() == 1, "Entry is absent or ambiguous: {name}");
                let path = matches[0].path.clone();
                if action == "open_folder" {
                    anyhow::ensure!(
                        matches[0].is_dir,
                        "Only folder activation is supported semantically"
                    );
                    self.navigate_to(path);
                } else {
                    self.select_single(&path);
                }
            }
            "new_folder" => self.dialog = DialogType::CreateFolder,
            "new_file" => self.dialog = DialogType::CreateFile,
            "dismiss_error" => self.error_message = None,
            "cancel" => {
                self.dialog = DialogType::None;
                self.rename_state = None;
                self.context_menu.hide();
            }
            "click" | "scroll" => {
                anyhow::ensure!(
                    command["snapshot_id"].is_u64(),
                    "Pixel input requires snapshot_id from fresh state/screenshot"
                );
                anyhow::ensure!(
                    !self.loading,
                    "Folder is loading; wait for ready before pixel input"
                );
                let scale = ctx.pixels_per_point();
                let x = command["x"].as_f64().context("Missing x")? as f32 / scale;
                let y = command["y"].as_f64().context("Missing y")? as f32 / scale;
                let position = egui::pos2(x, y);
                anyhow::ensure!(
                    x.is_finite() && y.is_finite() && ctx.screen_rect().contains(position),
                    "Coordinates are outside this app viewport"
                );
                input.events.push(egui::Event::PointerMoved(position));
                if action == "scroll" {
                    let dy = command["dy"].as_f64().context("Missing dy")? as f32;
                    anyhow::ensure!(
                        dy.is_finite() && dy.abs() <= 10000.0,
                        "Invalid scroll distance"
                    );
                    input
                        .events
                        .push(egui::Event::Scroll(egui::vec2(0.0, dy / scale)));
                } else {
                    let button = match command["button"].as_str().unwrap_or("left") {
                        "right" => egui::PointerButton::Secondary,
                        "left" => egui::PointerButton::Primary,
                        _ => bail!("Button must be left or right"),
                    };
                    for pressed in [true, false] {
                        input.events.push(egui::Event::PointerButton {
                            pos: position,
                            button,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        });
                    }
                }
            }
            "text" => {
                let value = text("text")?;
                anyhow::ensure!(value.len() <= 16384, "Text is too long");
                input.events.push(egui::Event::Text(value.into()));
            }
            "key" => {
                let mut modifiers = egui::Modifiers::default();
                let chord = text("key")?;
                let parts: Vec<_> = chord.split('+').collect();
                for part in &parts[..parts.len().saturating_sub(1)] {
                    match part.to_ascii_lowercase().as_str() {
                        "ctrl" => {
                            modifiers.ctrl = true;
                            modifiers.command = true;
                        }
                        "shift" => modifiers.shift = true,
                        "alt" => modifiers.alt = true,
                        _ => bail!("Unknown key modifier"),
                    }
                }
                let key = egui::Key::from_name(parts.last().copied().unwrap_or(""))
                    .context("Unknown key name")?;
                input.modifiers = modifiers;
                for pressed in [true, false] {
                    input.events.push(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers,
                    });
                }
            }
            _ => bail!("Unknown GUI action: {action}"),
        }
        ctx.request_repaint();
        Ok(())
    }
    pub(super) fn automation_finish(&mut self, ctx: &egui::Context) {
        let Some(mut automation) = self.automation.take() else {
            return;
        };
        automation.frame += 1;
        let base = self.describe_gui(ctx, 0, 200);
        let fingerprint = base.to_string();
        if fingerprint != automation.fingerprint {
            automation.epoch += 1;
            automation.fingerprint = fingerprint;
        }
        if let Some(request) = automation.pending.take() {
            let mut state = base;
            let offset = request.command["offset"].as_u64().unwrap_or(0) as usize;
            let limit = request.command["limit"].as_u64().unwrap_or(200) as usize;
            if offset != 0 || limit != 200 {
                state = self.describe_gui(ctx, offset, limit);
            }
            state["snapshot_id"] = json!(automation.epoch);
            state["frame"] = json!(automation.frame);
            if request.command["action"] == "screenshot" {
                if automation
                    .capture_started
                    .is_some_and(|started| started.elapsed() > Duration::from_secs(10))
                {
                    let _ = request.respond.send(Reply::error(
                        "Renderer screenshot timed out; restore the viewport and retry",
                    ));
                    automation.capture_state = None;
                    automation.capture_started = None;
                    automation.image = None;
                    self.automation = Some(automation);
                    return;
                }
                if automation.image.is_some() && automation.capture_state.is_some() {
                    let image = automation.image.take().expect("image checked");
                    automation.capture_started = None;
                    let captured_state = automation
                        .capture_state
                        .take()
                        .expect("capture state checked");
                    if image.pixels.len() > 16 * 1024 * 1024 {
                        let _ = request
                            .respond
                            .send(Reply::error("Screenshot exceeds the 16 megapixel limit"));
                        self.automation = Some(automation);
                        return;
                    }
                    let rgba: Vec<_> = image
                        .pixels
                        .iter()
                        .flat_map(|pixel| pixel.to_array())
                        .collect();
                    let reply = fyle::harness::encode_png(image.width() as u32, image.height() as u32, &rgba)
                        .map(|png| Reply { data:json!({"state":captured_state,"width":image.width(),"height":image.height(),"coordinate_units":"image_pixels"}), png }).unwrap_or_else(Reply::error);
                    let _ = request.respond.send(reply);
                } else {
                    if automation.capture_state.is_none() {
                        automation.image = None;
                        automation.capture_state = Some(state);
                        automation.capture_started = Some(Instant::now());
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                    }
                    automation.pending = Some(request);
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            } else {
                let _ = request.respond.send(Reply::ok(state));
            }
        }
        if let Some(deadline) = automation.close_after {
            if Instant::now() >= deadline {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
            }
        }
        self.automation = Some(automation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_or_ungrounded_pointer_input_is_rejected_before_injection() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = FileManager::new(dir.path().into());
        app.loading = false;
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        assert!(app
            .apply_automation(
                &ctx,
                &mut input,
                &json!({"action":"click","x":10,"y":10,"snapshot_id":3}),
                4
            )
            .is_err());
        assert!(app
            .apply_automation(
                &ctx,
                &mut input,
                &json!({"action":"click","x":10,"y":10}),
                4
            )
            .is_err());
        assert!(input.events.is_empty());
    }
    #[test]
    fn semantic_folder_activation_never_launches_a_file_application() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file.txt"), "fixture").unwrap();
        let mut app = FileManager::new(dir.path().into());
        app.entries =
            fyle::core::load_directory_entries(dir.path(), true, &HashSet::new()).unwrap();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        assert!(app
            .apply_automation(
                &ctx,
                &mut input,
                &json!({"action":"open_folder","name":"file.txt"}),
                1
            )
            .is_err());
        assert_eq!(app.current_path, dir.path());
    }
}
