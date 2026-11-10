//! RGBCUE — native Linux companion for Corsair keyboards and mice.
//!
//! This is the Rust/egui frontend. It talks to a local OpenLinkHub service
//! (http://127.0.0.1:27003) over HTTP; OpenLinkHub owns the per-device HID
//! protocol, and this app owns the interface. A background thread polls the
//! device list so the UI never blocks on the network.

mod api;
mod ui;

use eframe::egui;
use std::sync::mpsc;
use std::time::Duration;

/// Messages the poller thread sends back to the UI.
enum Poll {
    Devices(Result<Vec<api::Device>, String>),
}

/// Which top-level page is shown in the main panel.
#[derive(Clone, Copy, PartialEq)]
enum View {
    Devices,
    Macros,
}

/// Pending per-action edits (hold / repeat / repeat-delay), kept in `self` so
/// they persist across frames until explicitly saved.
#[derive(Clone, Copy, Default)]
struct ActionEdit {
    hold: bool,
    repeat: u8,
    repeat_delay: u16,
}

struct RgbcueApp {
    rx: mpsc::Receiver<Poll>,
    devices: Vec<api::Device>,
    selected: Option<String>,
    connected: bool,
    error: Option<String>,
    view: View,

    // Lighting (milestone 2).
    rgb: Option<(String, api::RgbData)>,
    color: [u8; 3],
    active_profile: Option<(String, String)>,
    lighting_result: Option<Result<String, String>>,

    // Macros (milestone 3).
    macros: Vec<api::Macro>,
    selected_macro: Option<i32>,
    macro_dirty: bool,
    macro_result: Option<Result<String, String>>,
    new_macro_name: String,
    new_action_type: u8,
    new_action_command: u16,
    new_action_delay: u16,
    new_action_text: String,
    new_action_enter: bool,
    new_action_x: i32,
    new_action_y: i32,
    new_action_abs: bool,
    // Key/button tables for pickers (index -> name).
    keys: Vec<(u16, String)>,
    media: Vec<(u16, String)>,
    mouse: Vec<(u16, String)>,
    keys_loaded: bool,
    key_search: String,
    capture_armed: bool,
    f8_rx: mpsc::Receiver<()>,
    hotkey: String,
    text_delay: i32,
    text_variance: i32,
    delay_variance: i32,
    // Pending repeat edits for the selected macro (persist across frames).
    edit_macro_id: Option<i32>,
    repeat_loop: bool,
    repeat_count: i32,
    repeat_delay: i32,
    // Per-action editing.
    selected_action: Option<i32>,
    action_edits: std::collections::HashMap<i32, ActionEdit>,
}

impl RgbcueApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            rx: spawn_poller(),
            devices: Vec::new(),
            selected: None,
            connected: false,
            error: None,
            rgb: None,
            color: [255, 255, 255],
            active_profile: None,
            lighting_result: None,
            view: View::Devices,
            macros: Vec::new(),
            selected_macro: None,
            macro_dirty: true,
            macro_result: None,
            new_macro_name: String::new(),
            new_action_type: api::ACTION_KEYBOARD,
            new_action_command: 0,
            new_action_delay: 100,
            new_action_text: String::new(),
            new_action_enter: false,
            new_action_x: 0,
            new_action_y: 0,
            new_action_abs: true,
            keys: Vec::new(),
            media: Vec::new(),
            mouse: Vec::new(),
            keys_loaded: false,
            key_search: String::new(),
            capture_armed: false,
            f8_rx: spawn_f8_listener(66),
            hotkey: "F8".to_string(),
            text_delay: 40,
            text_variance: 0,
            delay_variance: 0,
            edit_macro_id: None,
            repeat_loop: false,
            repeat_count: 0,
            repeat_delay: 0,
            selected_action: None,
            action_edits: std::collections::HashMap::new(),
        }
    }

    /// Drain whatever the poller thread has sent and fold it into app state.
    fn poll(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Poll::Devices(Ok(devices)) => {
                    self.devices = devices;
                    self.connected = true;
                    self.error = None;
                    // Drop a selection whose device has gone away.
                    if let Some(sel) = &self.selected {
                        if !self.devices.iter().any(|d| &d.serial == sel) {
                            self.selected = None;
                        }
                    }
                }
                Poll::Devices(Err(e)) => {
                    self.connected = false;
                    self.error = Some(e);
                }
            }
        }
        // Handle the capture hotkey. When armed, a hotkey press captures the
        // cursor position. Otherwise, discard stale events.
        if self.capture_armed {
            if self.f8_rx.try_recv().is_ok() {
                if let Some((x, y)) = query_cursor() {
                    self.new_action_x = x;
                    self.new_action_y = y;
                }
                self.capture_armed = false;
            }
        } else {
            while self.f8_rx.try_recv().is_ok() {}
        }
    }

    /// Make sure `self.rgb` holds the selected device's lighting data.
    /// (Synchronous on first selection; localhost, so effectively instant.)
    fn ensure_rgb(&mut self, serial: &str) {
        let needs = match &self.rgb {
            Some((s, _)) => s != serial,
            None => true,
        };
        if !needs {
            return;
        }
        self.lighting_result = None;
        self.active_profile = None;
        match api::fetch_rgb_profiles(serial) {
            Ok(rgb) => {
                if let Some(static_p) = rgb.profiles.get("static") {
                    self.color = [
                        clamp_u8(static_p.start.red),
                        clamp_u8(static_p.start.green),
                        clamp_u8(static_p.start.blue),
                    ];
                }
                self.rgb = Some((serial.to_string(), rgb));
            }
            Err(e) => {
                self.rgb = None;
                self.lighting_result = Some(Err(format!("could not load lighting: {e}")));
            }
        }
    }

    fn draw_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("RGBCUE");
        ui.add_space(8.0);

        ui.strong("WORKSPACE");
        ui.separator();
        if ui.selectable_label(self.view == View::Devices, "Devices").clicked() {
            self.view = View::Devices;
        }
        if ui.selectable_label(self.view == View::Macros, "Macros").clicked() {
            self.view = View::Macros;
            self.macro_dirty = true;
        }
        ui.add_space(6.0);

        ui.strong("DEVICES");
        ui.separator();

        if self.devices.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(if self.connected {
                    "No devices connected"
                } else {
                    "Waiting for OpenLinkHub…"
                })
                .weak(),
            );
        }

        for d in &self.devices {
            let is_selected = self.selected.as_deref() == Some(d.serial.as_str());
            if ui.selectable_label(is_selected, &d.product).clicked() {
                self.selected = Some(d.serial.clone());
            }
            ui.label(egui::RichText::new(d.kind()).size(10.0).weak());
            ui.add_space(2.0);
        }

        // Status footer pinned to the bottom of the sidebar.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.separator();
            if let Some(err) = &self.error {
                ui.label(egui::RichText::new("OpenLinkHub unreachable").color(egui::Color32::LIGHT_RED));
                ui.label(egui::RichText::new(err).size(10.0).weak());
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "{} device{} · connected",
                        self.devices.len(),
                        if self.devices.len() == 1 { "" } else { "s" }
                    ))
                    .weak(),
                );
            }
        });
    }

    fn draw_main(&mut self, ui: &mut egui::Ui) {
        match self.view {
            View::Devices => self.draw_device_view(ui),
            View::Macros => self.draw_macro_view(ui),
        }
    }

    fn draw_device_view(&mut self, ui: &mut egui::Ui) {
        let Some(serial) = self.selected.clone() else {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("Select a device from the sidebar").weak());
            });
            return;
        };
        let Some(dev) = self.devices.iter().find(|d| d.serial == serial).cloned() else {
            ui.label("Device is no longer connected.");
            return;
        };

        ui.add_space(4.0);
        ui.heading(&dev.product);
        ui.label(egui::RichText::new(dev.kind()).color(egui::Color32::GRAY));
        ui.add_space(8.0);
        ui.separator();

        egui::Grid::new("device-info")
            .num_columns(2)
            .spacing([24.0, 8.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Serial").weak());
                ui.monospace(&dev.serial);
                ui.end_row();

                ui.label(egui::RichText::new("Product ID").weak());
                ui.monospace(format!("0x{:04X}", dev.product_id));
                ui.end_row();

                ui.label(egui::RichText::new("Device type").weak());
                ui.monospace(dev.device_type.to_string());
                ui.end_row();

                ui.label(egui::RichText::new("Firmware").weak());
                ui.monospace(if dev.firmware.is_empty() { "—" } else { &dev.firmware });
                ui.end_row();

                ui.label(egui::RichText::new("HID path").weak());
                ui.monospace(if dev.device_path.is_empty() { "—" } else { &dev.device_path });
                ui.end_row();
            });

        self.draw_lighting(ui, &serial);
    }

    fn draw_lighting(&mut self, ui: &mut egui::Ui, serial: &str) {
        self.ensure_rgb(serial);

        ui.add_space(12.0);
        ui.separator();
        ui.strong("Lighting presets");
        ui.add_space(2.0);
        ui.label(egui::RichText::new("Click a preset to apply it").weak());

        // Snapshot the profiles (keeping their internal keys) so we can drop
        // the borrow before mutating state below.
        let profiles: Vec<(String, String, [u8; 3])> = self
            .rgb
            .as_ref()
            .map(|(_, rgb)| {
                let mut v: Vec<(String, String, [u8; 3])> = rgb
                    .profiles
                    .iter()
                    .map(|(key, p)| {
                        let name = if p.name.is_empty() { key.clone() } else { p.name.clone() };
                        (
                            key.clone(),
                            name,
                            [clamp_u8(p.start.red), clamp_u8(p.start.green), clamp_u8(p.start.blue)],
                        )
                    })
                    .collect();
                v.sort_by(|a, b| a.1.cmp(&b.1));
                v
            })
            .unwrap_or_default();

        if profiles.is_empty() {
            ui.label(egui::RichText::new("No lighting profiles reported").weak());
        } else {
            let selected_text = self
                .active_profile
                .as_ref()
                .map(|(_, name)| name.clone())
                .unwrap_or_else(|| "Select preset…".to_string());
            let mut activate: Option<(String, String)> = None;
            egui::ComboBox::from_id_salt("preset")
                .selected_text(selected_text)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for (key, name, rgb) in &profiles {
                        let is_selected =
                            self.active_profile.as_ref().map(|(k, _)| k == key).unwrap_or(false);
                        ui.horizontal(|ui| {
                            swatch(ui, rgb[0], rgb[1], rgb[2]);
                            if ui.selectable_label(is_selected, name).clicked() {
                                activate = Some((key.clone(), name.clone()));
                            }
                        });
                    }
                });
            if let Some((key, name)) = activate {
                self.lighting_result = Some(match api::activate_profile(serial, &key) {
                    Ok(()) => Ok(format!("Activated {name}")),
                    Err(e) => Err(e),
                });
                self.active_profile = Some((key, name));
            }
        }

        // Solid colour, tucked away since it's a secondary control.
        ui.add_space(10.0);
        egui::CollapsingHeader::new("Solid colour")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut self.color[0], 0..=255).text("R"));
                    ui.add(egui::Slider::new(&mut self.color[1], 0..=255).text("G"));
                    ui.add(egui::Slider::new(&mut self.color[2], 0..=255).text("B"));
                });
                ui.horizontal(|ui| {
                    swatch(ui, self.color[0], self.color[1], self.color[2]);
                    if ui.button("Apply").clicked() {
                        let (r, g, b) = (self.color[0], self.color[1], self.color[2]);
                        self.lighting_result = Some(api::set_solid_color(serial, r, g, b)
                            .map(|_| "Solid colour applied".to_string()));
                    }
                });
            });

        if let Some(result) = &self.lighting_result {
            match result {
                Ok(msg) => {
                    ui.label(
                        egui::RichText::new(msg).color(egui::Color32::from_rgb(0x2e, 0xcc, 0x40)),
                    );
                }
                Err(e) => {
                    ui.label(
                        egui::RichText::new(format!("Error: {e}"))
                            .color(egui::Color32::LIGHT_RED),
                    );
                }
            }
        }
    }

    /// Fetch the macro library once per change (guarded by `macro_dirty`).
    fn ensure_macros(&mut self) {
        if !self.macro_dirty {
            return;
        }
        self.macro_dirty = false;
        match api::fetch_macros() {
            Ok(macros) => {
                self.macros = macros;
                if let Some(sel) = self.selected_macro {
                    if !self.macros.iter().any(|m| m.id == sel) {
                        self.selected_macro = None;
                    }
                }
            }
            Err(e) => {
                self.macro_result = Some(Err(format!("could not load macros: {e}")));
            }
        }
    }

    /// Load the key/button tables once, lazily, for the pickers.
    fn ensure_keys(&mut self) {
        if self.keys_loaded {
            return;
        }
        self.keys_loaded = true;
        let pairs = |v: Result<Vec<(u16, api::InputKey)>, String>| -> Vec<(u16, String)> {
            v.map(|v| v.into_iter().map(|(i, k)| (i, k.name)).collect())
                .unwrap_or_default()
        };
        self.keys = pairs(api::fetch_keyboard_keys());
        self.media = pairs(api::fetch_media_keys());
        self.mouse = pairs(api::fetch_mouse_buttons());
    }

    /// Index of the "Enter" key (for the text "send Enter" convenience).
    fn enter_index(&self) -> u16 {
        self.keys
            .iter()
            .find(|(_, n)| n == "Enter")
            .map(|(i, _)| *i)
            .unwrap_or(68)
    }

    /// Human-readable name for an action's command (key/button), if known.
    fn command_name(&self, ty: u8, cmd: u16) -> Option<String> {
        let list = match ty {
            api::ACTION_KEYBOARD => &self.keys,
            api::ACTION_MEDIA => &self.media,
            api::ACTION_MOUSE => &self.mouse,
            _ => return None,
        };
        list.iter().find(|(i, _)| *i == cmd).map(|(_, n)| n.clone())
    }

    /// Re-watch the capture hotkey under a new key name.
    fn respawn_hotkey(&mut self, key: &str) -> Result<(), String> {
        match key_name_to_code(key) {
            Some(code) => {
                self.f8_rx = spawn_f8_listener(code);
                Ok(())
            }
            None => Err(format!("Unknown key: {key}")),
        }
    }

    /// Add an action from the form fields, optionally appending Enter for text.
    fn add_action(
        &mut self,
        id: i32,
        ty: u8,
        cmd: u16,
        dly: u16,
        text: &str,
        x: i32,
        y: i32,
        abs: bool,
        enter: bool,
    ) {
        let a = api::MacroAction {
            action_type: ty,
            action_command: cmd,
            action_delay: if ty == api::ACTION_DELAY { dly } else { 0 },
            action_text: text.to_string(),
            mouse_position_x: x,
            mouse_position_y: y,
            mouse_position_absolute: abs,
            ..Default::default()
        };
        match api::add_macro_action(id, &a) {
            Ok(()) => {
                self.macro_result = Some(Ok("Action added".to_string()));
                if ty == api::ACTION_TEXT && enter {
                    let enter_a = api::MacroAction {
                        action_type: api::ACTION_KEYBOARD,
                        action_command: self.enter_index(),
                        ..Default::default()
                    };
                    if api::add_macro_action(id, &enter_a).is_ok() {
                        self.macro_result = Some(Ok("Action + Enter added".to_string()));
                    }
                }
            }
            Err(e) => self.macro_result = Some(Err(e)),
        }
        self.macro_dirty = true;
        self.new_action_text.clear();
    }

    fn draw_macro_view(&mut self, ui: &mut egui::Ui) {
        self.ensure_macros();
        self.ensure_keys();

        ui.add_space(4.0);
        ui.heading("Macros");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("New macro:");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_macro_name)
                    .hint_text("Name")
                    .desired_width(180.0),
            );
            if ui.button("Create").clicked() {
                let name = self.new_macro_name.trim().to_string();
                if !name.is_empty() {
                    self.macro_result =
                        Some(api::create_macro(&name).map(|_| format!("Created \"{name}\"")));
                    self.new_macro_name.clear();
                    self.macro_dirty = true;
                }
            }
        });

        if let Some(result) = &self.macro_result {
            match result {
                Ok(msg) => {
                    ui.label(
                        egui::RichText::new(msg).color(egui::Color32::from_rgb(0x2e, 0xcc, 0x40)),
                    );
                }
                Err(e) => {
                    ui.label(
                        egui::RichText::new(format!("Error: {e}"))
                            .color(egui::Color32::LIGHT_RED),
                    );
                }
            }
        }

        egui::CollapsingHeader::new("Typing & delay settings")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Char delay");
                    ui.add(
                        egui::DragValue::new(&mut self.text_delay)
                            .range(10..=1000)
                            .suffix(" ms"),
                    );
                    ui.label("char jitter");
                    ui.add(
                        egui::DragValue::new(&mut self.text_variance)
                            .range(0..=500)
                            .suffix(" ms"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Delay jitter");
                    ui.add(
                        egui::DragValue::new(&mut self.delay_variance)
                            .range(0..=500)
                            .suffix(" ms"),
                    );
                    if ui.button("Save").clicked() {
                        self.macro_result = Some(
                            api::set_text_delay(
                                self.text_delay,
                                self.text_variance,
                                self.delay_variance,
                            )
                            .map(|_| "Settings saved".to_string()),
                        );
                    }
                });
            });

        egui::CollapsingHeader::new("Capture hotkey")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Hotkey");
                    ui.add(egui::TextEdit::singleline(&mut self.hotkey).desired_width(60.0));
                    if ui.button("Apply").clicked() {
                        let key = self.hotkey.clone();
                        self.macro_result =
                            Some(self.respawn_hotkey(&key).map(|_| "Hotkey updated".to_string()));
                    }
                });
            });

        ui.separator();

        if self.macros.is_empty() {
            ui.label(egui::RichText::new("No macros yet.").weak());
            return;
        }

        // Snapshot list + selection so we can mutate `self` freely in the closures.
        let sel = self.selected_macro;
        let list: Vec<(i32, String)> = self
            .macros
            .iter()
            .map(|m| (m.id, m.name.clone()))
            .collect();
        let mut select: Option<i32> = None;
        let mut delete: Option<i32> = None;

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(210.0);
                for (id, name) in &list {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(sel == Some(*id), name).clicked() {
                            select = Some(*id);
                        }
                        if ui.small_button("✕").clicked() {
                            delete = Some(*id);
                        }
                    });
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                self.draw_macro_editor(ui);
            });
        });

        if let Some(id) = select {
            self.selected_macro = Some(id);
        }
        if let Some(id) = delete {
            self.macro_result = Some(api::delete_macro(id).map(|_| "Deleted macro".to_string()));
            if self.selected_macro == Some(id) {
                self.selected_macro = None;
            }
            self.macro_dirty = true;
        }
    }

    fn draw_macro_editor(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected_macro else {
            ui.label(egui::RichText::new("Select a macro to edit.").weak());
            return;
        };
        let Some(m) = self.macros.iter().find(|m| m.id == id).cloned() else {
            ui.label("Macro no longer exists.");
            return;
        };

        ui.heading(&m.name);
        ui.separator();

        // Repeat settings. These live in `self` (not locals) so the checkbox and
        // sliders keep their state across frames until "Save repeat" is clicked.
        ui.label(egui::RichText::new("Repeat").strong());
        if self.edit_macro_id != Some(id) {
            self.edit_macro_id = Some(id);
            self.repeat_loop = m.repeat < 0;
            self.repeat_count = m.repeat.max(0);
            self.repeat_delay = m.repeat_delay;
            self.selected_action = None;
            self.action_edits.clear();
            for (idx, a) in api::ordered_actions(&m) {
                self.action_edits.insert(
                    idx,
                    ActionEdit {
                        hold: a.action_hold,
                        repeat: a.action_repeat,
                        repeat_delay: a.action_repeat_delay,
                    },
                );
            }
        }
        ui.checkbox(&mut self.repeat_loop, "Loop constantly");
        if !self.repeat_loop {
            ui.horizontal(|ui| {
                ui.label("Repeat");
                ui.add(egui::DragValue::new(&mut self.repeat_count).range(0..=9999));
                ui.label("times");
            });
        }
        ui.horizontal(|ui| {
            ui.label("Delay");
            ui.add(egui::DragValue::new(&mut self.repeat_delay).suffix(" ms").range(0..=60000));
            if ui.button("Save repeat").clicked() {
                let r = if self.repeat_loop { -1 } else { self.repeat_count };
                self.macro_result = Some(
                    api::update_macro_settings(id, r, self.repeat_delay)
                        .map(|_| "Repeat saved".to_string()),
                );
                self.macro_dirty = true;
            }
        });

        ui.separator();

        // Actions list (click to select; drag to reorder).
        ui.label(egui::RichText::new("Actions (drag to reorder)").strong());
        let full: Vec<(i32, api::MacroAction)> = api::ordered_actions(&m)
            .into_iter()
            .map(|(idx, a)| (idx, a.clone()))
            .collect();
        if full.is_empty() {
            ui.label(egui::RichText::new("No actions.").weak());
        }
        let mut select_action: Option<i32> = None;
        let mut delete_action: Option<i32> = None;
        let mut drop_pair: Option<(i32, i32)> = None;
        for (idx, a) in &full {
            let label = match a.action_type {
                api::ACTION_DELAY => format!("{idx}. Delay ({}ms)", a.action_delay),
                api::ACTION_TEXT => format!("{idx}. Text"),
                api::ACTION_MOUSE_POSITION => format!("{idx}. Mouse position"),
                _ => {
                    let name = self
                        .command_name(a.action_type, a.action_command)
                        .unwrap_or_else(|| format!("cmd {}", a.action_command));
                    format!("{idx}. {} ({name})", api::action_type_label(a.action_type))
                }
            };
            ui.horizontal(|ui| {
                let resp = ui.dnd_drag_source(egui::Id::unique(("action", *idx)), *idx, |ui| {
                    ui.selectable_label(self.selected_action == Some(*idx), label)
                });
                if resp.inner.clicked() {
                    select_action = Some(*idx);
                }
                if let Some(dragged) = resp.response.dnd_release_payload::<i32>() {
                    drop_pair = Some((*dragged, *idx));
                }
                if ui.small_button("✕").clicked() {
                    delete_action = Some(*idx);
                }
            });
        }
        if let Some(idx) = select_action {
            self.selected_action = Some(idx);
        }
        if let Some(idx) = delete_action {
            self.macro_result =
                Some(api::delete_macro_action(id, idx).map(|_| "Action deleted".to_string()));
            self.macro_dirty = true;
            self.action_edits.remove(&idx);
            if self.selected_action == Some(idx) {
                self.selected_action = None;
            }
        }
        if let Some((from_idx, to_idx)) = drop_pair {
            let mut reordered = full.clone();
            let from = reordered.iter().position(|(i, _)| *i == from_idx);
            let to = reordered.iter().position(|(i, _)| *i == to_idx);
            if let (Some(from), Some(to)) = (from, to) {
                if from != to {
                    let item = reordered.remove(from);
                    let to = if to > from { to - 1 } else { to };
                    reordered.insert(to, item);
                    let indices: Vec<i32> = full.iter().map(|(i, _)| *i).collect();
                    let ordered: Vec<api::MacroAction> =
                        reordered.iter().map(|(_, a)| a.clone()).collect();
                    self.macro_result = Some(
                        api::reorder_macro_actions(id, &indices, &ordered)
                            .map(|_| "Actions reordered".to_string()),
                    );
                    self.macro_dirty = true;
                }
            }
        }

        // Selected action editor.
        if let Some(aidx) = self.selected_action {
            ui.separator();
            ui.label(egui::RichText::new(format!("Action {aidx}")).strong());
            let (hold, rep, rdelay) = {
                let e = self.action_edits.entry(aidx).or_default();
                ui.checkbox(&mut e.hold, "Hold (press-and-hold)");
                ui.horizontal(|ui| {
                    ui.label("Repeat");
                    ui.add(egui::DragValue::new(&mut e.repeat).range(0..=255));
                    ui.label("times");
                });
                ui.horizontal(|ui| {
                    ui.label("Repeat delay");
                    ui.add(
                        egui::DragValue::new(&mut e.repeat_delay)
                            .suffix(" ms")
                            .range(0..=60000),
                    );
                });
                (e.hold, e.repeat, e.repeat_delay)
            };
            ui.horizontal(|ui| {
                if ui.button("Save action").clicked() {
                    self.macro_result = Some(
                        api::update_macro_action(id, aidx, hold, rep, rdelay)
                            .map(|_| "Action saved".to_string()),
                    );
                    self.macro_dirty = true;
                }
                if ui.button("Delete action").clicked() {
                    self.macro_result = Some(
                        api::delete_macro_action(id, aidx)
                            .map(|_| "Action deleted".to_string()),
                    );
                    self.macro_dirty = true;
                    self.action_edits.remove(&aidx);
                    self.selected_action = None;
                }
            });
        }

        ui.separator();

        // Add action form.
        ui.label(egui::RichText::new("Add action").strong());
        self.ensure_keys();
        ui.horizontal(|ui| {
            ui.label("Type");
            egui::ComboBox::from_id_salt("action-type")
                .selected_text(api::action_type_label(self.new_action_type))
                .show_ui(ui, |ui| {
                    for t in [
                        api::ACTION_KEYBOARD,
                        api::ACTION_MEDIA,
                        api::ACTION_DELAY,
                        api::ACTION_TEXT,
                        api::ACTION_MOUSE,
                        api::ACTION_MOUSE_POSITION,
                    ] {
                        ui.selectable_value(&mut self.new_action_type, t, api::action_type_label(t));
                    }
                });
        });
        command_editor(
            ui,
            self.new_action_type,
            &self.keys,
            &self.media,
            &self.mouse,
            &mut self.new_action_command,
            &mut self.new_action_text,
            &mut self.new_action_x,
            &mut self.new_action_y,
            &mut self.new_action_abs,
            &mut self.key_search,
            &mut self.capture_armed,
            "add",
        );
        if self.new_action_type == api::ACTION_TEXT {
            ui.checkbox(&mut self.new_action_enter, "send Enter after text");
        }
        if self.capture_armed {
            ui.label(
                egui::RichText::new("● Capturing… move cursor to target, then press F8")
                    .strong()
                    .color(egui::Color32::from_rgb(0xff, 0xb0, 0x20)),
            );
        }
        ui.horizontal(|ui| {
            if self.new_action_type == api::ACTION_DELAY {
                ui.label("Delay");
                ui.add(egui::DragValue::new(&mut self.new_action_delay).suffix(" ms"));
            }
            if ui.button("Add").clicked() {
                let ty = self.new_action_type;
                let cmd = self.new_action_command;
                let dly = self.new_action_delay;
                let text = self.new_action_text.clone();
                let x = self.new_action_x;
                let y = self.new_action_y;
                let abs = self.new_action_abs;
                let enter = self.new_action_enter;
                self.add_action(id, ty, cmd, dly, &text, x, y, abs, enter);
            }
        });
    }
}

impl eframe::App for RgbcueApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();

        // Esc cancels an armed capture.
        if self.capture_armed && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.capture_armed = false;
        }

        egui::Panel::left("devices")
            .exact_size(240.0)
            .show(ui, |ui| self.draw_sidebar(ui));

        egui::CentralPanel::default().show(ui, |ui| self.draw_main(ui));
    }
}

/// Draw a small filled colour swatch.
fn swatch(ui: &mut egui::Ui, r: u8, g: u8, b: u8) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 3.0, egui::Color32::from_rgb(r, g, b));
}

/// Render the type-specific "command" editor for one action: a searchable
/// key/media/mouse picker, a text field, or mouse-position X/Y + teleport.
/// `id` keeps combo-box ids unique between callers.
fn command_editor(
    ui: &mut egui::Ui,
    ty: u8,
    keys: &[(u16, String)],
    media: &[(u16, String)],
    mouse: &[(u16, String)],
    cmd: &mut u16,
    text: &mut String,
    x: &mut i32,
    y: &mut i32,
    abs: &mut bool,
    search: &mut String,
    capture_armed: &mut bool,
    id: &str,
) {
    match ty {
        api::ACTION_KEYBOARD => {
            let selected = keys
                .iter()
                .find(|(i, _)| *i == *cmd)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| "Select key…".to_string());
            egui::ComboBox::from_id_salt(format!("{id}-key"))
                .selected_text(selected)
                .width(200.0)
                .show_ui(ui, |ui| {
                    let search_resp = ui.add(
                        egui::TextEdit::singleline(search)
                            .hint_text("type a key…")
                            .desired_width(180.0),
                    );
                    search_resp.request_focus();
                    // Auto-select on an exact (case-insensitive) match: typing
                    // "m" selects "M" and clears the box for the next key.
                    let exact_query = search.trim().to_lowercase();
                    if let Some(i) = keys
                        .iter()
                        .find(|(_, n)| !exact_query.is_empty() && n.to_lowercase() == exact_query)
                        .map(|(i, _)| *i)
                    {
                        *cmd = i;
                        search.clear();
                    }
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for (i, n) in keys.iter().filter(|(_, n)| fuzzy_match(n, search)) {
                            ui.selectable_value(cmd, *i, n);
                        }
                    });
                });
        }
        api::ACTION_MEDIA => {
            let selected = media
                .iter()
                .find(|(i, _)| *i == *cmd)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| "Select media key…".to_string());
            egui::ComboBox::from_id_salt(format!("{id}-media"))
                .selected_text(selected)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (i, n) in media {
                        ui.selectable_value(cmd, *i, n);
                    }
                });
        }
        api::ACTION_MOUSE => {
            let selected = mouse
                .iter()
                .find(|(i, _)| *i == *cmd)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| "Select button…".to_string());
            egui::ComboBox::from_id_salt(format!("{id}-mouse"))
                .selected_text(selected)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (i, n) in mouse {
                        ui.selectable_value(cmd, *i, n);
                    }
                });
        }
        api::ACTION_TEXT => {
            ui.add(egui::TextEdit::singleline(text).hint_text("text").desired_width(200.0));
        }
        api::ACTION_MOUSE_POSITION => {
            ui.label("X");
            ui.add(egui::DragValue::new(x));
            ui.label("Y");
            ui.add(egui::DragValue::new(y));
            let btn_text = if *capture_armed { "Armed — press F8" } else { "Capture" };
            if ui.button(btn_text).clicked() {
                *capture_armed = !*capture_armed;
            }
            ui.checkbox(abs, "teleport (vs move)");
        }
        _ => {}
    }
}

/// Query the global cursor position via X11 (one-shot).
fn query_cursor() -> Option<(i32, i32)> {
    use x11_dl::xlib;
    unsafe {
        let Ok(xlib) = xlib::Xlib::open() else {
            return None;
        };
        let dpy = (xlib.XOpenDisplay)(std::ptr::null());
        if dpy.is_null() {
            return None;
        }
        let root = (xlib.XDefaultRootWindow)(dpy);
        let mut root_ret: u64 = 0;
        let mut child_ret: u64 = 0;
        let mut root_x: i32 = 0;
        let mut root_y: i32 = 0;
        let mut win_x: i32 = 0;
        let mut win_y: i32 = 0;
        let mut mask: u32 = 0;
        (xlib.XQueryPointer)(
            dpy,
            root,
            &mut root_ret,
            &mut child_ret,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        );
        (xlib.XCloseDisplay)(dpy);
        Some((root_x, root_y))
    }
}

/// Map a key name ("F8", "A", ...) to a Linux input keycode.
fn key_name_to_code(name: &str) -> Option<u16> {
    let n = name.trim().to_uppercase();
    Some(match n.as_str() {
        "F1" => 59, "F2" => 60, "F3" => 61, "F4" => 62, "F5" => 63, "F6" => 64,
        "F7" => 65, "F8" => 66, "F9" => 67, "F10" => 68, "F11" => 69, "F12" => 70,
        "ESC" | "ESCAPE" => 1, "ENTER" | "RETURN" => 28, "SPACE" => 57,
        "A" => 30, "B" => 48, "C" => 46, "D" => 32, "E" => 18, "F" => 33,
        "G" => 34, "H" => 35, "I" => 23, "J" => 36, "K" => 37, "L" => 38,
        "M" => 50, "N" => 49, "O" => 24, "P" => 25, "Q" => 16, "R" => 19,
        "S" => 31, "T" => 20, "U" => 22, "V" => 47, "W" => 17, "X" => 45,
        "Y" => 21, "Z" => 44,
        "0" => 11, "1" => 2, "2" => 3, "3" => 4, "4" => 5, "5" => 6, "6" => 7,
        "7" => 8, "8" => 9, "9" => 10,
        _ => return None,
    })
}

/// Find readable keyboard input devices.
fn find_keyboard_devices() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/class/input") {
        for e in entries.flatten() {
            let name =
                std::fs::read_to_string(e.path().join("device/name")).unwrap_or_default();
            if !name.to_lowercase().contains("keyboard") {
                continue;
            }
            if let Ok(devs) = std::fs::read_dir(e.path()) {
                for d in devs.flatten() {
                    let dn = d.file_name().to_string_lossy().to_string();
                    if dn.starts_with("event") {
                        out.push(format!("/dev/input/{dn}"));
                    }
                }
            }
        }
    }
    out
}

/// Spawn threads that watch the keyboard input devices (evdev) and signal each
/// time the configured key is pressed. This works regardless of window focus,
/// unlike an X11 key grab on Wayland. Returns the receiving channel.
fn spawn_f8_listener(key_code: u16) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    for dev in find_keyboard_devices() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut f = match std::fs::File::open(&dev) {
                Ok(f) => f,
                Err(_) => return,
            };
            let mut buf = [0u8; 24];
            loop {
                use std::io::Read;
                if f.read_exact(&mut buf).is_err() {
                    break;
                }
                let typ = u16::from_ne_bytes([buf[16], buf[17]]);
                let code = u16::from_ne_bytes([buf[18], buf[19]]);
                let value = i32::from_ne_bytes([buf[20], buf[21], buf[22], buf[23]]);
                if typ == 1 && code == key_code && value == 1 {
                    eprintln!("[capture] hotkey detected (evdev code {code})");
                    if tx.send(()).is_err() {
                        break;
                    }
                }
            }
        });
    }
    rx
}

/// Case-insensitive subsequence match ("nmpd5" matches "Numpad 5").
fn fuzzy_match(name: &str, query: &str) -> bool {
    let name = name.to_lowercase();
    let query = query.to_lowercase();
    let mut chars = name.chars();
    for q in query.chars() {
        if !chars.any(|c| c == q) {
            return false;
        }
    }
    true
}

/// Clamp an f64 colour channel (0..=255) to a u8.
fn clamp_u8(v: f64) -> u8 {
    v.clamp(0.0, 255.0).round() as u8
}

/// Spawn a thread that polls OpenLinkHub forever and ships results over a
/// channel. Returns the receiving end for the UI to drain each frame.
fn spawn_poller() -> mpsc::Receiver<Poll> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || loop {
        let result = api::fetch_devices();
        if tx.send(Poll::Devices(result)).is_err() {
            // The app dropped its receiver; nothing left to do.
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    });
    rx
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--list-devices") {
        list_devices();
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--rgb") {
        match args.get(i + 1) {
            Some(serial) => list_rgb(serial),
            None => {
                eprintln!("--rgb requires a device serial");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--macros") {
        list_macros();
        return Ok(());
    }
    if args.iter().any(|a| a == "--keys") {
        list_keys();
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--preview") {
        let page = args.get(i + 1).map(|s| s.as_str()).unwrap_or("dashboard");
        return ui::run(page);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1260.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RGBCUE",
        options,
        Box::new(|cc| Ok(Box::new(RgbcueApp::new(cc)))),
    )
}

/// Print the devices OpenLinkHub reports and exit — no display needed.
fn list_devices() {
    match api::fetch_devices() {
        Ok(devices) if devices.is_empty() => {
            println!("No devices found (is OpenLinkHub running on {})?", api::BASE);
        }
        Ok(devices) => {
            for d in devices {
                println!("{:26} {:<9} {}", d.product, d.kind(), d.serial);
            }
        }
        Err(e) => {
            eprintln!("Could not reach OpenLinkHub: {e}");
            std::process::exit(1);
        }
    }
}

/// Print a device's lighting profiles and exit — no display needed.
fn list_rgb(serial: &str) {
    match api::fetch_rgb_profiles(serial) {
        Ok(rgb) => {
            println!(
                "Device: {} (default rgb({:.0}, {:.0}, {:.0}))",
                rgb.device,
                rgb.default_color.red,
                rgb.default_color.green,
                rgb.default_color.blue
            );
            let mut names: Vec<&String> = rgb.profiles.keys().collect();
            names.sort();
            for key in names {
                let p = &rgb.profiles[key];
                let name = if p.name.is_empty() { key.as_str() } else { p.name.as_str() };
                println!(
                    "  {:<24} rgb({:.0}, {:.0}, {:.0})",
                    name, p.start.red, p.start.green, p.start.blue
                );
            }
        }
        Err(e) => {
            eprintln!("Could not fetch lighting: {e}");
            std::process::exit(1);
        }
    }
}

/// Print the macro library and exit — no display needed.
fn list_macros() {
    match api::fetch_macros() {
        Ok(macros) if macros.is_empty() => {
            println!("No macros defined.");
        }
        Ok(macros) => {
            for m in macros {
                println!(
                    "[{:2}] {}  repeat={} delay={}ms  ({} actions)",
                    m.id,
                    m.name,
                    m.repeat,
                    m.repeat_delay,
                    m.actions.len()
                );
                for (idx, a) in api::ordered_actions(&m) {
                    println!(
                        "      {:2}. {:<14} cmd={} delay={}ms hold={} repeat={}",
                        idx,
                        api::action_type_label(a.action_type),
                        a.action_command,
                        a.action_delay,
                        a.action_hold,
                        a.action_repeat
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Could not fetch macros: {e}");
            std::process::exit(1);
        }
    }
}

/// Print the key/button tables and exit — no display needed.
fn list_keys() {
    for (label, res) in [
        ("Keyboard", api::fetch_keyboard_keys()),
        ("Media", api::fetch_media_keys()),
        ("Mouse", api::fetch_mouse_buttons()),
    ] {
        match res {
            Ok(keys) => {
                println!("{label}: {} entries", keys.len());
                for (idx, k) in keys.iter().take(8) {
                    println!("  idx={idx}  name={}  cmd={}", k.name, k.command_code);
                }
            }
            Err(e) => eprintln!("{label}: error {e}"),
        }
    }
}
