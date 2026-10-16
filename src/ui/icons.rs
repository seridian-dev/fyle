//! Font-independent vector icons on a shared 24-unit grid.
use crate::types::FileEntry;
use eframe::egui::{self, Color32, Painter, Rect, Stroke, Ui};
#[derive(Clone, Copy)]
pub enum Icon {
    Folder,
    File,
    Home,
    Desktop,
    Documents,
    Downloads,
    Image,
    Music,
    Video,
    Code,
    Terminal,
    Back,
    Forward,
    Up,
    Grid,
    List,
    Bookmark,
    Drive,
    Plus,
    Peek,
    Open,
    Copy,
    Cut,
    Rename,
    Delete,
    Properties,
}
pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let rect = rect.shrink(rect.width() * 0.08);
    let point = |x: f32, y: f32| {
        egui::pos2(
            rect.left() + x / 24.0 * rect.width(),
            rect.top() + y / 24.0 * rect.height(),
        )
    };
    let stroke = Stroke::new((rect.width() / 15.0).clamp(1.3, 2.0), color);
    let line = |coords: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            coords.iter().map(|&(x, y)| point(x, y)).collect(),
            stroke,
        ));
    };
    let box_at = |x, y, w, h| {
        p.rect_stroke(
            Rect::from_min_max(point(x, y), point(x + w, y + h)),
            2.0,
            stroke,
        );
    };
    match icon {
        Icon::Folder => {
            line(&[
                (2., 7.),
                (2., 4.),
                (9., 4.),
                (12., 7.),
                (22., 7.),
                (22., 20.),
                (2., 20.),
                (2., 7.),
                (22., 7.),
            ]);
        }
        Icon::File | Icon::Documents => {
            line(&[
                (5., 2.),
                (14., 2.),
                (20., 8.),
                (20., 22.),
                (5., 22.),
                (5., 2.),
            ]);
            line(&[(14., 2.), (14., 8.), (20., 8.)]);
            line(&[(8., 13.), (16., 13.)]);
            line(&[(8., 17.), (16., 17.)]);
        }
        Icon::Home => {
            line(&[(2., 11.), (12., 3.), (22., 11.)]);
            line(&[
                (5., 9.),
                (5., 21.),
                (10., 21.),
                (10., 15.),
                (14., 15.),
                (14., 21.),
                (19., 21.),
                (19., 9.),
            ]);
        }
        Icon::Desktop => {
            box_at(2., 3., 20., 14.);
            line(&[(12., 17.), (12., 21.)]);
            line(&[(7., 21.), (17., 21.)]);
        }
        Icon::Downloads => {
            line(&[(12., 2.), (12., 15.)]);
            line(&[(7., 10.), (12., 15.), (17., 10.)]);
            line(&[(3., 15.), (3., 21.), (21., 21.), (21., 15.)]);
        }
        Icon::Image => {
            box_at(2., 3., 20., 18.);
            line(&[(3., 18.), (8., 12.), (12., 16.), (17., 10.), (22., 15.)]);
            p.circle_stroke(point(8., 8.), rect.width() / 12., stroke);
        }
        Icon::Music => {
            line(&[(9., 18.), (9., 5.), (20., 3.), (20., 16.)]);
            line(&[(9., 9.), (20., 7.)]);
            p.circle_stroke(point(6., 18.), rect.width() / 8., stroke);
            p.circle_stroke(point(17., 16.), rect.width() / 8., stroke);
        }
        Icon::Video => {
            box_at(2., 5., 14., 14.);
            line(&[(16., 10.), (22., 6.), (22., 18.), (16., 14.)]);
        }
        Icon::Code => {
            line(&[(8., 6.), (2., 12.), (8., 18.)]);
            line(&[(16., 6.), (22., 12.), (16., 18.)]);
            line(&[(14., 3.), (10., 21.)]);
        }
        Icon::Terminal => {
            box_at(2., 3., 20., 18.);
            line(&[(6., 8.), (10., 12.), (6., 16.)]);
            line(&[(13., 16.), (18., 16.)]);
        }
        Icon::Back => line(&[(15., 5.), (8., 12.), (15., 19.)]),
        Icon::Forward => line(&[(9., 5.), (16., 12.), (9., 19.)]),
        Icon::Up => {
            line(&[(5., 10.), (12., 3.), (19., 10.)]);
            line(&[(12., 3.), (12., 21.)]);
        }
        Icon::Grid => {
            for x in [3., 14.] {
                for y in [3., 14.] {
                    box_at(x, y, 7., 7.);
                }
            }
        }
        Icon::List => {
            for y in [5., 12., 19.] {
                line(&[(8., y), (22., y)]);
                line(&[(2., y), (3., y)]);
            }
        }
        Icon::Bookmark => line(&[
            (6., 3.),
            (18., 3.),
            (18., 22.),
            (12., 17.),
            (6., 22.),
            (6., 3.),
        ]),
        Icon::Drive => {
            box_at(2., 5., 20., 14.);
            line(&[(3., 14.), (21., 14.)]);
            p.circle_filled(point(17., 17.), 1., color);
        }
        Icon::Plus => {
            line(&[(12., 4.), (12., 20.)]);
            line(&[(4., 12.), (20., 12.)]);
        }
        Icon::Peek => {
            line(&[
                (1., 12.),
                (6., 6.),
                (12., 4.),
                (18., 6.),
                (23., 12.),
                (18., 18.),
                (12., 20.),
                (6., 18.),
                (1., 12.),
            ]);
            p.circle_stroke(point(12., 12.), rect.width() / 6., stroke);
        }
        Icon::Open => {
            box_at(3., 3., 18., 18.);
            line(&[(8., 16.), (17., 7.), (11., 7.), (17., 7.), (17., 13.)]);
        }
        Icon::Copy => {
            box_at(7., 7., 14., 15.);
            line(&[(4., 16.), (3., 16.), (3., 3.), (16., 3.), (16., 4.)]);
        }
        Icon::Cut => {
            p.circle_stroke(point(6., 18.), rect.width() / 8., stroke);
            p.circle_stroke(point(6., 6.), rect.width() / 8., stroke);
            line(&[(8., 8.), (20., 20.)]);
            line(&[(8., 16.), (20., 4.)]);
        }
        Icon::Rename => {
            line(&[
                (4., 20.),
                (8., 19.),
                (20., 7.),
                (17., 4.),
                (5., 16.),
                (4., 20.),
            ]);
            line(&[(15., 6.), (18., 9.)]);
        }
        Icon::Delete => {
            line(&[(4., 7.), (20., 7.)]);
            line(&[(9., 4.), (15., 4.)]);
            line(&[(6., 7.), (7., 21.), (17., 21.), (18., 7.)]);
            line(&[(10., 10.), (10., 18.)]);
            line(&[(14., 10.), (14., 18.)]);
        }
        Icon::Properties => {
            p.circle_stroke(point(12., 12.), rect.width() * 0.42, stroke);
            p.circle_filled(point(12., 7.), rect.width() * 0.055, color);
            line(&[(12., 11.), (12., 17.)]);
        }
    }
}
pub fn show(ui: &mut Ui, icon: Icon, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), rect, icon, color);
    }
}
pub fn file_icon(entry: &FileEntry) -> (Icon, Color32) {
    use crate::theme::colors;
    if entry.is_dir {
        return (Icon::Folder, colors::AMBER);
    }
    match entry.file_type.as_str() {
        "RS" | "PY" | "JS" | "TS" | "TSX" | "JSX" | "GO" | "C" | "CPP" | "H" | "SH" => {
            (Icon::Code, colors::ELECTRIC_BLUE)
        }
        "JSON" | "TOML" | "YAML" | "YML" | "INI" => (Icon::Code, colors::MAGENTA),
        "MD" | "TXT" | "RST" => (Icon::Documents, colors::CYBER_GREEN),
        "PNG" | "JPG" | "JPEG" | "SVG" | "WEBP" | "GIF" => (Icon::Image, colors::MAGENTA),
        "MP3" | "FLAC" | "WAV" | "OGG" => (Icon::Music, colors::CYBER_GREEN),
        "MP4" | "MKV" | "WEBM" | "MOV" => (Icon::Video, colors::MAGENTA),
        _ => (Icon::File, colors::MUTED),
    }
}
