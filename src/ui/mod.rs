//! Ported OpenLinkHub web UI, rendered natively with egui.
//!
//! The dashboard and sidebar are wired to the OpenLinkHub API; other screens
//! are still visual previews. Launch it with `rgbcue --preview`.

pub mod icons;
pub mod theme;

use crate::api;
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};
use icons::Icon;
use std::sync::mpsc;
use std::time::{Duration, Instant};
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
const KEY_SCOPES: &[&str] = &["Current key", "Current row", "All keys"];
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

impl Kind {
    fn api_kind(self) -> api::DeviceKind {
        match self {
            Kind::Keyboard => api::DeviceKind::Keyboard,
            Kind::Mouse => api::DeviceKind::Mouse,
            Kind::Commander => api::DeviceKind::Commander,
            Kind::Pump => api::DeviceKind::Pump,
        }
    }
}

fn default_profiles() -> Vec<String> {
    PROFILES.iter().map(|s| s.to_string()).collect()
}

fn default_rgb_modes() -> Vec<String> {
    EFFECTS.iter().map(|s| s.to_string()).collect()
}

fn default_layouts() -> Vec<String> {
    LAYOUTS.iter().map(|s| s.to_string()).collect()
}

fn default_polling_rates() -> Vec<(i64, String)> {
    POLLING
        .iter()
        .enumerate()
        .map(|(i, s)| (i as i64, s.to_string()))
        .collect()
}

#[derive(Clone)]
struct Device {
    name: String,
    kind: Kind,
    firmware: String,
    serial: String,
    connected: bool,
    battery: Option<u8>,
    profile: usize,
    brightness: f32,
    effect: usize,
    speed: f32,
    polling: usize,
    layout: usize,
    cluster: bool,
    profiles: Vec<String>,
    rgb_modes: Vec<String>,
    layouts: Vec<String>,
    polling_rates: Vec<(i64, String)>,
    keyboard: Option<api::KeyboardLayout>,
    selected_key: Option<i32>,
    key_scope: usize,
    stat: (String, String),
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
    id: String,
    title: String,
    value: String,
    detail: String,
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

enum DashMsg {
    Ok(api::Dashboard),
    Err(String),
}

enum DeviceAction {
    Profile(String),
    Rgb(String),
    Layout(String),
    Polling(i64),
    Cluster(bool),
    KeyColor {
        key_id: i32,
        option: i32,
        color: [u8; 3],
    },
    Assignments,
    Add,
    Remove,
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
    dash_rx: mpsc::Receiver<DashMsg>,
    refresh_tx: mpsc::Sender<()>,
    live: bool,
    error: Option<String>,
    add_choice: usize,
    pending_brightness: Option<(String, u8, Instant)>,
    notice: Option<(String, bool)>,
}

impl PreviewApp {
    fn new(page: Page) -> Self {
        let (refresh_tx, dash_rx) = spawn_poller();
        Self {
            page,
            collapsed: false,
            color: [80, 180, 255],
            macro_sel: 0,
            theme_idx: 0,
            lang_idx: 0,
            celsius: true,
            autostart: true,
            dash_rx,
            refresh_tx,
            live: false,
            error: None,
            add_choice: 0,
            pending_brightness: None,
            notice: None,
            monitor_order: (0..5).collect(),
            device_order: (0..5).collect(),
            monitors: vec![
                Monitor {
                    id: "cpu".into(),
                    title: "CPU temperature".into(),
                    value: "41 °C".into(),
                    detail: "Ryzen 9 7950X".into(),
                    color: t::WARN,
                },
                Monitor {
                    id: "gpu".into(),
                    title: "GPU temperature".into(),
                    value: "58 °C".into(),
                    detail: "RTX 3090".into(),
                    color: t::WARN,
                },
                Monitor {
                    id: "coolant".into(),
                    title: "Coolant temperature".into(),
                    value: "32 °C".into(),
                    detail: "H115i Platinum".into(),
                    color: t::ACCENT,
                },
                Monitor {
                    id: "fans".into(),
                    title: "Fan speed".into(),
                    value: "1 240 RPM".into(),
                    detail: "Commander PRO".into(),
                    color: t::TEXT,
                },
                Monitor {
                    id: "storage".into(),
                    title: "Storage temperature".into(),
                    value: "46 °C".into(),
                    detail: "990 PRO 2 TB".into(),
                    color: t::WARN,
                },
            ],
            devices: vec![
                Device {
                    name: "K65 RGB MINI".into(),
                    kind: Kind::Keyboard,
                    firmware: "1.0.8".into(),
                    serial: "K65-01".into(),
                    connected: true,
                    battery: None,
                    profile: 0,
                    brightness: 80.0,
                    effect: 1,
                    speed: 60.0,
                    polling: 3,
                    layout: 0,
                    cluster: false,
                    profiles: default_profiles(),
                    rgb_modes: default_rgb_modes(),
                    layouts: default_layouts(),
                    polling_rates: default_polling_rates(),
                    keyboard: None,
                    selected_key: None,
                    key_scope: 0,
                    stat: ("1000 Hz".into(), "Polling rate".into()),
                },
                Device {
                    name: "Scimitar RGB Elite".into(),
                    kind: Kind::Mouse,
                    firmware: "3.12".into(),
                    serial: "SCIM-01".into(),
                    connected: true,
                    battery: None,
                    profile: 0,
                    brightness: 75.0,
                    effect: 0,
                    speed: 50.0,
                    polling: 3,
                    layout: 0,
                    cluster: false,
                    profiles: default_profiles(),
                    rgb_modes: default_rgb_modes(),
                    layouts: default_layouts(),
                    polling_rates: default_polling_rates(),
                    keyboard: None,
                    selected_key: None,
                    key_scope: 0,
                    stat: ("18 000".into(), "DPI".into()),
                },
                Device {
                    name: "Scimitar Elite Wireless SE".into(),
                    kind: Kind::Mouse,
                    firmware: "2.1.4".into(),
                    serial: "SCIM-SE".into(),
                    connected: true,
                    battery: Some(84),
                    profile: 1,
                    brightness: 65.0,
                    effect: 7,
                    speed: 50.0,
                    polling: 3,
                    layout: 0,
                    cluster: false,
                    profiles: default_profiles(),
                    rgb_modes: default_rgb_modes(),
                    layouts: default_layouts(),
                    polling_rates: default_polling_rates(),
                    keyboard: None,
                    selected_key: None,
                    key_scope: 0,
                    stat: ("1 600".into(), "DPI".into()),
                },
                Device {
                    name: "Commander PRO".into(),
                    kind: Kind::Commander,
                    firmware: "0.9.2".into(),
                    serial: "CPRO-1".into(),
                    connected: true,
                    battery: None,
                    profile: 0,
                    brightness: 70.0,
                    effect: 0,
                    speed: 50.0,
                    polling: 0,
                    layout: 0,
                    cluster: false,
                    profiles: default_profiles(),
                    rgb_modes: default_rgb_modes(),
                    layouts: default_layouts(),
                    polling_rates: default_polling_rates(),
                    keyboard: None,
                    selected_key: None,
                    key_scope: 0,
                    stat: ("1 240".into(), "Fan RPM".into()),
                },
                Device {
                    name: "H115i RGB Platinum".into(),
                    kind: Kind::Pump,
                    firmware: "1.2.0".into(),
                    serial: "H115I-1".into(),
                    connected: true,
                    battery: None,
                    profile: 0,
                    brightness: 75.0,
                    effect: 0,
                    speed: 55.0,
                    polling: 0,
                    layout: 0,
                    cluster: false,
                    profiles: default_profiles(),
                    rgb_modes: default_rgb_modes(),
                    layouts: default_layouts(),
                    polling_rates: default_polling_rates(),
                    keyboard: None,
                    selected_key: None,
                    key_scope: 0,
                    stat: ("32 °C".into(), "Coolant".into()),
                },
            ],
        }
    }

    fn apply_device_action(&mut self, idx: usize, action: DeviceAction) {
        let d = &self.devices[idx];
        let refresh = matches!(action, DeviceAction::Add | DeviceAction::Remove);
        let message = match &action {
            DeviceAction::Profile(_) => "Profile updated",
            DeviceAction::Rgb(_) => "RGB profile applied",
            DeviceAction::Layout(_) => "Layout updated",
            DeviceAction::Polling(_) => "Polling rate updated",
            DeviceAction::Cluster(_) => "RGB cluster updated",
            DeviceAction::KeyColor { .. } => "Key colour applied",
            DeviceAction::Assignments => "Key assignments",
            DeviceAction::Add => "Added to dashboard",
            DeviceAction::Remove => "Removed from dashboard",
        };
        let result = match action {
            DeviceAction::Profile(name) => api::change_user_profile(&d.serial, &name),
            DeviceAction::Rgb(name) => api::set_rgb_profile(&d.serial, 0, &name),
            DeviceAction::Layout(name) => api::set_keyboard_layout(&d.serial, &name),
            DeviceAction::Polling(rate) => {
                api::set_polling_rate(&d.serial, d.kind.api_kind(), rate)
            }
            DeviceAction::Cluster(on) => api::set_rgb_cluster(&d.serial, on),
            DeviceAction::KeyColor {
                key_id,
                option,
                color,
            } => api::set_keyboard_color(&d.serial, key_id, option, color),
            DeviceAction::Assignments => {
                self.notice = Some(("Key assignment editor is not wired yet".into(), false));
                return;
            }
            DeviceAction::Add => api::add_dashboard_device(&d.serial),
            DeviceAction::Remove => api::remove_dashboard_device(&d.serial),
        };
        self.notice = Some(match result {
            Ok(()) => (message.into(), true),
            Err(err) => (err, false),
        });
        if refresh {
            let _ = self.refresh_tx.send(());
        }
    }

    fn apply_dashboard(&mut self, dash: api::Dashboard) {
        let selected = match self.page {
            Page::Device(i) => self.devices.get(i).map(|d| d.serial.clone()),
            _ => None,
        };
        let old_serials: Vec<String> = self.devices.iter().map(|d| d.serial.clone()).collect();
        let old_order = std::mem::take(&mut self.device_order);

        let mut next = Vec::with_capacity(dash.devices.len());
        for s in dash.devices {
            let kind = match s.kind {
                api::DeviceKind::Keyboard => Kind::Keyboard,
                api::DeviceKind::Mouse => Kind::Mouse,
                api::DeviceKind::Commander => Kind::Commander,
                api::DeviceKind::Pump => Kind::Pump,
                api::DeviceKind::Other => Kind::Mouse,
            };
            if let Some(old) = self.devices.iter().find(|d| d.serial == s.serial) {
                let mut d = old.clone();
                d.name = s.product;
                d.firmware = s.firmware;
                d.connected = s.connected;
                d.battery = s.battery;
                d.kind = kind;
                d.keyboard = s.keyboard;
                d.stat = (s.stat_value, s.stat_label);
                next.push(d);
            } else {
                let profiles = if s.profiles.is_empty() {
                    vec!["Default".to_string()]
                } else {
                    s.profiles
                };
                let profile = profiles
                    .iter()
                    .position(|p| *p == s.active_profile)
                    .unwrap_or(0);
                let rgb_modes = if s.rgb_modes.is_empty() {
                    vec![s.active_rgb.clone()]
                } else {
                    s.rgb_modes
                };
                let effect = rgb_modes
                    .iter()
                    .position(|e| *e == s.active_rgb)
                    .unwrap_or(0);
                let layouts = if s.layouts.is_empty() {
                    vec!["US".to_string()]
                } else {
                    s.layouts
                };
                let layout = layouts
                    .iter()
                    .position(|l| *l == s.active_layout)
                    .unwrap_or(0);
                let polling_rates = if s.polling_rates.is_empty() {
                    default_polling_rates()
                } else {
                    s.polling_rates
                };
                let polling = polling_rates
                    .iter()
                    .position(|(n, _)| *n == s.active_polling)
                    .unwrap_or(0);
                next.push(Device {
                    name: s.product,
                    kind,
                    firmware: s.firmware,
                    serial: s.serial,
                    connected: s.connected,
                    battery: s.battery,
                    profile,
                    brightness: s.brightness as f32,
                    effect,
                    speed: 50.0,
                    polling,
                    layout,
                    cluster: s.cluster,
                    profiles,
                    rgb_modes,
                    layouts,
                    polling_rates,
                    keyboard: s.keyboard,
                    selected_key: None,
                    key_scope: 0,
                    stat: (s.stat_value, s.stat_label),
                });
            }
        }

        let mut order = Vec::new();
        for idx in old_order {
            if let Some(serial) = old_serials.get(idx)
                && dash.dashboard.iter().any(|s| s == serial)
                && let Some(pos) = next.iter().position(|d| &d.serial == serial)
                && !order.contains(&pos)
            {
                order.push(pos);
            }
        }
        for serial in &dash.dashboard {
            if let Some(pos) = next.iter().position(|d| &d.serial == serial)
                && !order.contains(&pos)
            {
                order.push(pos);
            }
        }
        self.devices = next;
        self.device_order = order;

        let old_ids: Vec<String> = self.monitors.iter().map(|m| m.id.clone()).collect();
        let old_monitor_order = std::mem::take(&mut self.monitor_order);
        self.monitors = dash
            .monitors
            .into_iter()
            .map(|m| Monitor {
                id: m.id,
                title: m.title,
                value: m.value,
                detail: m.detail,
                color: match m.tone {
                    api::MonitorTone::Warn => t::WARN,
                    api::MonitorTone::Good => t::GOOD,
                    api::MonitorTone::Text => t::TEXT,
                },
            })
            .collect();

        let mut morder = Vec::new();
        for idx in old_monitor_order {
            if let Some(id) = old_ids.get(idx)
                && let Some(pos) = self.monitors.iter().position(|m| &m.id == id)
                && !morder.contains(&pos)
            {
                morder.push(pos);
            }
        }
        for i in 0..self.monitors.len() {
            if !morder.contains(&i) {
                morder.push(i);
            }
        }
        self.monitor_order = morder;

        if let Some(serial) = selected {
            self.page = match self.devices.iter().position(|d| d.serial == serial) {
                Some(i) => Page::Device(i),
                None => Page::Dashboard,
            };
        }
    }

    fn subtitle(&self) -> String {
        match self.page {
            Page::Dashboard => {
                let suffix = if self.error.is_some() && !self.live {
                    " · demo data"
                } else {
                    ""
                };
                format!(
                    "{} monitors · {} devices{suffix}",
                    self.monitors.len(),
                    self.devices.len()
                )
            }
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
            Page::Device(i) => self.devices[i].name.clone(),
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
                        &d.name,
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
                if let Some((message, ok)) = &self.notice {
                    ui.add_space(6.0);
                    ui.label(RichText::new(message).size(11.5).color(if *ok {
                        t::GOOD
                    } else {
                        t::BAD
                    }));
                }
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
                let (label, color) = if self.error.is_some() {
                    ("Offline", t::BAD)
                } else if self.live {
                    ("Live", t::GOOD)
                } else {
                    ("Connecting", t::MUTED)
                };
                t::pill(ui, label, color);
                ui.add_space(4.0);
                if t::button(ui, "Refresh", t::Btn::Ghost) {
                    let _ = self.refresh_tx.send(());
                }
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
        let mw = 260.0;
        let per_row = (((avail + mgap) / (mw + mgap)).floor() as usize).max(1);
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

        let dashboard_serials: Vec<String> = device_order
            .iter()
            .filter_map(|i| self.devices.get(*i))
            .map(|d| d.serial.clone())
            .collect();
        let available: Vec<(String, String)> = self
            .devices
            .iter()
            .filter(|d| !dashboard_serials.contains(&d.serial))
            .map(|d| (d.serial.clone(), d.name.clone()))
            .collect();
        let available_names: Vec<String> = available.iter().map(|(_, n)| n.clone()).collect();
        let mut add_choice = self.add_choice.min(available.len().saturating_sub(1));
        let mut add_serial: Option<String> = None;

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
                        if add_card(ui, dw, &available_names, &mut add_choice)
                            && let Some((serial, _)) = available.get(add_choice)
                        {
                            add_serial = Some(serial.clone());
                        }
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
        self.add_choice = add_choice;
        if let Some(serial) = add_serial {
            match api::add_dashboard_device(&serial) {
                Ok(()) => {
                    self.notice = Some(("Added to dashboard".into(), true));
                    let _ = self.refresh_tx.send(());
                }
                Err(err) => self.notice = Some((err, false)),
            }
        }
        if let Some(page) = next_page {
            self.page = page;
        }
    }

    // ---- device ------------------------------------------------------------

    fn device_page(&mut self, ui: &mut Ui, idx: usize) {
        let on_dashboard = self.device_order.contains(&idx);
        let color = &mut self.color;
        let d = &mut self.devices[idx];
        let mut action: Option<DeviceAction> = None;
        let mut brightness: Option<f32> = None;

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
                        ui.label(
                            RichText::new(d.name.as_str())
                                .size(14.0)
                                .strong()
                                .color(t::WHITE),
                        );
                        ui.add_space(8.0);

                        t::stat_row(ui, "Firmware", &d.firmware);
                        t::settings_row(ui, "Profile", |ui| {
                            let changed = t::select_strings(ui, &d.profiles, &mut d.profile);
                            if changed {
                                action = Some(DeviceAction::Profile(d.profiles[d.profile].clone()));
                            }
                            changed
                        });
                        t::settings_row(ui, "Brightness", |ui| {
                            let changed = t::slider(ui, &mut d.brightness, 0.0, 100.0, "%");
                            if changed {
                                brightness = Some(d.brightness);
                            }
                            changed
                        });
                        t::settings_row(ui, "RGB profile", |ui| {
                            let changed = t::select_strings(ui, &d.rgb_modes, &mut d.effect);
                            if changed {
                                action = Some(DeviceAction::Rgb(d.rgb_modes[d.effect].clone()));
                            }
                            changed
                        });
                        if d.kind == Kind::Keyboard {
                            t::settings_row(ui, "Layout", |ui| {
                                let changed = t::select_strings(ui, &d.layouts, &mut d.layout);
                                if changed {
                                    action =
                                        Some(DeviceAction::Layout(d.layouts[d.layout].clone()));
                                }
                                changed
                            });
                        }
                        if matches!(d.kind, Kind::Keyboard | Kind::Mouse) {
                            let polling_names: Vec<String> =
                                d.polling_rates.iter().map(|(_, n)| n.clone()).collect();
                            t::settings_row(ui, "Polling rate", |ui| {
                                let changed = t::select_strings(ui, &polling_names, &mut d.polling);
                                if changed {
                                    action =
                                        Some(DeviceAction::Polling(d.polling_rates[d.polling].0));
                                }
                                changed
                            });
                        }
                        if d.kind == Kind::Keyboard {
                            t::settings_row(ui, "Performance", |ui| {
                                t::button(ui, "Configure", t::Btn::Normal)
                            });
                        }
                        t::settings_row(ui, "RGB Cluster", |ui| {
                            let changed = t::switch(ui, &mut d.cluster);
                            if changed {
                                action = Some(DeviceAction::Cluster(d.cluster));
                            }
                            changed
                        });

                        ui.add_space(14.0);
                        let (label, kind) = if on_dashboard {
                            ("Remove from dashboard", t::Btn::Danger)
                        } else {
                            ("Add to dashboard", t::Btn::Normal)
                        };
                        if t::button(ui, label, kind) {
                            action = Some(if on_dashboard {
                                DeviceAction::Remove
                            } else {
                                DeviceAction::Add
                            });
                        }
                    });
                },
            );

            ui.add_space(16.0);

            ui.vertical(|ui| {
                let has_layout = d.kind == Kind::Keyboard && d.keyboard.is_some();
                if has_layout {
                    if let Some(next) = keyboard_editor(ui, d, color) {
                        action = Some(next);
                    }
                } else {
                    let mut rgb_action: Option<String> = None;
                    lighting_card(ui, d, color, &mut brightness, &mut rgb_action);
                    if let Some(name) = rgb_action {
                        action = Some(DeviceAction::Rgb(name));
                    }
                    ui.add_space(16.0);
                    assignments_card(ui, d);
                }
            });
        });

        if let Some(value) = brightness {
            let serial = self.devices[idx].serial.clone();
            self.pending_brightness = Some((
                serial,
                value.round().clamp(0.0, 100.0) as u8,
                Instant::now(),
            ));
        }
        if let Some(action) = action {
            self.apply_device_action(idx, action);
        }
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

        while let Ok(msg) = self.dash_rx.try_recv() {
            match msg {
                DashMsg::Ok(dash) => {
                    self.apply_dashboard(dash);
                    self.live = true;
                    self.error = None;
                }
                DashMsg::Err(err) => self.error = Some(err),
            }
        }
        if let Some((serial, value, at)) = &self.pending_brightness
            && at.elapsed() >= Duration::from_millis(250)
        {
            let serial = serial.clone();
            let value = *value;
            self.pending_brightness = None;
            self.notice = Some(match api::set_brightness(&serial, value) {
                Ok(()) => ("Brightness updated".into(), true),
                Err(err) => (err, false),
            });
        }
        ui.ctx().request_repaint_after(Duration::from_millis(750));

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

fn spawn_poller() -> (mpsc::Sender<()>, mpsc::Receiver<DashMsg>) {
    let (tx, rx) = mpsc::channel();
    let (req_tx, req_rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let msg = match api::fetch_dashboard() {
                Ok(dash) => DashMsg::Ok(dash),
                Err(err) => DashMsg::Err(err),
            };
            if tx.send(msg).is_err() {
                break;
            }
            match req_rx.recv_timeout(Duration::from_secs(4)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    (req_tx, rx)
}

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

fn ellipsize(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn monitor_card(
    ui: &mut Ui,
    m: &Monitor,
    width: f32,
    pos: usize,
    pending: &mut Option<(Group, usize, usize)>,
) {
    let resp = t::card_at(ui, Vec2::new(width, 112.0), |ui| {
        let r = ui.max_rect();

        let grip_rect = Rect::from_min_size(
            egui::pos2(r.right() - 16.0, r.top() + 2.0),
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

        ui.painter().text(
            egui::pos2(r.left(), r.top() + 2.0),
            Align2::LEFT_TOP,
            m.title.to_uppercase(),
            FontId::proportional(11.0),
            t::MUTED,
        );
        ui.painter().text(
            egui::pos2(r.left(), r.top() + 24.0),
            Align2::LEFT_TOP,
            &m.value,
            FontId::proportional(28.0),
            m.color,
        );
        ui.painter().text(
            egui::pos2(r.left(), r.top() + 64.0),
            Align2::LEFT_TOP,
            ellipsize(&m.detail, 38),
            FontId::proportional(12.0),
            t::MUTED,
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
        ui.label(
            RichText::new(d.stat.0.as_str())
                .size(26.0)
                .strong()
                .color(t::WHITE),
        );
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

fn add_card(ui: &mut Ui, width: f32, names: &[String], choice: &mut usize) -> bool {
    let mut save = false;
    t::card_at(ui, Vec2::new(width, 236.0), |ui| {
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
            ui.add_space(8.0);
            if names.is_empty() {
                ui.label(
                    RichText::new("All devices are on the dashboard")
                        .size(11.0)
                        .color(t::MUTED),
                );
            } else {
                let _ = t::select_strings_sized(ui, names, choice, 200.0);
                ui.add_space(8.0);
                if t::button(ui, "Save", t::Btn::Normal) {
                    save = true;
                }
            }
        });
    });
    save
}

// ---- device page cards -----------------------------------------------------

fn keyboard_editor(ui: &mut Ui, d: &mut Device, color: &mut [u8; 3]) -> Option<DeviceAction> {
    let mut action = None;
    t::card_padded(16).show(ui, |ui| {
        let selected_name = d.selected_key.and_then(|index| {
            d.keyboard.as_ref().and_then(|layout| {
                layout
                    .rows
                    .iter()
                    .flat_map(|row| row.keys.iter())
                    .find(|key| key.index == index)
                    .map(|key| key.name.clone())
            })
        });

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                t::section_label(ui, "Keyboard");
                ui.label(
                    RichText::new(
                        selected_name
                            .map(|name| format!("Selected: {name}"))
                            .unwrap_or_else(|| "Click a key to select".into()),
                    )
                    .size(15.0)
                    .strong()
                    .color(t::WHITE),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                t::pill(ui, "Live", t::GOOD);
            });
        });

        ui.add_space(12.0);
        let Some(layout) = d.keyboard.clone() else {
            return;
        };
        let max_units = layout
            .rows
            .iter()
            .map(|row| row.keys.iter().map(|key| key.width).sum::<f32>())
            .fold(0.0_f32, f32::max);
        let avail = ui.available_width();
        let gap = 3.0;
        let unit_w = ((avail - gap * 16.0) / max_units.max(1.0)).clamp(10.0, 40.0);
        let key_h = (unit_w * 0.9).clamp(20.0, 36.0);
        let mut clicked = None;

        for (ri, row) in layout.rows.iter().enumerate() {
            let total = row.keys.iter().map(|key| key.width * unit_w).sum::<f32>()
                + gap * row.keys.len().saturating_sub(1) as f32;
            let (row_rect, _) = ui.allocate_exact_size(Vec2::new(avail, key_h), Sense::hover());
            let mut x = row_rect.left() + (avail - total) / 2.0;

            for key in &row.keys {
                let w = key.width * unit_w;
                let rect = Rect::from_min_size(egui::pos2(x, row_rect.top()), Vec2::new(w, key_h));
                let id = ui.scope_id().with(("keycap", ri, key.index));
                let resp = ui.interact(rect, id, Sense::click());
                let painter = ui.painter();
                let base = Color32::from_rgb(key.color[0], key.color[1], key.color[2]);
                let selected = d.selected_key == Some(key.index);
                let fill = if selected {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 26)
                } else if resp.hovered() {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 13)
                } else {
                    Color32::from_rgb(0x10, 0x10, 0x11)
                };
                painter.rect_filled(rect, CornerRadius::same(3), fill);
                painter.rect_stroke(
                    rect,
                    CornerRadius::same(3),
                    Stroke::new(
                        if selected { 2.0 } else { 1.0 },
                        if selected { t::GOLD } else { t::BORDER },
                    ),
                    StrokeKind::Inside,
                );

                // The web UI colours the legend, not the cap.
                let fg = base;
                let label = if key.name == "<---" {
                    "←"
                } else {
                    key.name.as_str()
                };
                let font = FontId::proportional((key_h * 0.34).clamp(6.5, 10.5));
                let label_y = if key.sub.is_empty() {
                    rect.center().y
                } else {
                    rect.center().y - key_h * 0.14
                };
                painter.text(
                    egui::pos2(rect.center().x, label_y),
                    Align2::CENTER_CENTER,
                    label,
                    font,
                    fg,
                );
                if !key.sub.is_empty() {
                    painter.text(
                        egui::pos2(rect.center().x, rect.center().y + key_h * 0.24),
                        Align2::CENTER_CENTER,
                        &key.sub,
                        FontId::proportional((key_h * 0.2).clamp(5.5, 7.5)),
                        fg,
                    );
                }
                if resp.clicked() {
                    clicked = Some(key.index);
                }
                x += w + gap;
            }
            ui.add_space(gap);
        }

        if let Some(index) = clicked {
            d.selected_key = Some(index);
        }

        ui.add_space(8.0);
        t::divider(ui);
        ui.add_space(8.0);

        let scopes: Vec<String> = KEY_SCOPES.iter().map(|s| s.to_string()).collect();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for swatch in SWATCHES {
                if t::swatch(ui, *swatch, *color == *swatch) {
                    *color = *swatch;
                }
            }
            ui.add_space(6.0);
            t::select_strings(ui, &scopes, &mut d.key_scope);
            let can_apply = d.selected_key.is_some() || d.key_scope == 2;
            if t::button(
                ui,
                "Apply",
                if can_apply {
                    t::Btn::Success
                } else {
                    t::Btn::Normal
                },
            ) && can_apply
            {
                action = Some(DeviceAction::KeyColor {
                    key_id: d.selected_key.unwrap_or(1),
                    option: d.key_scope as i32,
                    color: *color,
                });
            }
            if t::button(ui, "Key Assignments", t::Btn::Normal) {
                action = Some(DeviceAction::Assignments);
            }
        });
        if d.selected_key.is_none() && d.key_scope != 2 {
            ui.add_space(4.0);
            ui.label(
                RichText::new("Select a key, or choose All keys.")
                    .size(10.0)
                    .color(t::DIM),
            );
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Per-key colour applies to OpenLinkHub; key assignment editing is not wired yet.",
            )
            .size(10.0)
            .italics()
            .color(t::DIM),
        );
    });
    action
}

fn lighting_card(
    ui: &mut Ui,
    d: &mut Device,
    color: &mut [u8; 3],
    brightness_out: &mut Option<f32>,
    rgb_out: &mut Option<String>,
) {
    t::card_padded(16).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                t::section_label(ui, "Lighting");
                ui.label(
                    RichText::new(
                        d.rgb_modes
                            .get(d.effect)
                            .map(String::as_str)
                            .unwrap_or("Static"),
                    )
                        .size(17.0)
                        .strong()
                        .color(t::WHITE),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                t::pill(ui, "Live", t::GOOD);
            });
        });

        ui.add_space(12.0);
        t::section_label(ui, "Effect");
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            let modes = d.rgb_modes.clone();
            for (i, effect) in modes.iter().enumerate() {
                if t::chip(ui, effect, d.effect == i) {
                    d.effect = i;
                    *rgb_out = Some(effect.clone());
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
                    if t::slider(ui, &mut d.brightness, 0.0, 100.0, "%") {
                        *brightness_out = Some(d.brightness);
                    }
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
            RichText::new("Effect, brightness, layout and polling apply to OpenLinkHub; colour and speed are preview-only.")
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
