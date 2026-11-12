//! Visual language for the ported OpenLinkHub web UI.
//!
//! Colours, spacing and control shapes mirror `static/css/themes/default.css`
//! in OpenLinkHub: near-black gradient background, 14px cards with a hairline
//! border, muted uppercase labels, a thin accent-green slider and a warm gold
//! selection colour.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2,
};

// ---- palette ---------------------------------------------------------------

pub const BG_TOP: Color32 = Color32::from_rgb(0x0b, 0x0b, 0x0c);
pub const BG_BOT: Color32 = Color32::from_rgb(0x14, 0x14, 0x16);
pub const SIDEBAR_TOP: Color32 = Color32::from_rgb(0x0f, 0x0f, 0x10);
pub const SIDEBAR_BOT: Color32 = Color32::from_rgb(0x12, 0x12, 0x13);
pub const CARD: Color32 = Color32::from_rgb(0x17, 0x17, 0x18);
pub const FIELD: Color32 = Color32::from_rgb(0x20, 0x20, 0x21);
pub const TRACK: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2b);
pub const BORDER: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 16);
pub const BORDER_MID: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 26);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xe6);
pub const MUTED: Color32 = Color32::from_rgb(0xa1, 0xa1, 0xa1);
pub const DIM: Color32 = Color32::from_rgb(0x6f, 0x6f, 0x72);
pub const GOLD: Color32 = Color32::from_rgb(0xbf, 0x8d, 0x19);
pub const GOLD_TEXT: Color32 = Color32::from_rgb(0xf5, 0xd9, 0x8a);
pub const GOOD: Color32 = Color32::from_rgb(0x6e, 0xe7, 0xb7);
pub const WARN: Color32 = Color32::from_rgb(0xfa, 0xcc, 0x15);
pub const BAD: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
pub const ACCENT: Color32 = Color32::from_rgb(0xa9, 0xff, 0xad);
pub const WHITE: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);

pub const RADIUS_CARD: u8 = 14;
pub const RADIUS_CTRL: u8 = 8;

// ---- painting helpers ------------------------------------------------------

/// Vertical two-stop gradient, painted as a single mesh.
pub fn gradient(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(egui::Shape::mesh(mesh));
}

pub fn hairline(painter: &egui::Painter, y: f32, x0: f32, x1: f32, color: Color32) {
    painter.line_segment(
        [egui::pos2(x0, y), egui::pos2(x1, y)],
        Stroke::new(1.0, color),
    );
}

/// Arc from `start` to `end` radians, sampled into a polyline.
pub fn arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    start: f32,
    end: f32,
    stroke: Stroke,
) {
    let steps = 48;
    let mut points = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = start + (end - start) * (i as f32 / steps as f32);
        points.push(center + Vec2::angled(t) * radius);
    }
    painter.add(egui::Shape::line(points, stroke));
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

pub fn hsv(h: f32, s: f32, v: f32) -> Color32 {
    let h = (h.rem_euclid(1.0)) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

// ---- containers ------------------------------------------------------------

pub fn card() -> egui::Frame {
    egui::Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(RADIUS_CARD)
}

pub fn card_padded(pad: i8) -> egui::Frame {
    card().inner_margin(egui::Margin::same(pad))
}

/// Fixed-size card that reacts to hover and clicks (used for dashboard tiles).
pub fn card_at(ui: &mut Ui, size: Vec2, add: impl FnOnce(&mut Ui)) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let fill = if resp.hovered() {
        mix(CARD, WHITE, 0.035)
    } else {
        CARD
    };
    painter.rect_filled(rect, CornerRadius::same(RADIUS_CARD), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(RADIUS_CARD),
        Stroke::new(1.0, if resp.hovered() { BORDER_MID } else { BORDER }),
        StrokeKind::Inside,
    );

    let inner = rect.shrink2(Vec2::new(14.0, 12.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        add,
    );
    // The content often paints without allocating (custom-painted cards), so
    // restore the cursor to the card's right edge for the next sibling.
    ui.advance_cursor_after_rect(rect);
    resp
}

pub fn divider(ui: &mut Ui) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, BORDER);
}

pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.0)
            .color(DIM)
            .extra_letter_spacing(1.6),
    );
}

// ---- navigation ------------------------------------------------------------

pub fn nav_item(
    ui: &mut Ui,
    icon: crate::ui::icons::Icon,
    label: &str,
    active: bool,
    collapsed: bool,
) -> bool {
    let h = 34.0;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let painter = ui.painter();

    if active {
        painter.rect_filled(
            rect,
            CornerRadius::same(10),
            Color32::from_rgba_unmultiplied(255, 255, 255, 26),
        );
        painter.rect_stroke(
            rect,
            CornerRadius::same(10),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 20)),
            StrokeKind::Inside,
        );
    } else if resp.hovered() {
        painter.rect_filled(
            rect,
            CornerRadius::same(10),
            Color32::from_rgba_unmultiplied(255, 255, 255, 13),
        );
    }

    let fg = if active {
        WHITE
    } else if resp.hovered() {
        TEXT
    } else {
        MUTED
    };

    let icon_rect = if collapsed {
        Rect::from_center_size(rect.center(), Vec2::splat(18.0))
    } else {
        Rect::from_min_size(
            egui::pos2(rect.left() + 12.0, rect.center().y - 9.0),
            Vec2::splat(18.0),
        )
    };
    crate::ui::icons::icon(painter, icon_rect, icon, fg);

    if !collapsed {
        painter.text(
            egui::pos2(rect.left() + 40.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            fg,
        );
    }

    let resp = if collapsed && resp.hovered() {
        resp.on_hover_text(label)
    } else {
        resp
    };

    resp.clicked()
}

// ---- buttons and controls --------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    Normal,
    Ghost,
    Danger,
    Success,
}

pub fn button(ui: &mut Ui, label: &str, kind: Btn) -> bool {
    let font = FontId::proportional(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), TEXT);
    let w = (galley.size().x + 28.0).max(64.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 30.0), Sense::click());
    let painter = ui.painter();

    let (fill, border, fg) = match kind {
        Btn::Normal => (
            Color32::from_rgb(0x23, 0x23, 0x24),
            BORDER,
            Color32::from_rgb(0xd2, 0xd2, 0xd2),
        ),
        Btn::Ghost => (Color32::TRANSPARENT, Color32::TRANSPARENT, MUTED),
        Btn::Danger => (
            Color32::from_rgb(0x3a, 0x1d, 0x1d),
            Color32::from_rgba_unmultiplied(239, 68, 68, 102),
            Color32::from_rgb(0xfe, 0xca, 0xca),
        ),
        Btn::Success => (
            Color32::from_rgb(0x16, 0x3a, 0x28),
            Color32::from_rgba_unmultiplied(34, 197, 94, 115),
            Color32::from_rgb(0xbb, 0xf7, 0xd0),
        ),
    };

    if kind != Btn::Ghost {
        let hovered = resp.hovered();
        painter.rect_filled(
            rect,
            CornerRadius::same(10),
            if hovered {
                mix(fill, WHITE, 0.06)
            } else {
                fill
            },
        );
        painter.rect_stroke(
            rect,
            CornerRadius::same(10),
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
    } else if resp.hovered() {
        painter.rect_filled(
            rect,
            CornerRadius::same(10),
            Color32::from_rgba_unmultiplied(255, 255, 255, 13),
        );
    }

    painter.galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp.clicked()
}

pub fn chip(ui: &mut Ui, label: &str, selected: bool) -> bool {
    let font = FontId::proportional(12.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, MUTED);
    let w = galley.size().x + 22.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
    let painter = ui.painter();

    let fill = if selected {
        Color32::from_rgba_unmultiplied(191, 141, 25, 36)
    } else if resp.hovered() {
        Color32::from_rgba_unmultiplied(255, 255, 255, 16)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 9)
    };
    let border = if selected { GOLD } else { BORDER };
    let fg = if selected { GOLD_TEXT } else { MUTED };

    painter.rect_filled(rect, CornerRadius::same(RADIUS_CTRL), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(RADIUS_CTRL),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp.clicked()
}

pub fn switch(ui: &mut Ui, on: &mut bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());
    let painter = ui.painter();
    let mut changed = false;
    if resp.clicked() {
        *on = !*on;
        changed = true;
    }
    let track = Rect::from_center_size(rect.center(), Vec2::new(36.0, 20.0));
    painter.rect_filled(
        track,
        CornerRadius::same(10),
        if *on {
            Color32::from_rgb(0x2f, 0x6b, 0x52)
        } else {
            TRACK
        },
    );
    if *on {
        painter.rect_stroke(
            track,
            CornerRadius::same(10),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(110, 231, 183, 90)),
            StrokeKind::Inside,
        );
    }
    let cx = if *on {
        track.right() - 10.0
    } else {
        track.left() + 10.0
    };
    painter.circle_filled(
        egui::pos2(cx, track.center().y),
        7.0,
        if *on {
            GOOD
        } else {
            Color32::from_rgb(0x8a, 0x8a, 0x8d)
        },
    );
    changed
}

pub fn slider(ui: &mut Ui, value: &mut f32, min: f32, max: f32, suffix: &str) -> bool {
    slider_impl(ui, value, min, max, Some(suffix))
}

/// A slider without the numeric readout, for rows that show the value separately.
pub fn slider_plain(ui: &mut Ui, value: &mut f32, min: f32, max: f32) -> bool {
    slider_impl(ui, value, min, max, None)
}

fn slider_impl(
    ui: &mut Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    suffix: Option<&str>,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 28.0),
        Sense::click_and_drag(),
    );
    let painter = ui.painter();
    let value_w = if suffix.is_some() { 52.0 } else { 0.0 };
    let track = Rect::from_min_max(
        egui::pos2(rect.left(), rect.center().y - 2.0),
        egui::pos2(rect.right() - value_w, rect.center().y + 2.0),
    );

    let mut changed = false;
    if let Some(pos) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            let t = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
            let v = min + (max - min) * t;
            if (*value - v).abs() > 0.01 {
                *value = v;
                changed = true;
            }
        }
    }

    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    painter.rect_filled(track, CornerRadius::same(2), TRACK);
    if t > 0.0 {
        let filled = Rect::from_min_max(
            track.min,
            egui::pos2(track.left() + track.width() * t, track.bottom()),
        );
        painter.rect_filled(filled, CornerRadius::same(2), ACCENT);
    }
    painter.circle_filled(
        egui::pos2(track.left() + track.width() * t, track.center().y),
        6.0,
        WHITE,
    );
    if let Some(suffix) = suffix {
        painter.text(
            egui::pos2(rect.right(), rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{} {suffix}", value.round() as i32),
            FontId::monospace(11.5),
            TEXT,
        );
    }
    changed
}

pub fn select(ui: &mut Ui, options: &[&str], idx: &mut usize) -> bool {
    select_sized(ui, options, idx, 132.0)
}

pub fn select_sized(ui: &mut Ui, options: &[&str], idx: &mut usize, width: f32) -> bool {
    let mut changed = false;
    let selected = options.get(*idx).copied().unwrap_or("");
    let id = ui.next_auto_id();
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.weak_bg_fill = FIELD;
        widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.hovered.weak_bg_fill = mix(FIELD, WHITE, 0.05);
        widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_MID);
        widgets.active.weak_bg_fill = mix(FIELD, WHITE, 0.08);
        for state in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
        ] {
            state.corner_radius = CornerRadius::same(RADIUS_CTRL);
        }
        ui.spacing_mut().interact_size.y = 28.0;
        egui::ComboBox::from_id_salt(id)
            .selected_text(egui::RichText::new(selected).size(12.0).color(TEXT))
            .width(width)
            .show_ui(ui, |ui| {
                ui.visuals_mut().selection.bg_fill =
                    Color32::from_rgba_unmultiplied(191, 141, 25, 45);
                for (i, option) in options.iter().enumerate() {
                    if ui.selectable_value(idx, i, *option).changed() {
                        changed = true;
                    }
                }
            });
    });
    changed
}

pub fn select_strings(ui: &mut Ui, options: &[String], idx: &mut usize) -> bool {
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    select(ui, &refs, idx)
}

pub fn select_strings_sized(ui: &mut Ui, options: &[String], idx: &mut usize, width: f32) -> bool {
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    select_sized(ui, &refs, idx, width)
}

pub fn swatch(ui: &mut Ui, rgb: [u8; 3], selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        CornerRadius::same(7),
        Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(7),
        Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected { WHITE } else { BORDER_MID },
        ),
        StrokeKind::Inside,
    );
    resp.clicked()
}

pub fn pill(ui: &mut Ui, label: &str, color: Color32) {
    let font = FontId::proportional(11.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let w = galley.size().x + 26.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        CornerRadius::same(12),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 20),
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(12),
        Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 90),
        ),
        StrokeKind::Inside,
    );
    painter.circle_filled(egui::pos2(rect.left() + 11.0, rect.center().y), 3.0, color);
    painter.galley(
        egui::pos2(rect.left() + 19.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
}

/// Label on the left, value on the right, separated by a hairline above.
pub fn stat_row(ui: &mut Ui, label: &str, value: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
    let painter = ui.painter();
    hairline(painter, rect.top(), rect.left(), rect.right(), BORDER);
    painter.text(
        egui::pos2(rect.left() + 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        MUTED,
    );
    painter.text(
        egui::pos2(rect.right() - 2.0, rect.center().y),
        Align2::RIGHT_CENTER,
        value,
        FontId::monospace(11.5),
        TEXT,
    );
}

/// A settings row with a label and a control on the right.
pub fn settings_row(ui: &mut Ui, label: &str, control: impl FnOnce(&mut Ui) -> bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::hover());
    let painter = ui.painter();
    hairline(painter, rect.top(), rect.left(), rect.right(), BORDER);
    painter.text(
        egui::pos2(rect.left() + 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        MUTED,
    );

    let control_rect = Rect::from_min_max(
        egui::pos2(rect.right() - 160.0, rect.top() + 8.0),
        egui::pos2(rect.right(), rect.bottom() - 8.0),
    );
    let mut changed = false;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(control_rect)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
        |ui| {
            changed = control(ui);
        },
    );
    changed
}
