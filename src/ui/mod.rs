//! Ported OpenLinkHub web UI, rendered natively with egui.
//!
//! This is a visual shell only: it uses mock devices and local widget state, and
//! never talks to the OpenLinkHub API. Launch it with `rgbcue --preview`.

pub mod icons;
pub mod theme;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};
use icons::Icon;
use theme as t;

const PROFILES: &[&str] = &["Default", "Profile 1", "Profile 2", "Profile 3"];
const EFFECTS: &[&str] = &[
    "Static",
    "Rainbow Wave",
    "Breathing",
    "Gradient",
    "Ripple",
    "Spinner",
    "Rain",
    "Color Pulse",
    "Temperature",
];
const LAYOUTS: &[&str] = &["US", "UK", "DE", "Nordic"];
const POLLING: &[&str] = &["125 Hz", "250 Hz", "500 Hz", "1000 Hz"];
const THEMES: &[&str] = &["Default", "Midnight", "Nord"];
const LANGUAGES: &[&str] = &["English", "Deutsch", "Français", "Hrvatski"];
const SWATCHES: &[[u8; 3]] = &[
    [255, 255, 255],
    [255, 82, 82],
    [255, 150, 60],
    [250, 220, 80],
    [110, 231, 183],
    [80, 180, 255],
    [140, 120, 255],
    [255, 90, 200],
    [120, 120, 130],
    [10, 10, 10],
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Keyboard,
    Mouse,
    Commander,
    Pump,
}

struct Device {
    name: &'static str,
    kind: Kind,
    firmware: &'static str,
    serial: &'static str,
    connected: bool,
    battery: Option<u8>,
    profile: usize,
    brightness: f32,
    effect: usize,
    speed: f32,
    polling: usize,
    layout: usize,
    cluster: bool,
    stat: (&'static str, &'static str),
    #[allow(dead_code)]
    detail: [(&'static str, &'static str); 2],
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Dashboard,
    Device(usize),
    Apps,
    Lcd,
    Macros,
    Cluster,
    Rgb,
    Profiles,
    Settings,
}

struct Monitor {
    title: &'static str,
    value: &'static str,
    detail: &'static str,
    color: Color32,
}

#[derive(Clone, Copy, PartialEq)]
enum Group {
    Monitor,
    Device,
}

#[derive(Clone, Copy)]
struct DragItem {
    group: Group,
    pos: usize,
}

pub fn run(page: &str) -> eframe::Result<()> {
    let page = match page {
        "device" => Page::Device(0),
        "macros" => Page::Macros,
        "settings" => Page::Settings,
        _ => Page::Dashboard,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1120.0, 720.0])
            .with_app_id("rgbcue")
            .with_resizable(true)
            .with_fullscreen(false)
            .with_title("RGBCUE"),
        ..Default::default()
    };
    eframe::run_native(
        "RGBCUE",
        options,
        Box::new(move |cc| {
            configure(&cc.egui_ctx);
            Ok(Box::new(PreviewApp::new(page)))
        }),
    )
}

fn configure(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (egui::TextStyle::Body, FontId::proportional(13.0)),
            (egui::TextStyle::Button, FontId::proportional(13.0)),
            (egui::TextStyle::Small, FontId::proportional(11.0)),
            (egui::TextStyle::Heading, FontId::proportional(24.0)),
            (egui::TextStyle::Monospace, FontId::monospace(12.0)),
        ]
        .into();
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Color32::TRANSPARENT;
        style.visuals.window_fill = t::CARD;
        style.visuals.override_text_color = Some(t::TEXT);
    });
}

struct PreviewApp {
    page: Page,
    collapsed: bool,
    devices: Vec<Device>,
    monitors: Vec<Monitor>,
    monitor_order: Vec<usize>,
    device_order: Vec<usize>,
    color: [u8; 3],
    macro_sel: usize,
    theme_idx: usize,
    lang_idx: usize,
    celsius: bool,
    autostart: bool,
}

impl PreviewApp {
    fn new(page: Page) -> Self {
        Self {
            page,
            collapsed: false,
            color: [80, 180, 255],
            macro_sel: 0,
            theme_idx: 0,
            lang_idx: 0,
            celsius: true,
            autostart: true,
            monitor_order: (0..5).collect(),
            device_order: (0..5).collect(),
            monitors: vec![
                Monitor {
                    title: "CPU temp",
                    value: "41 °C",
                    detail: "Ryzen 9 7950X",
                    color: t::WARN,
                },
                Monitor {
                    title: "GPU temp",
                    value: "58 °C",
                    detail: "RTX 3090",
                    color: t::WARN,
                },
                Monitor {
                    title: "Coolant",
                    value: "32 °C",
                    detail: "H115i Platinum",
                    color: t::ACCENT,
                },
                Monitor {
                    title: "Fans",
                    value: "1 240 RPM",
                    detail: "Commander PRO",
                    color: t::TEXT,
                },
                Monitor {
                    title: "Storage",
                    value: "46 °C",
                    detail: "990 PRO 2 TB",
                    color: t::WARN,
                },
            ],
            devices: vec![
                Device {
                    name: "K65 RGB MINI",
                    kind: Kind::Keyboard,
                    firmware: "1.0.8",
                    serial: "K65-01",
                    connected: true,
                    battery: None,
                    profile: 0,
                    brightness: 80.0,
                    effect: 1,
                    speed: 60.0,
                    polling: 3,
                    layout: 0,
                    cluster: false,
                    stat: ("1000 Hz", "Polling rate"),
                    detail: [("Layout", "US"), ("Profile", "Default")],
                },
                Device {
                    name: "Scimitar RGB Elite",
                    kind: Kind::Mouse,
                    firmware: "3.12",
                    serial: "SCIM-01",
                    connected: true,
                    battery: None,
                    profile: 0,
                    brightness: 75.0,
                    effect: 0,
                    speed: 50.0,
                    polling: 3,
                    layout: 0,
                    cluster: false,
                    stat: ("18 000", "DPI"),
                    detail: [("Buttons", "17"), ("Profile", "Default")],
                },
                Device {
                    name: "Scimitar Elite Wireless SE",
                    kind: Kind::Mouse,
                    firmware: "2.1.4",
                    serial: "SCIM-SE",
                    connected: true,
                    battery: Some(84),
                    profile: 1,
                    brightness: 65.0,
                    effect: 7,
                    speed: 50.0,
                    polling: 3,
                    layout: 0,
                    cluster: false,
                    stat: ("1 600", "DPI"),
                    detail: [("Battery", "84%"), ("Profile", "FPS")],
                },
                Device {
                    name: "Commander PRO",
                    kind: Kind::Commander,
                    firmware: "0.9.2",
                    serial: "CPRO-1",
                    connected: true,
                    battery: None,
                    profile: 0,
                    brightness: 70.0,
                    effect: 0,
                    speed: 50.0,
                    polling: 0,
                    layout: 0,
                    cluster: false,
                    stat: ("1 240", "Fan RPM"),
                    detail: [("Fans", "3"), ("Sensors", "4")],
                },
                Device {
                    name: "H115i RGB Platinum",
                    kind: Kind::Pump,
                    firmware: "1.2.0",
                    serial: "H115I-1",
                    connected: true,
                    battery: None,
                    profile: 0,
                    brightness: 75.0,
                    effect: 0,
                    speed: 55.0,
                    polling: 0,
                    layout: 0,
                    cluster: false,
                    stat: ("32 °C", "Coolant"),
                    detail: [("Pump", "2 100 RPM"), ("Profile", "Balanced")],
                },
            ],
        }
    }

    fn subtitle(&self) -> String {
        match self.page {
            Page::Dashboard => format!(
                "{} monitors · {} devices · OpenLinkHub 0.9.5",
                self.monitors.len(),
                self.devices.len()
            ),
            Page::Device(i) => format!(
                "{} · firmware {}",
                self.devices[i].serial, self.devices[i].firmware
            ),
            Page::Macros => "4 profiles · 6 macros".into(),
            Page::Settings => "Dashboard and application preferences".into(),
            _ => "OpenLinkHub 0.9.5".into(),
        }
    }

    fn title(&self) -> String {
        match self.page {
            Page::Dashboard => "Dashboard".into(),
            Page::Device(i) => self.devices[i].name.into(),
            Page::Apps => "Applications".into(),
            Page::Lcd => "LCD".into(),
            Page::Macros => "Macros".into(),
            Page::Cluster => "RGB Cluster".into(),
            Page::Rgb => "RGB Editor".into(),
            Page::Profiles => "Temperature Profiles".into(),
            Page::Settings => "Settings".into(),
        }
    }

    // ---- sidebar -----------------------------------------------------------

    fn sidebar(&mut self, ui: &mut Ui) {
        let r = ui.max_rect();
        t::gradient(ui.painter(), r, t::SIDEBAR_TOP, t::SIDEBAR_BOT);
        ui.painter().line_segment(
            [r.right_top(), r.right_bottom()],
            Stroke::new(1.0, t::BORDER),
        );

        let pad_x = if self.collapsed { 12.0 } else { 16.0 };
        let inner = r.shrink2(Vec2::new(pad_x, 14.0));
        let bottom_h = if self.collapsed { 250.0 } else { 320.0 };
        let top_rect = Rect::from_min_max(
            inner.min,
            egui::pos2(inner.right(), (inner.bottom() - bottom_h).max(inner.top())),
        );
        let bottom_rect =
            Rect::from_min_max(egui::pos2(inner.left(), top_rect.bottom()), inner.max);

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(top_rect)
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                if self.collapsed {
                    if menu_button(ui) {
                        self.collapsed = false;
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("RGBCUE")
                                .size(15.0)
                                .strong()
                                .color(t::WHITE)
                                .extra_letter_spacing(3.0),
                        );
                        ui.add_space((ui.available_width() - 26.0).max(0.0));
                        if menu_button(ui) {
                            self.collapsed = true;
                        }
                    });
                }

                ui.add_space(18.0);
                if !self.collapsed {
                    t::section_label(ui, "Navigation");
                    ui.add_space(2.0);
                }
                if t::nav_item(
                    ui,
                    Icon::Dashboard,
                    "Dashboard",
                    self.page == Page::Dashboard,
                    self.collapsed,
                ) {
                    self.page = Page::Dashboard;
                }

                ui.add_space(16.0);
                if !self.collapsed {
                    t::section_label(ui, "Devices");
                    ui.add_space(2.0);
                }
                for i in 0..self.devices.len() {
                    let d = &self.devices[i];
                    let icon = match d.kind {
                        Kind::Keyboard => Icon::Keyboard,
                        Kind::Mouse => Icon::Mouse,
                        Kind::Commander => Icon::Fan,
                        Kind::Pump => Icon::Pump,
                    };
                    if t::nav_item(
                        ui,
                        icon,
                        d.name,
                        self.page == Page::Device(i),
                        self.collapsed,
                    ) {
                        self.page = Page::Device(i);
                    }
                }
            },
        );

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(bottom_rect)
                .layout(Layout::bottom_up(Align::Min)),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if !self.collapsed {
                    // bottom_up: the first item added sits at the bottom.
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Version").size(11.0).color(t::DIM));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new("0.9.5").monospace().size(10.5).color(t::TEXT));
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Build").size(11.0).color(t::DIM));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new("3048db0b")
                                    .monospace()
                                    .size(10.5)
                                    .color(t::TEXT),
                            );
                        });
                    });
                    ui.add_space(6.0);
                    t::divider(ui);
                    ui.add_space(4.0);
                }

                if t::nav_item(ui, Icon::Github, "GitHub", false, self.collapsed) {
                    // Preview only.
                }
                ui.add_space(4.0);
                t::divider(ui);
                ui.add_space(4.0);

                let items: [(Page, Icon, &str); 7] = [
                    (Page::Apps, Icon::Apps, "Applications"),
                    (Page::Lcd, Icon::Lcd, "LCD"),
                    (Page::Macros, Icon::Macro, "Macros"),
                    (Page::Cluster, Icon::Cluster, "RGB Cluster"),
                    (Page::Rgb, Icon::Rgb, "RGB Editor"),
                    (Page::Profiles, Icon::Profiles, "Profiles"),
                    (Page::Settings, Icon::Settings, "Settings"),
                ];
                for (page, icon, label) in items.into_iter().rev() {
                    if t::nav_item(ui, icon, label, self.page == page, self.collapsed) {
                        self.page = page;
                    }
                }
            },
        );
    }

    // ---- main panel --------------------------------------------------------

    fn main(&mut self, ui: &mut Ui) {
        let r = ui.max_rect();
        t::gradient(ui.painter(), r, t::BG_TOP, t::BG_BOT);

        let inner = r.shrink2(Vec2::new(28.0, 22.0));
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                self.header(ui);
                ui.add_space(10.0);
                t::divider(ui);
                ui.add_space(16.0);

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        Page::Dashboard => self.dashboard(ui),
                        Page::Device(i) => self.device_page(ui, i),
                        Page::Macros => self.macros_page(ui),
                        Page::Settings => self.settings_page(ui),
                        Page::Apps => self.empty_page(
                            ui,
                            Icon::Apps,
                            "Applications",
                            "Application profiles will live here once the API is wired up.",
                        ),
                        Page::Lcd => self.empty_page(
                            ui,
                            Icon::Lcd,
                            "LCD",
                            "LCD screens, widgets and uploads will be ported next.",
                        ),
                        Page::Cluster => self.empty_page(
                            ui,
                            Icon::Cluster,
                            "RGB Cluster",
                            "Cluster grouping and synchronisation controls go here.",
                        ),
                        Page::Rgb => self.empty_page(
                            ui,
                            Icon::Rgb,
                            "RGB Editor",
                            "Per-zone and per-LED editing will live here.",
                        ),
                        Page::Profiles => self.empty_page(
                            ui,
                            Icon::Profiles,
                            "Temperature Profiles",
                            "Fan and pump curves will be edited here.",
                        ),
                    });
            },
        );
    }

    fn header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if let Page::Device(_) = self.page {
                if t::button(ui, "Back", t::Btn::Ghost) {
                    self.page = Page::Dashboard;
                }
            }
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.title())
                        .size(24.0)
                        .strong()
                        .color(t::WHITE),
                );
                ui.add_space(2.0);
                ui.label(RichText::new(self.subtitle()).size(12.0).color(t::MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                t::pill(ui, "Connected", t::GOOD);
                ui.add_space(4.0);
                let _ = t::button(ui, "Refresh", t::Btn::Ghost);
            });
        });
    }

    // ---- dashboard ---------------------------------------------------------

    fn dashboard(&mut self, ui: &mut Ui) {
        let monitor_order = self.monitor_order.clone();
        let device_order = self.device_order.clone();
        let mut pending: Option<(Group, usize, usize)> = None;

        section_header(ui, "System", "Drag to reorder");
        ui.add_space(8.0);

        let mgap = 25.0;
        let avail = ui.available_width();
        let per_row = if avail > 980.0 {
            5
        } else if avail > 760.0 {
            4
        } else {
            3
        };
        let mw = (avail - mgap * (per_row - 1) as f32) / per_row as f32 * 0.5;
        for (row, chunk) in monitor_order.chunks(per_row).enumerate() {
            if row > 0 {
                ui.add_space(mgap);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = mgap;
                for (col, &mi) in chunk.iter().enumerate() {
                    let pos = row * per_row + col;
                    monitor_card(ui, &self.monitors[mi], mw, pos, &mut pending);
                }
                for _ in chunk.len()..per_row {
                    ui.allocate_space(Vec2::new(mw, 0.0));
                }
            });
        }

        ui.add_space(16.0);
        section_header(ui, "Devices", "Drag to reorder");
        ui.add_space(8.0);

        let gap = 29.0;
        let cols = 3usize;
        let dw = (avail - gap * (cols - 1) as f32) / cols as f32 * 0.85;

        let mut next_page = None;
        let total = device_order.len() + 1;
        let mut i = 0usize;
        let mut row = 0usize;
        while i < total {
            if row > 0 {
                ui.add_space(gap);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for _ in 0..cols {
                    if i < device_order.len() {
                        let di = device_order[i];
                        if device_card(ui, &self.devices[di], dw, i, &mut pending) {
                            next_page = Some(Page::Device(di));
                        }
                    } else if i == device_order.len() {
                        add_card(ui, dw);
                    } else {
                        ui.allocate_space(Vec2::new(dw, 0.0));
                    }
                    i += 1;
                }
            });
            row += 1;
        }

        if let Some((group, from, to)) = pending {
            let order = match group {
                Group::Monitor => &mut self.monitor_order,
                Group::Device => &mut self.device_order,
            };
            reorder(order, from, to);
        }
        if let Some(page) = next_page {
            self.page = page;
        }
    }

    // ---- device ------------------------------------------------------------

    fn device_page(&mut self, ui: &mut Ui, idx: usize) {
        let color = &mut self.color;
        let d = &mut self.devices[idx];

        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(300.0, 10.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(300.0);
                    t::card_padded(14).show(ui, |ui| {
                        ui.set_width(272.0);
                        t::section_label(ui, "Device");
                        ui.add_space(4.0);
                        ui.label(RichText::new(d.name).size(14.0).strong().color(t::WHITE));
                        ui.add_space(8.0);

                        t::stat_row(ui, "Firmware", d.firmware);
                        t::settings_row(ui, "Profile", |ui| {
                            t::select(ui, PROFILES, &mut d.profile)
                        });
                        t::settings_row(ui, "Brightness", |ui| {
                            t::slider(ui, &mut d.brightness, 0.0, 100.0, "%")
                        });
                        t::settings_row(ui, "RGB profile", |ui| {
                            t::select(ui, EFFECTS, &mut d.effect)
                        });
                        t::settings_row(ui, "Layout", |ui| t::select(ui, LAYOUTS, &mut d.layout));
                        t::settings_row(ui, "Polling rate", |ui| {
                            t::select(ui, POLLING, &mut d.polling)
                        });
                        t::settings_row(ui, "Performance", |ui| {
                            t::button(ui, "Configure", t::Btn::Normal)
                        });
                        t::settings_row(ui, "RGB Cluster", |ui| t::switch(ui, &mut d.cluster));

                        ui.add_space(14.0);
                        let _ = t::button(ui, "Remove from dashboard", t::Btn::Danger);
                    });
                },
            );

            ui.add_space(16.0);

            ui.vertical(|ui| {
                lighting_card(ui, d, color);
                ui.add_space(16.0);
                assignments_card(ui, d);
            });
        });
    }

    // ---- macros ------------------------------------------------------------

    fn macros_page(&mut self, ui: &mut Ui) {
        let macros: [(&str, usize, &str); 4] = [
            ("FPS Tap", 6, "Default"),
            ("Text Snippet", 3, "Default"),
            ("Media Control", 5, "Media"),
            ("DPI Toggle", 2, "FPS"),
        ];

        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(300.0, 10.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(300.0);
                    t::card_padded(14).show(ui, |ui| {
                        ui.set_width(300.0);
                        t::section_label(ui, "Macros");
                        ui.add_space(8.0);
                        for (i, (name, actions, profile)) in macros.iter().enumerate() {
                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 52.0),
                                Sense::click(),
                            );
                            if self.macro_sel == i {
                                ui.painter().rect_filled(
                                    rect,
                                    CornerRadius::same(10),
                                    Color32::from_rgba_unmultiplied(191, 141, 25, 26),
                                );
                                ui.painter().rect_stroke(
                                    rect,
                                    CornerRadius::same(10),
                                    Stroke::new(1.0, t::GOLD),
                                    StrokeKind::Inside,
                                );
                            } else if resp.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    CornerRadius::same(10),
                                    Color32::from_rgba_unmultiplied(255, 255, 255, 10),
                                );
                            }
                            ui.painter().text(
                                egui::pos2(rect.left() + 12.0, rect.top() + 16.0),
                                Align2::LEFT_CENTER,
                                *name,
                                FontId::proportional(13.0),
                                t::TEXT,
                            );
                            ui.painter().text(
                                egui::pos2(rect.left() + 12.0, rect.top() + 34.0),
                                Align2::LEFT_CENTER,
                                format!("{actions} actions · {profile}"),
                                FontId::proportional(10.5),
                                t::MUTED,
                            );
                            if resp.clicked() {
                                self.macro_sel = i;
                            }
                            ui.add_space(2.0);
                        }
                    });
                },
            );

            ui.add_space(16.0);

            ui.vertical(|ui| {
                let (name, actions, profile) = macros[self.macro_sel];
                t::card_padded(16).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            t::section_label(ui, "Macro");
                            ui.label(RichText::new(name).size(18.0).strong().color(t::WHITE));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            t::pill(ui, profile, t::MUTED);
                        });
                    });
                    ui.add_space(10.0);
                    t::divider(ui);
                    ui.add_space(8.0);

                    let rows: [(&str, &str, &str); 6] = [
                        ("Key", "W", "keyboard"),
                        ("Delay", "40 ms", "delay"),
                        ("Key", "A", "keyboard"),
                        ("Delay", "40 ms", "delay"),
                        ("Key", "S", "keyboard"),
                        ("Delay", "40 ms", "delay"),
                    ];
                    for (kind, value, _) in rows.iter().take(actions.min(rows.len())) {
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 34.0),
                            Sense::hover(),
                        );
                        t::hairline(
                            ui.painter(),
                            rect.top(),
                            rect.left(),
                            rect.right(),
                            t::BORDER,
                        );
                        let badge = Rect::from_min_size(
                            egui::pos2(rect.left() + 4.0, rect.center().y - 9.0),
                            Vec2::new(64.0, 18.0),
                        );
                        ui.painter().rect_filled(
                            badge,
                            CornerRadius::same(5),
                            Color32::from_rgba_unmultiplied(255, 255, 255, 12),
                        );
                        ui.painter().text(
                            badge.center(),
                            Align2::CENTER_CENTER,
                            *kind,
                            FontId::proportional(10.0),
                            t::MUTED,
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 84.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            *value,
                            FontId::monospace(11.5),
                            t::TEXT,
                        );
                    }

                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        let _ = t::button(ui, "Save", t::Btn::Success);
                        let _ = t::button(ui, "Duplicate", t::Btn::Normal);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let _ = t::button(ui, "Delete", t::Btn::Danger);
                        });
                    });
                });
            });
        });
    }

    // ---- settings ----------------------------------------------------------

    fn settings_page(&mut self, ui: &mut Ui) {
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width().min(760.0), 10.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(760.0_f32.min(ui.available_width()));
                t::card_padded(16).show(ui, |ui| {
                    t::section_label(ui, "Dashboard");
                    ui.add_space(8.0);
                    t::settings_row(ui, "Theme", |ui| t::select(ui, THEMES, &mut self.theme_idx));
                    t::settings_row(ui, "Language", |ui| {
                        t::select(ui, LANGUAGES, &mut self.lang_idx)
                    });
                    t::settings_row(ui, "Temperature unit", |ui| {
                        let changed = t::switch(ui, &mut self.celsius);
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(if self.celsius {
                                "Celsius"
                            } else {
                                "Fahrenheit"
                            })
                            .size(11.0)
                            .color(t::MUTED),
                        );
                        changed
                    });
                    t::settings_row(ui, "Start with system", |ui| {
                        t::switch(ui, &mut self.autostart)
                    });
                    t::settings_row(ui, "Collapse sidebar", |ui| {
                        t::switch(ui, &mut self.collapsed)
                    });
                });

                ui.add_space(16.0);
                t::card_padded(16).show(ui, |ui| {
                    t::section_label(ui, "About");
                    ui.add_space(8.0);
                    t::stat_row(ui, "Frontend", "RGBCUE 0.1.0 (egui)");
                    t::stat_row(ui, "Backend", "OpenLinkHub 0.9.5");
                    t::stat_row(ui, "Build", "3048db0b");
                });
            },
        );
    }

    // ---- empty state -------------------------------------------------------

    fn empty_page(&mut self, ui: &mut Ui, icon: Icon, title: &str, body: &str) {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(18),
                Color32::from_rgba_unmultiplied(255, 255, 255, 8),
            );
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(18),
                Stroke::new(1.0, t::BORDER),
                StrokeKind::Inside,
            );
            icons::icon(
                ui.painter(),
                Rect::from_center_size(rect.center(), Vec2::splat(28.0)),
                icon,
                t::MUTED,
            );
            ui.add_space(16.0);
            ui.label(RichText::new(title).size(20.0).strong().color(t::WHITE));
            ui.add_space(6.0);
            ui.label(RichText::new(body).size(12.5).color(t::MUTED));
            ui.add_space(18.0);
            if t::button(ui, "Back to dashboard", t::Btn::Normal) {
                self.page = Page::Dashboard;
            }
        });
    }
}

impl eframe::App for PreviewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if ui.input(|i| i.key_pressed(egui::Key::F11)) {
            let full = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
        }

        let full = ui.max_rect();
        let sidebar_w = if self.collapsed { 72.0 } else { 260.0 };
        let sidebar = Rect::from_min_size(full.min, Vec2::new(sidebar_w, full.height()));
        let main = Rect::from_min_max(egui::pos2(full.left() + sidebar_w, full.top()), full.max);

        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(sidebar)
                .layout(Layout::top_down(Align::Min)),
            |ui| self.sidebar(ui),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(main)
                .layout(Layout::top_down(Align::Min)),
            |ui| self.main(ui),
        );
    }
}

// ---- dashboard tiles -------------------------------------------------------

fn menu_button(ui: &mut Ui) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(8),
            Color32::from_rgba_unmultiplied(255, 255, 255, 16),
        );
    }
    icons::icon(ui.painter(), rect.shrink(4.0), Icon::Menu, t::MUTED);
    resp.clicked()
}

fn section_header(ui: &mut Ui, title: &str, hint: &str) {
    ui.horizontal(|ui| {
        t::section_label(ui, title);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(hint).size(10.0).color(t::DIM));
        });
    });
}

fn drag_grip(ui: &mut Ui, group: Group, pos: usize) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::drag());
    resp.dnd_set_drag_payload(DragItem { group, pos });
    let color = if resp.hovered() { t::TEXT } else { t::DIM };
    icons::icon(ui.painter(), rect.shrink(2.0), Icon::Grip, color);
}

fn drop_target(
    ui: &mut Ui,
    resp: &egui::Response,
    group: Group,
    pos: usize,
    pending: &mut Option<(Group, usize, usize)>,
) {
    if let Some(payload) = resp.dnd_hover_payload::<DragItem>() {
        if payload.group == group && payload.pos != pos {
            ui.painter().rect_stroke(
                resp.rect,
                CornerRadius::same(t::RADIUS_CARD),
                Stroke::new(2.0, t::GOLD),
                StrokeKind::Inside,
            );
        }
    }
    if let Some(payload) = resp.dnd_release_payload::<DragItem>() {
        if payload.group == group {
            *pending = Some((group, payload.pos, pos));
        }
    }
}

fn reorder(order: &mut Vec<usize>, from: usize, to: usize) {
    if from == to || from >= order.len() || to >= order.len() {
        return;
    }
    let item = order.remove(from);
    order.insert(to, item);
}

fn monitor_card(
    ui: &mut Ui,
    m: &Monitor,
    width: f32,
    pos: usize,
    pending: &mut Option<(Group, usize, usize)>,
) {
    let resp = t::card_at(ui, Vec2::new(width, 104.0), |ui| {
        let r = ui.max_rect();

        let grip_rect = Rect::from_min_size(
            egui::pos2(r.right() - 14.0, r.top() + 1.0),
            Vec2::splat(14.0),
        );
        let grip = ui.interact(
            grip_rect,
            ui.scope_id().with(("monitor-grip", pos)),
            Sense::drag(),
        );
        grip.dnd_set_drag_payload(DragItem {
            group: Group::Monitor,
            pos,
        });
        icons::icon(
            ui.painter(),
            grip_rect,
            Icon::Grip,
            if grip.hovered() { t::TEXT } else { t::DIM },
        );

        let clip = Rect::from_min_max(r.min, egui::pos2(r.right() - 18.0, r.top() + 14.0));
        ui.painter().with_clip_rect(clip).text(
            egui::pos2(r.left(), r.top() + 2.0),
            Align2::LEFT_TOP,
            m.title.to_uppercase(),
            FontId::proportional(8.5),
            t::MUTED,
        );
        ui.painter().text(
            egui::pos2(r.left(), r.top() + 26.0),
            Align2::LEFT_TOP,
            m.value,
            FontId::proportional(17.0),
            m.color,
        );
        let detail_clip = Rect::from_min_max(r.min, egui::pos2(r.right(), r.bottom()));
        ui.painter().with_clip_rect(detail_clip).text(
            egui::pos2(r.left(), r.top() + 54.0),
            Align2::LEFT_TOP,
            m.detail,
            FontId::proportional(9.0),
            t::DIM,
        );
    });
    drop_target(ui, &resp, Group::Monitor, pos, pending);
    let _ = resp.on_hover_text(format!("{} · {}", m.title, m.detail));
}

fn device_card(
    ui: &mut Ui,
    d: &Device,
    width: f32,
    pos: usize,
    pending: &mut Option<(Group, usize, usize)>,
) -> bool {
    let resp = t::card_at(ui, Vec2::new(width, 236.0), |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(d.name.to_uppercase())
                    .size(10.0)
                    .color(t::MUTED)
                    .extra_letter_spacing(1.1),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(
                    rect.center(),
                    3.0,
                    if d.connected { t::GOOD } else { t::BAD },
                );
                drag_grip(ui, Group::Device, pos);
            });
        });
        ui.add_space(4.0);

        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 92.0), Sense::hover());
        let brightness = d.brightness / 100.0;
        let color = Color32::from_rgb(80, 180, 255);
        let rainbow = matches!(d.effect, 1 | 7);
        match d.kind {
            Kind::Keyboard => icons::keyboard(ui.painter(), rect, color, brightness, rainbow),
            Kind::Mouse => icons::mouse(ui.painter(), rect, color, brightness, rainbow),
            Kind::Commander => icons::commander(ui.painter(), rect, color),
            Kind::Pump => icons::pump(ui.painter(), rect, 32.0, 50.0, t::ACCENT),
        }

        ui.add_space(6.0);
        ui.label(RichText::new(d.stat.0).size(26.0).strong().color(t::WHITE));
        ui.label(
            RichText::new(d.stat.1.to_uppercase())
                .size(9.0)
                .color(t::DIM)
                .extra_letter_spacing(1.2),
        );

        ui.add_space(8.0);
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Firmware {}", d.firmware))
                        .size(10.0)
                        .color(t::DIM),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(battery) = d.battery {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(26.0, 12.0), Sense::hover());
                        icons::battery(ui.painter(), rect, battery);
                        ui.label(
                            RichText::new(format!("{battery}%"))
                                .size(10.0)
                                .monospace()
                                .color(t::MUTED),
                        );
                    } else {
                        ui.label(RichText::new("Connected").size(10.0).color(t::MUTED));
                    }
                });
            });
        });
    });
    drop_target(ui, &resp, Group::Device, pos, pending);
    resp.clicked()
}

fn add_card(ui: &mut Ui, width: f32) {
    let resp = t::card_at(ui, Vec2::new(width, 236.0), |ui| {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
            ui.painter().circle_filled(
                rect.center(),
                22.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 10),
            );
            ui.painter()
                .circle_stroke(rect.center(), 22.0, Stroke::new(1.0, t::BORDER));
            icons::icon(
                ui.painter(),
                Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
                Icon::Plus,
                t::MUTED,
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new("Add device to dashboard")
                    .size(12.5)
                    .color(t::TEXT),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Pick a connected device")
                    .size(11.0)
                    .color(t::MUTED),
            );
            ui.add_space(14.0);
            let _ = t::button(ui, "Select device", t::Btn::Normal);
        });
    });
    let _ = resp;
}

// ---- device page cards -----------------------------------------------------

fn lighting_card(ui: &mut Ui, d: &mut Device, color: &mut [u8; 3]) {
    t::card_padded(16).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                t::section_label(ui, "Lighting");
                ui.label(
                    RichText::new(EFFECTS[d.effect])
                        .size(17.0)
                        .strong()
                        .color(t::WHITE),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                t::pill(ui, "Local preview", t::GOLD);
            });
        });

        ui.add_space(12.0);
        t::section_label(ui, "Effect");
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for (i, effect) in EFFECTS.iter().enumerate() {
                if t::chip(ui, effect, d.effect == i) {
                    d.effect = i;
                }
            }
        });

        ui.add_space(12.0);
        t::divider(ui);
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                t::section_label(ui, "Colour");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for rgb in SWATCHES {
                        if t::swatch(ui, *rgb, *color == *rgb) {
                            *color = *rgb;
                        }
                    }
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                ui.vertical(|ui| {
                    ui.set_width(220.0);
                    t::section_label(ui, "Brightness");
                    t::slider(ui, &mut d.brightness, 0.0, 100.0, "%");
                    t::section_label(ui, "Speed");
                    t::slider(ui, &mut d.speed, 0.0, 100.0, "%");
                });
            });
        });

        ui.add_space(10.0);
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 190.0), Sense::hover());
        let brightness = d.brightness / 100.0;
        let c = Color32::from_rgb(color[0], color[1], color[2]);
        let rainbow = matches!(d.effect, 1 | 7);
        match d.kind {
            Kind::Keyboard => icons::keyboard(ui.painter(), rect, c, brightness, rainbow),
            Kind::Mouse => icons::mouse(ui.painter(), rect, c, brightness, rainbow),
            Kind::Commander => icons::commander(ui.painter(), rect, c),
            Kind::Pump => icons::pump(ui.painter(), rect, 32.0, 50.0, t::ACCENT),
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new("Preview only — these controls are not wired to OpenLinkHub yet.")
                .size(10.5)
                .italics()
                .color(t::DIM),
        );
    });
}

fn assignments_card(ui: &mut Ui, d: &Device) {
    t::card_padded(16).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                t::section_label(ui, "Key assignments");
                ui.label(
                    RichText::new("Remap keys and buttons")
                        .size(15.0)
                        .strong()
                        .color(t::WHITE),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let _ = t::button(ui, "Edit layout", t::Btn::Normal);
            });
        });
        ui.add_space(10.0);
        let rows: [(&str, &str); 4] = [
            ("Escape", "Escape"),
            ("Caps Lock", "Caps Lock"),
            ("Left Alt", "Left Alt"),
            ("Side button 4", "DPI Toggle"),
        ];
        for (key, action) in rows {
            t::stat_row(ui, key, action);
        }
        if let Some(battery) = d.battery {
            t::stat_row(ui, "Battery", &format!("{battery}%"));
        }
    });
}
