//! One GL triangle behind the file viewport. All GL calls stay on the render thread.
use eframe::{
    egui,
    glow::{self, HasContext},
};
use std::sync::Arc;
pub struct HudShader {
    program: glow::Program,
    vao: glow::VertexArray,
}
impl HudShader {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        // SAFETY: eframe supplies the current context on the UI/render thread.
        unsafe {
            let program = gl.create_program()?;
            let mut shaders = Vec::new();
            let prefix = if gl.version().is_embedded {
                "#version 300 es\nprecision highp float;\n"
            } else {
                "#version 330 core\n"
            };
            for (kind, body) in [
                (glow::VERTEX_SHADER, include_str!("shaders/field.vert")),
                (glow::FRAGMENT_SHADER, include_str!("shaders/field.frag")),
            ] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, &format!("{prefix}{body}"));
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let error = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    for shader in shaders {
                        gl.delete_shader(shader);
                    }
                    gl.delete_program(program);
                    return Err(error);
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }
            gl.link_program(program);
            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }
            if !gl.get_program_link_status(program) {
                let error = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(error);
            }
            match gl.create_vertex_array() {
                Ok(vao) => Ok(Self { program, vao }),
                Err(error) => {
                    gl.delete_program(program);
                    Err(error)
                }
            }
        }
    }
    pub fn paint(
        self: &Arc<Self>,
        ui: &egui::Ui,
        rect: egui::Rect,
        time: f32,
        settings: &fyle::appearance::Appearance,
    ) {
        let shader = self.clone();
        let intensity = settings.intensity;
        let accent = settings.highlight;
        let size = rect.size() * ui.ctx().pixels_per_point();
        ui.painter().add(egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |_info, painter| {
                let gl = painter.gl();
                // SAFETY: egui_glow invokes this callback with its context current.
                unsafe {
                    gl.use_program(Some(shader.program));
                    gl.bind_vertex_array(Some(shader.vao));
                    gl.uniform_2_f32(
                        gl.get_uniform_location(shader.program, "resolution")
                            .as_ref(),
                        size.x,
                        size.y,
                    );
                    gl.uniform_1_f32(
                        gl.get_uniform_location(shader.program, "time").as_ref(),
                        time,
                    );
                    gl.uniform_1_f32(
                        gl.get_uniform_location(shader.program, "intensity")
                            .as_ref(),
                        intensity,
                    );
                    gl.uniform_3_f32(
                        gl.get_uniform_location(shader.program, "accent").as_ref(),
                        accent[0] as f32 / 255.0,
                        accent[1] as f32 / 255.0,
                        accent[2] as f32 / 255.0,
                    );
                    gl.draw_arrays(glow::TRIANGLES, 0, 3);
                    gl.bind_vertex_array(None);
                    gl.use_program(None);
                }
            })),
        });
    }
    pub fn destroy(&self, gl: &glow::Context) {
        // SAFETY: called once from eframe's shutdown hook with its context current.
        unsafe {
            gl.delete_vertex_array(self.vao);
            gl.delete_program(self.program);
        }
    }
}
