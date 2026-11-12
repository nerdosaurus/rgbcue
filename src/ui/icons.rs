//! Line icons and device illustrations drawn with egui's painter, so the port
//! does not depend on the web UI's SVG assets.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
};

use super::theme::{self, BORDER, BORDER_MID, DIM, MUTED, TEXT, TRACK};

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Dashboard,
    Keyboard,
    Mouse,
    Fan,
    Pump,
    Apps,
    Lcd,
    Macro,
    Cluster,
    Rgb,
    Profiles,
    Settings,
    Github,
    Plus,
    Chevron,
    Back,
    Check,
    Battery,
    Search,
    Menu,
    Grip,
}

fn p(r: Rect, x: f32, y: f32) -> Pos2 {
    egui::pos2(r.left() + r.width() * x, r.top() + r.height() * y)
}

pub fn icon(painter: &egui::Painter, rect: Rect, kind: Icon, color: Color32) {
    let sw = (rect.width() / 18.0 * 1.5).clamp(1.0, 2.0);
    let stroke = Stroke::new(sw, color);
    let r = rect.shrink(rect.width() * 0.08);

    match kind {
        Icon::Dashboard => {
            let gap = r.width() * 0.1;
            let w = (r.width() - gap) / 2.0;
            for (i, (x, y)) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
                .iter()
                .enumerate()
            {
                let _ = i;
                let cell = Rect::from_min_size(
                    egui::pos2(r.left() + x * (w + gap), r.top() + y * (w + gap)),
                    Vec2::splat(w),
                );
                painter.rect_stroke(cell, CornerRadius::same(2), stroke, StrokeKind::Inside);
            }
        }
        Icon::Keyboard => {
            let board = Rect::from_min_max(p(r, 0.02, 0.28), p(r, 0.98, 0.74));
            painter.rect_stroke(board, CornerRadius::same(3), stroke, StrokeKind::Inside);
            for row in 0..3 {
                let y = board.top() + board.height() * (0.2 + row as f32 * 0.28);
                painter.line_segment(
                    [
                        egui::pos2(board.left() + 2.0, y),
                        egui::pos2(board.right() - 2.0, y),
                    ],
                    stroke,
                );
            }
            let y = board.bottom() - board.height() * 0.14;
            painter.line_segment(
                [
                    egui::pos2(board.left() + board.width() * 0.3, y),
                    egui::pos2(board.right() - board.width() * 0.3, y),
                ],
                stroke,
            );
        }
        Icon::Mouse => {
            let body = Rect::from_min_max(p(r, 0.3, 0.02), p(r, 0.7, 0.98));
            painter.rect_stroke(body, CornerRadius::same(9), stroke, StrokeKind::Inside);
            painter.line_segment(
                [
                    egui::pos2(body.center().x, body.top()),
                    egui::pos2(body.center().x, body.top() + body.height() * 0.42),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(body.center().x, body.top() + body.height() * 0.2),
                    egui::pos2(body.center().x, body.top() + body.height() * 0.32),
                ],
                stroke,
            );
        }
        Icon::Fan | Icon::Pump => {
            let c = r.center();
            let rad = r.width() * 0.42;
            painter.circle_stroke(c, rad, stroke);
            painter.circle_stroke(c, rad * 0.22, stroke);
            for i in 0..3 {
                let a = std::f32::consts::TAU * i as f32 / 3.0 - 0.4;
                let from = c + Vec2::angled(a) * rad * 0.22;
                let to = c + Vec2::angled(a) * rad * 0.92;
                painter.line_segment([from, to], stroke);
            }
        }
        Icon::Apps => {
            for y in 0..3 {
                for x in 0..3 {
                    painter.circle_filled(
                        egui::pos2(
                            r.left() + r.width() * (0.2 + x as f32 * 0.3),
                            r.top() + r.height() * (0.2 + y as f32 * 0.3),
                        ),
                        sw * 0.9,
                        color,
                    );
                }
            }
        }
        Icon::Lcd => {
            let screen = Rect::from_min_max(p(r, 0.06, 0.14), p(r, 0.94, 0.68));
            painter.rect_stroke(screen, CornerRadius::same(2), stroke, StrokeKind::Inside);
            painter.line_segment([p(r, 0.36, 0.9), p(r, 0.64, 0.9)], stroke);
            painter.line_segment(
                [egui::pos2(r.center().x, screen.bottom()), p(r, 0.5, 0.9)],
                stroke,
            );
        }
        Icon::Macro => {
            let box_ = Rect::from_min_max(p(r, 0.06, 0.16), p(r, 0.94, 0.84));
            painter.rect_stroke(box_, CornerRadius::same(3), stroke, StrokeKind::Inside);
            let tri = vec![p(r, 0.4, 0.32), p(r, 0.4, 0.68), p(r, 0.68, 0.5)];
            painter.add(Shape::convex_polygon(tri, color, Stroke::NONE));
        }
        Icon::Cluster => {
            let c = r.center();
            painter.circle_stroke(c, r.width() * 0.18, stroke);
            for i in 0..6 {
                let a = std::f32::consts::TAU * i as f32 / 6.0;
                painter.circle_filled(c + Vec2::angled(a) * r.width() * 0.38, sw * 1.1, color);
            }
        }
        Icon::Rgb => {
            let c = r.center();
            let rad = r.width() * 0.28;
            for i in 0..3 {
                let a = std::f32::consts::TAU * i as f32 / 3.0 - std::f32::consts::FRAC_PI_2;
                painter.circle_stroke(c + Vec2::angled(a) * rad * 0.55, rad, stroke);
            }
        }
        Icon::Profiles => {
            for i in 0..3 {
                let y = r.top() + r.height() * (0.2 + i as f32 * 0.3);
                painter.line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], stroke);
                let x = r.left() + r.width() * (0.25 + i as f32 * 0.22);
                painter.circle_filled(egui::pos2(x, y), sw * 1.6, color);
            }
        }
        Icon::Settings => {
            let c = r.center();
            painter.circle_stroke(c, r.width() * 0.2, stroke);
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                painter.line_segment(
                    [
                        c + Vec2::angled(a) * r.width() * 0.34,
                        c + Vec2::angled(a) * r.width() * 0.45,
                    ],
                    stroke,
                );
            }
        }
        Icon::Github => {
            let c = r.center();
            painter.circle_stroke(c, r.width() * 0.36, stroke);
            painter.circle_filled(
                c + Vec2::new(-r.width() * 0.12, -r.height() * 0.04),
                sw * 0.8,
                color,
            );
            painter.circle_filled(
                c + Vec2::new(r.width() * 0.12, -r.height() * 0.04),
                sw * 0.8,
                color,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-r.width() * 0.16, r.height() * 0.2),
                    c + Vec2::new(r.width() * 0.16, r.height() * 0.2),
                ],
                stroke,
            );
        }
        Icon::Plus => {
            painter.line_segment([p(r, 0.5, 0.15), p(r, 0.5, 0.85)], stroke);
            painter.line_segment([p(r, 0.15, 0.5), p(r, 0.85, 0.5)], stroke);
        }
        Icon::Chevron => {
            painter.line_segment([p(r, 0.3, 0.4), p(r, 0.5, 0.6)], stroke);
            painter.line_segment([p(r, 0.5, 0.6), p(r, 0.7, 0.4)], stroke);
        }
        Icon::Back => {
            painter.line_segment([p(r, 0.75, 0.5), p(r, 0.25, 0.5)], stroke);
            painter.line_segment([p(r, 0.25, 0.5), p(r, 0.45, 0.3)], stroke);
            painter.line_segment([p(r, 0.25, 0.5), p(r, 0.45, 0.7)], stroke);
        }
        Icon::Check => {
            painter.line_segment([p(r, 0.18, 0.52), p(r, 0.42, 0.76)], stroke);
            painter.line_segment([p(r, 0.42, 0.76), p(r, 0.84, 0.26)], stroke);
        }
        Icon::Battery => {
            let body = Rect::from_min_max(p(r, 0.02, 0.28), p(r, 0.86, 0.72));
            painter.rect_stroke(body, CornerRadius::same(2), stroke, StrokeKind::Inside);
            painter.rect_filled(
                Rect::from_min_max(p(r, 0.86, 0.42), p(r, 0.98, 0.58)),
                1,
                color,
            );
            painter.rect_filled(
                Rect::from_min_max(p(r, 0.1, 0.36), p(r, 0.6, 0.64)),
                1,
                color,
            );
        }
        Icon::Search => {
            painter.circle_stroke(p(r, 0.42, 0.42), r.width() * 0.28, stroke);
            painter.line_segment([p(r, 0.63, 0.63), p(r, 0.88, 0.88)], stroke);
        }
        Icon::Menu => {
            for i in 0..3 {
                let y = r.top() + r.height() * (0.24 + i as f32 * 0.26);
                painter.line_segment(
                    [
                        egui::pos2(r.left() + 1.0, y),
                        egui::pos2(r.right() - 1.0, y),
                    ],
                    stroke,
                );
            }
        }
        Icon::Grip => {
            for row in 0..3 {
                for col in 0..2 {
                    painter.circle_filled(
                        egui::pos2(
                            r.left() + r.width() * (0.36 + col as f32 * 0.28),
                            r.top() + r.height() * (0.22 + row as f32 * 0.28),
                        ),
                        sw * 0.85,
                        color,
                    );
                }
            }
        }
    }
}

// ---- device illustrations --------------------------------------------------

pub fn keyboard(
    painter: &egui::Painter,
    rect: Rect,
    color: Color32,
    brightness: f32,
    rainbow: bool,
) {
    painter.rect_filled(
        rect,
        CornerRadius::same(12),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 10),
    );
    let board = rect.shrink2(Vec2::new(rect.width() * 0.04, rect.height() * 0.1));
    painter.rect_filled(
        board,
        CornerRadius::same(9),
        Color32::from_rgb(0x0f, 0x0f, 0x10),
    );
    painter.rect_stroke(
        board,
        CornerRadius::same(9),
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );

    let inner = board.shrink2(Vec2::new(9.0, 10.0));
    let gap = 3.0;
    let rows: [usize; 5] = [14, 13, 12, 11, 8];
    let row_h = (inner.height() - gap * 4.0) / 5.0;

    let mut index = 0usize;
    for (row, count) in rows.iter().enumerate() {
        let y = inner.top() + row as f32 * (row_h + gap);
        let usable = if row == 4 {
            inner.width() * 0.62
        } else {
            inner.width()
        };
        let key_w = (usable - gap * (*count as f32 - 1.0)) / *count as f32;
        let start_x = if row == 4 {
            inner.left() + (inner.width() - usable) / 2.0
        } else {
            inner.left()
        };
        for col in 0..*count {
            let x = start_x + col as f32 * (key_w + gap);
            let key = Rect::from_min_size(
                egui::pos2(x, y),
                Vec2::new(
                    if row == 4 && col == 3 {
                        key_w * 4.0 + gap * 3.0
                    } else {
                        key_w
                    },
                    row_h,
                ),
            );
            let c = if rainbow {
                theme::hsv(index as f32 * 0.022, 0.55, 1.0)
            } else {
                color
            };
            let alpha = (40.0 + 200.0 * brightness).clamp(0.0, 255.0) as u8;
            painter.rect_filled(
                key,
                CornerRadius::same(2),
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha),
            );
            index += 1;
        }
    }
}

pub fn mouse(painter: &egui::Painter, rect: Rect, color: Color32, brightness: f32, rainbow: bool) {
    painter.circle_filled(
        rect.center(),
        rect.height().min(rect.width()) * 0.5,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 10),
    );
    let body = Rect::from_center_size(
        rect.center(),
        Vec2::new(rect.width() * 0.46, rect.height() * 0.9),
    );
    painter.rect_filled(
        body,
        CornerRadius::same(26),
        Color32::from_rgb(0x0f, 0x0f, 0x10),
    );
    painter.rect_stroke(
        body,
        CornerRadius::same(26),
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    painter.line_segment(
        [
            egui::pos2(body.center().x, body.top() + 6.0),
            egui::pos2(body.center().x, body.top() + body.height() * 0.45),
        ],
        Stroke::new(1.0, BORDER_MID),
    );
    let wheel = Rect::from_center_size(
        egui::pos2(body.center().x, body.top() + body.height() * 0.22),
        Vec2::new(7.0, 18.0),
    );
    painter.rect_filled(
        wheel,
        CornerRadius::same(3),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 200),
    );

    let logo_center = egui::pos2(body.center().x, body.top() + body.height() * 0.62);
    let c = if rainbow {
        theme::hsv(0.55, 0.6, 1.0)
    } else {
        color
    };
    let alpha = (30.0 + 190.0 * brightness).clamp(0.0, 255.0) as u8;
    let lit = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha);
    painter.circle_filled(
        logo_center,
        16.0,
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 18),
    );
    let d = 10.0;
    painter.add(Shape::convex_polygon(
        vec![
            logo_center + Vec2::new(0.0, -d),
            logo_center + Vec2::new(d * 0.8, 0.0),
            logo_center + Vec2::new(0.0, d),
            logo_center + Vec2::new(-d * 0.8, 0.0),
        ],
        lit,
        Stroke::NONE,
    ));
}

#[allow(dead_code)]
pub fn fan(painter: &egui::Painter, rect: Rect, value: f32, max: f32, color: Color32) {
    let c = rect.center();
    let rad = rect.width().min(rect.height()) * 0.38;
    painter.circle_stroke(c, rad, Stroke::new(5.0, TRACK));
    let t = (value / max).clamp(0.0, 1.0);
    if t > 0.0 {
        theme::arc(
            painter,
            c,
            rad,
            -std::f32::consts::FRAC_PI_2,
            -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * t,
            Stroke::new(5.0, color),
        );
    }
    for i in 0..4 {
        let a = std::f32::consts::TAU * i as f32 / 4.0;
        let mut pts = Vec::new();
        for s in 0..10 {
            let tt = s as f32 / 9.0;
            let ang = a + tt * 0.55;
            pts.push(c + Vec2::angled(ang) * (rad * (0.2 + 0.62 * tt)));
        }
        painter.add(Shape::line(
            pts,
            Stroke::new(3.0, Color32::from_rgb(0x2e, 0x2e, 0x30)),
        ));
    }
    painter.circle_filled(c, rad * 0.2, Color32::from_rgb(0x1e, 0x1e, 0x1f));
    painter.circle_stroke(c, rad * 0.2, Stroke::new(1.0, BORDER_MID));
    painter.text(
        c + Vec2::new(0.0, rad + 16.0),
        Align2::CENTER_CENTER,
        format!("{}", value.round() as i32),
        FontId::monospace(12.0),
        TEXT,
    );
    painter.text(
        c + Vec2::new(0.0, rad + 30.0),
        Align2::CENTER_CENTER,
        "RPM",
        FontId::proportional(8.0),
        MUTED,
    );
}

pub fn pump(painter: &egui::Painter, rect: Rect, temp: f32, max: f32, color: Color32) {
    let c = rect.center();
    let rad = rect.width().min(rect.height()) * 0.36;
    painter.circle_stroke(c, rad, Stroke::new(6.0, TRACK));
    let t = (temp / max).clamp(0.0, 1.0);
    theme::arc(
        painter,
        c,
        rad,
        -std::f32::consts::FRAC_PI_2,
        -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * t,
        Stroke::new(6.0, color),
    );
    painter.circle_stroke(c, rad * 0.7, Stroke::new(1.0, BORDER));
    painter.text(
        c + Vec2::new(0.0, -4.0),
        Align2::CENTER_CENTER,
        format!("{}°", temp.round() as i32),
        FontId::proportional(24.0),
        TEXT,
    );
    painter.text(
        c + Vec2::new(0.0, 16.0),
        Align2::CENTER_CENTER,
        "COOLANT",
        FontId::proportional(7.5),
        MUTED,
    );
}

pub fn commander(painter: &egui::Painter, rect: Rect, color: Color32) {
    let box_ = rect.shrink2(Vec2::new(rect.width() * 0.06, rect.height() * 0.16));
    painter.rect_filled(
        box_,
        CornerRadius::same(8),
        Color32::from_rgb(0x0f, 0x0f, 0x10),
    );
    painter.rect_stroke(
        box_,
        CornerRadius::same(8),
        Stroke::new(1.0, BORDER),
        StrokeKind::Inside,
    );
    let r = box_.height() * 0.22;
    for i in 0..3 {
        let c = egui::pos2(
            box_.left() + box_.width() * (0.24 + i as f32 * 0.26),
            box_.center().y,
        );
        painter.circle_stroke(c, r, Stroke::new(1.5, Color32::from_rgb(0x2e, 0x2e, 0x30)));
        painter.circle_stroke(c, r * 0.35, Stroke::new(1.0, DIM));
        theme::arc(
            painter,
            c,
            r + 3.0,
            -std::f32::consts::FRAC_PI_2,
            -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * (0.4 + i as f32 * 0.2),
            Stroke::new(2.0, color),
        );
    }
}

pub fn battery(painter: &egui::Painter, rect: Rect, pct: u8) {
    let margin = rect.height() * 0.16;
    let body = Rect::from_min_max(
        egui::pos2(rect.left(), rect.top() + margin),
        egui::pos2(rect.right() - 4.0, rect.bottom() - margin),
    );
    painter.rect_stroke(
        body,
        CornerRadius::same(2),
        Stroke::new(1.0, MUTED),
        StrokeKind::Inside,
    );
    let terminal = Rect::from_center_size(
        egui::pos2(body.right() + 2.0, body.center().y),
        Vec2::new(3.0, rect.height() * 0.36),
    );
    painter.rect_filled(terminal, 1, MUTED);

    let fill = (pct as f32 / 100.0).clamp(0.0, 1.0);
    let inner = body.shrink(2.0);
    painter.rect_filled(
        Rect::from_min_max(
            inner.min,
            egui::pos2(inner.left() + inner.width() * fill, inner.bottom()),
        ),
        1,
        if pct > 30 { theme::GOOD } else { theme::WARN },
    );
}
