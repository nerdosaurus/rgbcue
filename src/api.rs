//! Thin client for OpenLinkHub's local REST API.
//!
//! OpenLinkHub runs as a system service and exposes a JSON API on
//! `http://127.0.0.1:27003`. This module only talks to that API — all of the
//! device-protocol work (HID report formatting per device) lives in
//! OpenLinkHub, so this app is purely a frontend.
//!
//! Every response is wrapped in an envelope: `{ "code": 200, "status": 0,
//! <payload> }`. The payload field differs per endpoint (`devices`, `data`, ...).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

pub const BASE: &str = "http://127.0.0.1:27003";
const TIMEOUT: Duration = Duration::from_secs(3);

/// One Corsair device as reported by `GET /api/devices/`.
///
/// The list endpoint is intentionally sparse: `GetDevicesEx` in OpenLinkHub
/// fills in the name/serial/hidden flag and leaves the rest zeroed. Richer
/// per-device data (LED map, key map, DPI, firmware, ...) comes from the
/// per-device endpoints, added in later milestones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    #[serde(rename = "Product", default)]
    pub product: String,
    #[serde(rename = "Serial", default)]
    pub serial: String,
    #[serde(rename = "Firmware", default)]
    pub firmware: String,
    #[serde(rename = "Image", default)]
    pub image: String,
    #[serde(rename = "Hidden", default)]
    pub hidden: bool,
    #[serde(rename = "DeviceType", default)]
    pub device_type: u32,
    #[serde(rename = "ProductId", default)]
    pub product_id: u16,
    #[serde(rename = "DevicePath", default)]
    pub device_path: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Keyboard,
    Mouse,
    Cooling,
    Receiver,
    Other,
}

/// Broad device kind used by the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
    Commander,
    Pump,
    Other,
}

impl Device {
    fn category(&self) -> Category {
        let n = self.product.to_uppercase();
        if n.contains("KEYBOARD")
            || n.contains("STRAFE")
            || n.contains("K63")
            || n.contains("K65")
            || n.contains("K66")
            || n.contains("K68")
            || n.contains("K70")
            || n.contains("K95")
            || n.contains("K100")
            || n.contains("K55")
            || n.contains("K60")
            || n.contains("VANGUARD")
            || n.contains("MAKR")
            || n.contains("CLIPPER")
            || n.contains("GALLEON")
        {
            Category::Keyboard
        } else if n.contains("MOUSE")
            || n.contains("SCIMITAR")
            || n.contains("DARKSTAR")
            || n.contains("HARPOON")
            || n.contains("GLAIVE")
            || n.contains("IRONCLAW")
            || n.contains("NIGHTSWORD")
            || n.contains("NIGHTSABRE")
            || n.contains("SABRE")
            || n.contains("KATAR")
            || n.contains("M55")
            || n.contains("M65")
            || n.contains("M75")
        {
            Category::Mouse
        } else if n.contains("COOLER")
            || n.contains("H1")
            || n.contains("H100")
            || n.contains("H115")
            || n.contains("H150")
            || n.contains("H170")
            || n.contains("COMMANDER")
            || n.contains("PUMP")
            || n.contains("XC")
            || n.contains("XD")
        {
            Category::Cooling
        } else if n.contains("RECEIVER") || n.contains("SLIPSTREAM") || n.contains("DONGLE") {
            Category::Receiver
        } else {
            Category::Other
        }
    }

    /// Best-effort device category, inferred from the product name. The list
    /// endpoint doesn't populate `DeviceType`, so we guess until per-device
    /// detail is wired up.
    pub fn kind(&self) -> &'static str {
        match self.category() {
            Category::Keyboard => "Keyboard",
            Category::Mouse => "Mouse",
            Category::Cooling => "Cooling",
            Category::Receiver => "Receiver",
            Category::Other => "Device",
        }
    }

    /// Dashboard-level kind, splitting fan controllers from liquid coolers.
    pub fn kind_enum(&self) -> DeviceKind {
        let n = self.product.to_uppercase();
        match self.category() {
            Category::Keyboard => DeviceKind::Keyboard,
            Category::Mouse => DeviceKind::Mouse,
            Category::Cooling => {
                if n.contains("COMMANDER")
                    || n.contains("CORE")
                    || n.contains("NODE")
                    || n.contains("HUB")
                {
                    DeviceKind::Commander
                } else {
                    DeviceKind::Pump
                }
            }
            _ => DeviceKind::Other,
        }
    }
}

/// Envelope for `GET /api/devices/`.
#[derive(Debug, Deserialize)]
struct DevicesResponse {
    #[allow(dead_code)]
    code: i32,
    #[allow(dead_code)]
    status: i32,
    devices: HashMap<String, Device>,
}

/// Fetch the connected (non-hidden) devices, sorted by product name.
pub fn fetch_devices() -> Result<Vec<Device>, String> {
    let body = get(&format!("{BASE}/api/devices/"))?;
    let resp: DevicesResponse = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    let mut devices: Vec<Device> = resp.devices.into_values().filter(|d| !d.hidden).collect();
    devices.sort_by(|a, b| a.product.cmp(&b.product));
    Ok(devices)
}

/// Perform a GET and return the raw response body as a string.
fn get(url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("request failed: {e}"))?;
    resp.into_string().map_err(|e| format!("read failed: {e}"))
}

fn post_body(url: &str, payload: &serde_json::Value) -> Result<String, String> {
    ureq::post(url)
        .timeout(TIMEOUT)
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .map_err(|e| format!("request failed: {e}"))?
        .into_string()
        .map_err(|e| format!("read failed: {e}"))
}

// -- dashboard --------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MonitorTone {
    Warn,
    Good,
    Text,
}

#[derive(Debug, Clone)]
pub struct MonitorStatus {
    pub id: String,
    pub title: String,
    pub value: String,
    pub detail: String,
    pub tone: MonitorTone,
}

#[derive(Debug, Clone)]
pub struct KeyCap {
    pub name: String,
    pub sub: String,
    pub span: f32,
    pub color: [u8; 3],
    pub index: i32,
    #[allow(dead_code)]
    pub row: i32,
    pub action_type: u8,
    pub action_command: u16,
    pub action_hold: bool,
    pub default: bool,
    #[allow(dead_code)]
    pub retain_original: bool,
    pub modifier_key: i32,
    pub toggle_delay: u16,
}

#[derive(Debug, Clone)]
pub struct KeyRow {
    pub keys: Vec<KeyCap>,
}

#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    pub rows: Vec<KeyRow>,
}

/// One DPI sensitivity stage, with the colour assigned to it.
#[derive(Debug, Clone, Default)]
pub struct DpiStage {
    pub name: String,
    pub value: u16,
    pub color: [u8; 3],
}

/// One RGB zone on a mouse, with its current colour.
#[derive(Debug, Clone, Default)]
pub struct ZoneColor {
    pub name: String,
    pub color: [u8; 3],
}

/// One remappable mouse button.
#[derive(Debug, Clone, Default)]
pub struct MouseButton {
    pub index: i32,
    pub name: String,
    pub default: bool,
    pub action_hold: bool,
    #[allow(dead_code)]
    pub on_release: bool,
    pub action_type: u8,
    pub action_command: u16,
    pub modifier_key: i32,
    pub retain_original: bool,
    pub toggle_delay: u16,
}

/// One checkbox in the keyboard performance modal.
#[derive(Debug, Clone, Default)]
pub struct PerformanceItem {
    pub name: String,
    pub internal: String,
    pub value: bool,
}

/// One key slot in the FlashTap configuration.
#[derive(Debug, Clone, Default)]
pub struct FlashTapKey {
    pub index: usize,
    pub name: String,
    pub key_data: Option<i64>,
}

/// FlashTap configuration for a keyboard.
#[derive(Debug, Clone, Default)]
pub struct FlashTapData {
    pub active: bool,
    pub mode: i64,
    pub modes: Vec<(i64, String)>,
    pub color: [u8; 3],
    pub keys: Vec<FlashTapKey>,
}

#[derive(Debug, Clone)]
pub struct DeviceStatus {
    pub product: String,
    pub serial: String,
    pub firmware: String,
    pub connected: bool,
    pub battery: Option<u8>,
    pub kind: DeviceKind,
    pub stat_value: String,
    pub stat_label: String,
    pub brightness: u8,
    #[allow(dead_code)]
    pub dpi: Option<u16>,
    pub profiles: Vec<String>,
    pub active_profile: String,
    pub rgb_modes: Vec<String>,
    pub active_rgb: String,
    pub layouts: Vec<String>,
    pub active_layout: String,
    pub polling_rates: Vec<(i64, String)>,
    pub active_polling: i64,
    pub cluster: bool,
    pub keyboard: Option<KeyboardLayout>,
    pub keyboard_profiles: Vec<String>,
    pub active_keyboard_profile: String,
    pub assignment_types: Vec<(i32, String)>,
    pub dpi_stages: Vec<DpiStage>,
    pub min_dpi: u16,
    pub max_dpi: u16,
    pub zones: Vec<ZoneColor>,
    pub mouse_buttons: Vec<MouseButton>,
    pub sleep_modes: Vec<(i64, String)>,
    pub active_sleep: i64,
    pub angle_snapping: bool,
    pub button_optimization: bool,
    pub lift_heights: Vec<(i64, String)>,
    pub active_lift: i64,
    pub open_rgb: bool,
    pub usb: bool,
}

/// One snapshot of the dashboard: device cards and monitored values.
#[derive(Debug, Clone, Default)]
pub struct Dashboard {
    pub devices: Vec<DeviceStatus>,
    pub dashboard: Vec<String>,
    pub monitors: Vec<MonitorStatus>,
}

/// OpenLinkHub's global dashboard settings.
#[derive(Debug, Clone, Default)]
pub struct DashboardSettings {
    pub celsius: bool,
    pub temperature_bar: bool,
    pub add_device_to_dashboard: bool,
    pub show_labels: bool,
    pub rgb_off: bool,
    pub language_code: String,
    pub theme: String,
    pub themes: Vec<String>,
    pub keyboard_layout: i64,
    pub keyboard_layouts: Vec<(i64, String)>,
    pub sidebar_collapsed: bool,
}

#[derive(Debug, Deserialize, Default)]
struct DeviceDetail {
    #[serde(default)]
    firmware: String,
    #[serde(default, rename = "DeviceProfile")]
    profile: Option<DeviceProfile>,
    #[serde(default, rename = "PollingRates")]
    polling_rates: HashMap<String, String>,
    #[serde(default)]
    devices: Option<serde_json::Value>,
    #[serde(default, rename = "UserProfiles")]
    user_profiles: HashMap<String, UserProfile>,
    #[serde(default, rename = "RGBModes")]
    rgb_modes: Vec<String>,
    #[serde(default, rename = "Layouts")]
    layouts: Vec<String>,
    #[serde(default, rename = "KeyAssignmentTypes")]
    assignment_types: HashMap<String, String>,
    #[serde(default, rename = "KeyAssignment")]
    key_assignment: Option<serde_json::Value>,
    #[serde(default, rename = "MinDPI")]
    min_dpi: Option<u16>,
    #[serde(default, rename = "MaxDPI")]
    max_dpi: Option<u16>,
    #[serde(default, rename = "SleepModes")]
    sleep_modes: HashMap<String, String>,
    #[serde(default, rename = "LiftHeights")]
    lift_heights: HashMap<String, String>,
    #[serde(default, rename = "Usb")]
    usb: Option<bool>,
    #[serde(default, rename = "Connected")]
    connected_cap: Option<bool>,
    #[serde(default)]
    connected: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct UserProfile {
    #[serde(default, rename = "Active")]
    active: bool,
}

#[derive(Debug, Deserialize, Default)]
struct DeviceProfile {
    #[serde(default, rename = "BrightnessSlider")]
    brightness_slider: Option<u8>,
    #[serde(default, rename = "Brightness")]
    brightness: Option<u8>,
    #[serde(default, rename = "RGBProfile")]
    rgb_profile: Option<String>,
    #[serde(default, rename = "PollingRate")]
    polling_rate: Option<i64>,
    #[serde(default, rename = "Profile")]
    profile: Option<serde_json::Value>,
    #[serde(default, rename = "Profiles")]
    dpi_profiles: Option<serde_json::Value>,
    #[serde(default, rename = "ZoneColors")]
    zone_colors: Option<serde_json::Value>,
    #[serde(default, rename = "Label")]
    label: Option<String>,
    #[serde(default, rename = "Layout")]
    layout: Option<String>,
    #[serde(default, rename = "RGBCluster")]
    rgb_cluster: Option<bool>,
    #[serde(default, rename = "SleepMode")]
    sleep_mode: Option<i64>,
    #[serde(default, rename = "AngleSnapping")]
    angle_snapping: Option<i64>,
    #[serde(default, rename = "ButtonOptimization")]
    button_optimization: Option<i64>,
    #[serde(default, rename = "LiftHeight")]
    lift_height: Option<i64>,
    #[serde(default, rename = "OpenRGBIntegration")]
    open_rgb: Option<bool>,
    #[serde(default, rename = "Keyboards")]
    keyboards: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
struct Channel {
    #[serde(default, rename = "name")]
    _name: String,
    #[serde(default)]
    rpm: Option<f64>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default, rename = "HasSpeed")]
    has_speed: bool,
}

#[derive(Debug, Deserialize, Default)]
struct BatteryStat {
    #[serde(default, rename = "Level")]
    level: u16,
    #[serde(default, rename = "Connected")]
    connected: Option<bool>,
}

impl DeviceDetail {
    fn connected(&self) -> Option<bool> {
        self.connected_cap.or(self.connected)
    }

    fn channels(&self) -> Vec<Channel> {
        let Some(value) = &self.devices else {
            return Vec::new();
        };
        let Some(map) = value.as_object() else {
            return Vec::new();
        };
        map.values()
            .filter(|v| v.is_object())
            .filter_map(|v| serde_json::from_value::<Channel>(v.clone()).ok())
            .collect()
    }

    fn polling(&self) -> Option<String> {
        let idx = self.profile.as_ref()?.polling_rate?;
        self.polling_rates.get(&idx.to_string()).cloned()
    }

    fn dpi(&self) -> Option<u16> {
        let p = self.profile.as_ref()?;
        let idx = p.profile.as_ref()?.as_i64()?;
        let entries = p.dpi_profiles.as_ref()?.as_object()?;
        let entry = entries.get(&idx.to_string())?;
        entry.get("Value")?.as_u64().map(|v| v as u16)
    }

    fn dpi_name(&self, idx: i64) -> Option<String> {
        let p = self.profile.as_ref()?;
        let entries = p.dpi_profiles.as_ref()?.as_object()?;
        let entry = entries.get(&idx.to_string())?;
        // OpenLinkHub tags the Go field `Name` as `name`, so live payloads use
        // the lowercase key while older/synthetic payloads use `Name`.
        entry
            .get("name")
            .or_else(|| entry.get("Name"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }

    fn dpi_stages(&self) -> Vec<DpiStage> {
        let Some(entries) = self
            .profile
            .as_ref()
            .and_then(|p| p.dpi_profiles.as_ref())
            .and_then(|v| v.as_object())
        else {
            return Vec::new();
        };
        let mut stages: Vec<(i64, DpiStage)> = entries
            .iter()
            .filter_map(|(key, entry)| {
                let idx = key.parse::<i64>().ok()?;
                let name = entry
                    .get("name")
                    .or_else(|| entry.get("Name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Stage")
                    .to_string();
                let value = entry.get("Value").and_then(|v| v.as_u64())? as u16;
                let color = entry
                    .get("Color")
                    .and_then(json_color)
                    .unwrap_or([255, 255, 255]);
                Some((idx, DpiStage { name, value, color }))
            })
            .collect();
        stages.sort_by_key(|(idx, _)| *idx);
        stages.into_iter().map(|(_, stage)| stage).collect()
    }

    fn zone_colors(&self) -> Vec<ZoneColor> {
        let Some(entries) = self
            .profile
            .as_ref()
            .and_then(|p| p.zone_colors.as_ref())
            .and_then(|v| v.as_object())
        else {
            return Vec::new();
        };
        let mut zones: Vec<(i64, ZoneColor)> = entries
            .iter()
            .filter_map(|(key, entry)| {
                let idx = key.parse::<i64>().ok()?;
                let name = entry
                    .get("Name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Zone")
                    .to_string();
                let color = entry
                    .get("Color")
                    .and_then(json_color)
                    .unwrap_or([255, 255, 255]);
                Some((idx, ZoneColor { name, color }))
            })
            .collect();
        zones.sort_by_key(|(idx, _)| *idx);
        zones.into_iter().map(|(_, zone)| zone).collect()
    }

    fn dpi_range(&self) -> (u16, u16) {
        let min = self.min_dpi.unwrap_or(100);
        let max = self.max_dpi.unwrap_or(18000).max(min);
        (min, max)
    }

    fn mouse_buttons(&self) -> Vec<MouseButton> {
        let Some(entries) = self.key_assignment.as_ref().and_then(|v| v.as_object()) else {
            return Vec::new();
        };
        let flag = |entry: &serde_json::Value, key: &str| {
            entry.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
        };
        let mut buttons: Vec<(i64, MouseButton)> = entries
            .iter()
            .filter_map(|(key, entry)| {
                let index = key.parse::<i64>().ok()?;
                let name = entry.get("name").and_then(|v| v.as_str())?;
                Some((
                    index,
                    MouseButton {
                        index: index as i32,
                        name: name.to_string(),
                        default: flag(entry, "default"),
                        action_hold: flag(entry, "actionHold"),
                        on_release: flag(entry, "onRelease"),
                        action_type: entry
                            .get("actionType")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u8,
                        action_command: entry
                            .get("actionCommand")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u16,
                        modifier_key: entry
                            .get("modifierKey")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0) as i32,
                        retain_original: flag(entry, "retainOriginal"),
                        toggle_delay: entry
                            .get("toggleDelay")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u16,
                    },
                ))
            })
            .collect();
        buttons.sort_by_key(|(index, _)| *index);
        buttons.into_iter().map(|(_, button)| button).collect()
    }

    fn sorted_modes(map: &HashMap<String, String>) -> Vec<(i64, String)> {
        let mut items: Vec<(i64, String)> = map
            .iter()
            .filter_map(|(k, v)| k.parse::<i64>().ok().map(|n| (n, v.clone())))
            .collect();
        items.sort_by_key(|(n, _)| *n);
        items
    }

    fn sleep_modes(&self) -> Vec<(i64, String)> {
        Self::sorted_modes(&self.sleep_modes)
    }

    fn lift_heights(&self) -> Vec<(i64, String)> {
        Self::sorted_modes(&self.lift_heights)
    }

    fn sleep_mode(&self) -> i64 {
        self.profile
            .as_ref()
            .and_then(|p| p.sleep_mode)
            .unwrap_or(0)
    }

    fn lift_height(&self) -> i64 {
        self.profile
            .as_ref()
            .and_then(|p| p.lift_height)
            .unwrap_or(0)
    }

    fn angle_snapping(&self) -> bool {
        self.profile
            .as_ref()
            .and_then(|p| p.angle_snapping)
            .unwrap_or(0)
            != 0
    }

    fn button_optimization(&self) -> bool {
        self.profile
            .as_ref()
            .and_then(|p| p.button_optimization)
            .unwrap_or(0)
            != 0
    }

    fn open_rgb(&self) -> bool {
        self.profile
            .as_ref()
            .and_then(|p| p.open_rgb)
            .unwrap_or(false)
    }

    fn profile_name(&self) -> String {
        let Some(p) = self.profile.as_ref() else {
            return "Default".into();
        };
        if let Some(v) = p.profile.as_ref() {
            if let Some(s) = v.as_str() {
                return s.to_string();
            }
            if let Some(idx) = v.as_i64() {
                if let Some(name) = self.dpi_name(idx) {
                    return name;
                }
            }
        }
        p.label.clone().unwrap_or_else(|| "Default".into())
    }

    fn brightness(&self) -> u8 {
        self.profile
            .as_ref()
            .and_then(|p| p.brightness_slider.or(p.brightness))
            .unwrap_or(80)
    }

    fn rgb_profile(&self) -> String {
        self.profile
            .as_ref()
            .and_then(|p| p.rgb_profile.clone())
            .unwrap_or_else(|| "Static".into())
    }

    fn profiles(&self) -> Vec<String> {
        let mut names: Vec<String> = self.user_profiles.keys().cloned().collect();
        names.sort();
        names
    }

    fn active_profile(&self) -> Option<String> {
        if let Some((name, _)) = self.user_profiles.iter().find(|(_, p)| p.active) {
            return Some(name.clone());
        }
        self.profile
            .as_ref()
            .and_then(|p| p.profile.as_ref())
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }

    fn layout(&self) -> Option<String> {
        self.profile.as_ref().and_then(|p| p.layout.clone())
    }

    fn cluster(&self) -> bool {
        self.profile
            .as_ref()
            .and_then(|p| p.rgb_cluster)
            .unwrap_or(false)
    }

    fn assignment_types(&self) -> Vec<(i32, String)> {
        let mut items: Vec<(i32, String)> = self
            .assignment_types
            .iter()
            .filter_map(|(k, v)| k.parse::<i32>().ok().map(|n| (n, v.clone())))
            .collect();
        items.sort_by_key(|(n, _)| *n);
        items
    }

    fn keyboard_profiles(&self) -> Vec<String> {
        self.profile
            .as_ref()
            .and_then(|p| p.dpi_profiles.as_ref())
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn active_keyboard_profile(&self) -> Option<String> {
        self.profile
            .as_ref()
            .and_then(|p| p.profile.as_ref())
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }

    fn keyboard_layout(&self) -> Option<KeyboardLayout> {
        let p = self.profile.as_ref()?;
        let keyboards = p.keyboards.as_ref()?.as_object()?;
        let active = p
            .profile
            .as_ref()
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let keyboard = keyboards
            .get(active)
            .or_else(|| keyboards.values().next())?;
        let rows = keyboard.get("row")?.as_object()?;

        let mut ordered: Vec<(i64, &serde_json::Value)> = rows
            .iter()
            .filter_map(|(k, v)| k.parse::<i64>().ok().map(|n| (n, v)))
            .collect();
        ordered.sort_by_key(|(n, _)| *n);

        let mut out = Vec::new();
        for (ri, row) in ordered {
            let Some(keys) = row.get("keys").and_then(|v| v.as_object()) else {
                continue;
            };
            let mut caps: Vec<(i64, &serde_json::Value)> = keys
                .iter()
                .filter_map(|(k, v)| k.parse::<i64>().ok().map(|n| (n, v)))
                .collect();
            caps.sort_by_key(|(n, _)| *n);

            let mut row_caps = Vec::new();
            for (ki, key) in caps {
                let name = key
                    .get("keyName")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                let width = key.get("width").and_then(|v| v.as_u64()).unwrap_or(70) as f32 / 70.0;
                let css = format!(
                    "{} {}",
                    key.get("css").and_then(|v| v.as_str()).unwrap_or_default(),
                    key.get("extraCss")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default(),
                );
                let span = css_span(&css, width);
                let color = key.get("color");
                let channel = |name: &str| {
                    color
                        .and_then(|c| c.get(name))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(255) as u8
                };
                row_caps.push(KeyCap {
                    name: name.to_string(),
                    sub: key
                        .get("subKeyName")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    span,
                    color: [channel("red"), channel("green"), channel("blue")],
                    index: ki as i32,
                    row: ri as i32,
                    action_type: key.get("actionType").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                    action_command: key
                        .get("actionCommand")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u16,
                    action_hold: key
                        .get("actionHold")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    default: key.get("default").and_then(|v| v.as_bool()).unwrap_or(true),
                    retain_original: key
                        .get("retainOriginal")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    modifier_key: key.get("modifierKey").and_then(|v| v.as_i64()).unwrap_or(0)
                        as i32,
                    toggle_delay: key.get("toggleDelay").and_then(|v| v.as_u64()).unwrap_or(0)
                        as u16,
                });
            }
            if !row_caps.is_empty() {
                out.push(KeyRow { keys: row_caps });
            }
        }

        (!out.is_empty()).then_some(KeyboardLayout { rows: out })
    }
}

#[derive(Debug, Deserialize)]
struct DeviceEnvelope {
    #[serde(default)]
    device: Option<DeviceDetail>,
}

#[derive(Debug, Deserialize)]
struct StringData {
    #[serde(default)]
    data: String,
}

#[derive(Debug, Deserialize, Default)]
struct BatteryEnvelope {
    #[serde(default)]
    data: HashMap<String, BatteryStat>,
}

/// Key width in grid columns: CSS `wide` classes win, `width` is a fallback.
fn css_span(css: &str, width_units: f32) -> f32 {
    for token in css.split_whitespace() {
        if let Some(rest) = token.strip_prefix("wide") {
            if rest.is_empty() {
                return 2.0;
            }
            if let Ok(n) = rest.parse::<f32>() {
                return n.max(1.0);
            }
        }
    }
    width_units.round().max(1.0)
}

fn fetch_device_detail(serial: &str) -> Result<DeviceDetail, String> {
    let body = get(&format!("{BASE}/api/devices/{serial}"))?;
    let env: DeviceEnvelope = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    env.device.ok_or_else(|| "no device payload".to_string())
}

#[derive(Debug, Deserialize, Default)]
struct StorageTemp {
    #[serde(default, rename = "Key")]
    key: String,
    #[serde(default, rename = "Model")]
    model: String,
    #[serde(default, rename = "Temperature")]
    temperature: f64,
    #[serde(default, rename = "TemperatureString")]
    temperature_string: String,
}

#[derive(Debug, Deserialize, Default)]
struct StorageEnvelope {
    #[serde(default)]
    data: Vec<StorageTemp>,
}

fn fetch_storage_temps() -> Vec<StorageTemp> {
    get(&format!("{BASE}/api/storageTemp"))
        .ok()
        .and_then(|body| serde_json::from_str::<StorageEnvelope>(&body).ok())
        .map(|env| env.data)
        .unwrap_or_default()
}

fn fetch_temperature(path: &str) -> Option<String> {
    let body = get(&format!("{BASE}{path}")).ok()?;
    let parsed: StringData = serde_json::from_str(&body).ok()?;
    let value = parsed.data.trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[derive(Debug, Deserialize, Default)]
struct DashboardDevices {
    #[serde(default)]
    devices: Vec<String>,
}

fn fetch_dashboard_devices() -> Vec<String> {
    get(&format!("{BASE}/api/dashboard/devices/get"))
        .ok()
        .and_then(|body| serde_json::from_str::<DashboardDevices>(&body).ok())
        .map(|env| env.devices)
        .unwrap_or_default()
}

fn fetch_batteries() -> HashMap<String, BatteryStat> {
    get(&format!("{BASE}/api/batteryStats"))
        .ok()
        .and_then(|body| serde_json::from_str::<BatteryEnvelope>(&body).ok())
        .map(|env| env.data)
        .unwrap_or_default()
}

/// Fetch one live dashboard snapshot: device cards plus monitored values.
pub fn fetch_dashboard() -> Result<Dashboard, String> {
    let devices = fetch_devices()?;
    let batteries = fetch_batteries();

    let mut statuses = Vec::with_capacity(devices.len());

    for dev in &devices {
        let detail = fetch_device_detail(&dev.serial).ok();
        let kind = dev.kind_enum();
        let channels = detail
            .as_ref()
            .map(DeviceDetail::channels)
            .unwrap_or_default();
        let battery = batteries.get(&dev.serial);
        let connected = detail
            .as_ref()
            .and_then(DeviceDetail::connected)
            .or_else(|| battery.and_then(|b| b.connected))
            .unwrap_or(true);

        let total: u64 = channels
            .iter()
            .filter(|c| c.has_speed || c.rpm.unwrap_or(0.0) > 0.0)
            .map(|c| c.rpm.unwrap_or(0.0).max(0.0) as u64)
            .sum();
        let polling = detail.as_ref().and_then(DeviceDetail::polling);
        let dpi = detail.as_ref().and_then(DeviceDetail::dpi);
        let profile = detail
            .as_ref()
            .map(DeviceDetail::profile_name)
            .unwrap_or_else(|| "Default".into());
        let brightness = detail.as_ref().map(DeviceDetail::brightness).unwrap_or(80);
        let rgb_profile = detail
            .as_ref()
            .map(DeviceDetail::rgb_profile)
            .unwrap_or_else(|| "Static".into());

        let max_temp = channels
            .iter()
            .filter_map(|c| c.temperature)
            .fold(0.0_f64, f64::max);

        let (stat_value, stat_label) = match kind {
            DeviceKind::Pump => {
                if max_temp > 0.0 {
                    (format!("{max_temp:.1} °C"), "Coolant".to_string())
                } else {
                    ("Connected".into(), "Status".into())
                }
            }
            DeviceKind::Commander => {
                if total > 0 {
                    (total.to_string(), "Fan RPM".into())
                } else {
                    ("Connected".into(), "Status".into())
                }
            }
            DeviceKind::Keyboard => match &polling {
                Some(p) => (p.clone(), "Polling rate".into()),
                None => (profile.clone(), "Profile".into()),
            },
            DeviceKind::Mouse => {
                if let Some(dpi) = dpi {
                    ((dpi as u64).to_string(), "DPI".into())
                } else if let Some(b) = battery {
                    (format!("{}%", b.level), "Battery".into())
                } else {
                    ("Connected".into(), "Status".into())
                }
            }
            DeviceKind::Other => ("Connected".into(), "Status".into()),
        };

        let profiles = {
            let names = detail
                .as_ref()
                .map(DeviceDetail::profiles)
                .unwrap_or_default();
            if names.is_empty() {
                vec!["Default".to_string()]
            } else {
                names
            }
        };
        let active_profile = detail
            .as_ref()
            .and_then(DeviceDetail::active_profile)
            .unwrap_or_else(|| profiles[0].clone());
        let mut rgb_modes = detail
            .as_ref()
            .map(|d| d.rgb_modes.clone())
            .unwrap_or_default();
        if rgb_modes.is_empty() {
            rgb_modes.push(rgb_profile.clone());
        }
        let mut layouts = detail
            .as_ref()
            .map(|d| d.layouts.clone())
            .unwrap_or_default();
        let active_layout = detail
            .as_ref()
            .and_then(DeviceDetail::layout)
            .or_else(|| layouts.first().cloned())
            .unwrap_or_default();
        if layouts.is_empty() {
            layouts.push(active_layout.clone());
        }
        let mut polling_rates: Vec<(i64, String)> = detail
            .as_ref()
            .map(|d| {
                d.polling_rates
                    .iter()
                    .filter_map(|(k, v)| k.parse::<i64>().ok().map(|n| (n, v.clone())))
                    .collect()
            })
            .unwrap_or_default();
        polling_rates.sort_by_key(|(n, _)| *n);
        if polling_rates.is_empty() {
            polling_rates.push((0, "Default".into()));
        }
        let active_polling = detail
            .as_ref()
            .and_then(|d| d.profile.as_ref().and_then(|p| p.polling_rate))
            .unwrap_or(polling_rates[0].0);
        let cluster = detail.as_ref().map(DeviceDetail::cluster).unwrap_or(false);
        let keyboard = detail.as_ref().and_then(DeviceDetail::keyboard_layout);
        let keyboard_profiles = detail
            .as_ref()
            .map(DeviceDetail::keyboard_profiles)
            .unwrap_or_default();
        let active_keyboard_profile = detail
            .as_ref()
            .and_then(DeviceDetail::active_keyboard_profile)
            .unwrap_or_default();
        let assignment_types = detail
            .as_ref()
            .map(DeviceDetail::assignment_types)
            .unwrap_or_default();
        let dpi_stages = detail
            .as_ref()
            .map(DeviceDetail::dpi_stages)
            .unwrap_or_default();
        let (min_dpi, max_dpi) = detail
            .as_ref()
            .map(DeviceDetail::dpi_range)
            .unwrap_or((100, 18000));
        let zones = detail
            .as_ref()
            .map(DeviceDetail::zone_colors)
            .unwrap_or_default();
        let mouse_buttons = if kind == DeviceKind::Mouse {
            detail
                .as_ref()
                .map(DeviceDetail::mouse_buttons)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let sleep_modes = detail
            .as_ref()
            .map(DeviceDetail::sleep_modes)
            .unwrap_or_default();
        let active_sleep = detail.as_ref().map(DeviceDetail::sleep_mode).unwrap_or(0);
        let angle_snapping = detail
            .as_ref()
            .map(DeviceDetail::angle_snapping)
            .unwrap_or(false);
        let button_optimization = detail
            .as_ref()
            .map(DeviceDetail::button_optimization)
            .unwrap_or(false);
        let lift_heights = detail
            .as_ref()
            .map(DeviceDetail::lift_heights)
            .unwrap_or_default();
        let active_lift = detail.as_ref().map(DeviceDetail::lift_height).unwrap_or(0);
        let open_rgb = detail.as_ref().map(DeviceDetail::open_rgb).unwrap_or(false);
        let usb = detail.as_ref().and_then(|d| d.usb).unwrap_or(true);

        statuses.push(DeviceStatus {
            product: dev.product.clone(),
            serial: dev.serial.clone(),
            firmware: detail
                .as_ref()
                .map(|d| d.firmware.clone())
                .filter(|f| !f.is_empty())
                .unwrap_or_else(|| dev.firmware.clone()),
            connected,
            battery: battery.map(|b| b.level.min(100) as u8),
            kind,
            stat_value,
            stat_label,
            brightness,
            dpi,
            profiles,
            active_profile,
            rgb_modes,
            active_rgb: rgb_profile.clone(),
            layouts,
            active_layout,
            polling_rates,
            active_polling,
            cluster,
            keyboard,
            keyboard_profiles,
            active_keyboard_profile,
            assignment_types,
            dpi_stages,
            min_dpi,
            max_dpi,
            zones,
            mouse_buttons,
            sleep_modes,
            active_sleep,
            angle_snapping,
            button_optimization,
            lift_heights,
            active_lift,
            open_rgb,
            usb,
        });
    }

    let dashboard = fetch_dashboard_devices();

    let mut monitors = Vec::new();
    if let Some(value) = fetch_temperature("/api/cpuTemp") {
        monitors.push(MonitorStatus {
            id: "cpu".into(),
            title: "CPU temperature".into(),
            value,
            detail: cpu_model().unwrap_or_else(|| "Processor".into()),
            tone: MonitorTone::Warn,
        });
    }
    if let Some(value) = fetch_temperature("/api/gpuTemp") {
        monitors.push(MonitorStatus {
            id: "gpu".into(),
            title: "GPU temperature".into(),
            value,
            detail: gpu_model().unwrap_or_else(|| "Graphics".into()),
            tone: MonitorTone::Warn,
        });
    }
    for storage in fetch_storage_temps() {
        let value = if !storage.temperature_string.trim().is_empty() {
            storage.temperature_string
        } else if storage.temperature > 0.0 {
            format!("{:.1} °C", storage.temperature)
        } else {
            continue;
        };
        let id = if storage.key.is_empty() {
            format!("storage:{}", storage.model)
        } else {
            format!("storage:{}", storage.key)
        };
        monitors.push(MonitorStatus {
            id,
            title: "Storage temperature".into(),
            value,
            detail: if storage.model.is_empty() {
                "Storage".into()
            } else {
                storage.model
            },
            tone: MonitorTone::Warn,
        });
    }

    Ok(Dashboard {
        devices: statuses,
        dashboard,
        monitors,
    })
}

/// Read an `rgb.Color` from the API (0-255 channels, or its `Hex` fallback).
fn json_color(value: &serde_json::Value) -> Option<[u8; 3]> {
    let channel = |key: &str| value.get(key).and_then(|v| v.as_f64());
    if let (Some(r), Some(g), Some(b)) = (channel("red"), channel("green"), channel("blue")) {
        let clamp = |v: f64| v.round().clamp(0.0, 255.0) as u8;
        return Some([clamp(r), clamp(g), clamp(b)]);
    }
    let hex = value.get("Hex").and_then(|v| v.as_str())?;
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ])
}

fn cpu_model() -> Option<String> {
    let info = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    info.lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, model)| model.trim().to_string())
        .filter(|model| !model.is_empty())
}

fn gpu_model() -> Option<String> {
    let output = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    (!name.is_empty()).then_some(name)
}

// -- lighting ---------------------------------------------------------

/// A colour as OpenLinkHub represents it (`rgb.Color`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Color {
    #[serde(rename = "red", default)]
    pub red: f64,
    #[serde(rename = "green", default)]
    pub green: f64,
    #[serde(rename = "blue", default)]
    pub blue: f64,
    #[serde(rename = "brightness", default)]
    pub brightness: f64,
    #[serde(rename = "position", default)]
    pub position: f64,
    #[serde(rename = "temperature", default)]
    pub temperature: f64,
    #[serde(rename = "Hex", default)]
    pub hex: String,
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            red: f64::from(r),
            green: f64::from(g),
            blue: f64::from(b),
            brightness: 1.0,
            position: 0.0,
            temperature: 0.0,
            hex: String::new(),
        }
    }
}

/// One named lighting profile (the `"static"` one is solid colour).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbProfile {
    #[serde(rename = "profileName", default)]
    pub name: String,
    #[serde(rename = "speed", default)]
    pub speed: f64,
    #[serde(rename = "start", default)]
    pub start: Color,
}

/// The lighting payload returned by `GET /api/color/{serial}`.
#[derive(Debug, Clone, Deserialize)]
pub struct RgbData {
    #[serde(rename = "device", default)]
    pub device: String,
    #[serde(rename = "defaultColor", default)]
    pub default_color: Color,
    #[serde(rename = "profiles", default)]
    pub profiles: std::collections::HashMap<String, RgbProfile>,
}

#[derive(Debug, Deserialize)]
struct RgbResponse {
    #[allow(dead_code)]
    code: i32,
    #[allow(dead_code)]
    status: i32,
    data: Option<RgbData>,
}

/// Envelope returned by the write endpoints (`{code, status, message}`).
#[derive(Debug, Deserialize)]
struct StatusResponse {
    #[allow(dead_code)]
    code: i32,
    status: i32,
    #[serde(default)]
    message: String,
}

/// Fetch a device's lighting profiles.
pub fn fetch_rgb_profiles(serial: &str) -> Result<RgbData, String> {
    let body = get(&format!("{BASE}/api/color/{serial}"))?;
    let resp: RgbResponse = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    resp.data
        .ok_or_else(|| "device returned no lighting data".to_string())
}

/// Set a solid colour on a device: edit its `"static"` profile, then activate it.
pub fn set_solid_color(serial: &str, r: u8, g: u8, b: u8) -> Result<(), String> {
    let color = Color::rgb(r, g, b);
    let update = serde_json::json!({
        "deviceId": serial,
        "profile": "static",
        "startColor": color.clone(),
        "endColor": color.clone(),
        "middleColor": color,
        "speed": 1,
        "alternateColors": false,
        "rgbDirection": 0,
        "rgbMinTemp": 0,
        "rgbMaxTemp": 0,
    });
    send_json("PUT", &format!("{BASE}/api/color/change"), &update)?;
    activate_profile(serial, "static")
}

/// Activate a named lighting profile (e.g. `"rainbow"`, `"static"`).
pub fn activate_profile(serial: &str, profile: &str) -> Result<(), String> {
    let payload = serde_json::json!({ "deviceId": serial, "profile": profile });
    send_json("POST", &format!("{BASE}/api/color"), &payload)
}

/// Switch the active user profile.
pub fn change_user_profile(serial: &str, name: &str) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/userProfile/change"),
        &serde_json::json!({ "deviceId": serial, "userProfileName": name }),
    )
}

/// Set device brightness (gradual transition).
pub fn set_brightness(serial: &str, brightness: u8) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/brightness/gradual"),
        &serde_json::json!({ "deviceId": serial, "brightness": brightness }),
    )
}

/// Apply an RGB profile on a channel (0 = primary channel).
pub fn set_rgb_profile(serial: &str, channel: u8, profile: &str) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/color"),
        &serde_json::json!({ "deviceId": serial, "channelId": channel, "profile": profile }),
    )
}

/// Change the keyboard layout.
pub fn set_keyboard_layout(serial: &str, layout: &str) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/keyboard/layout"),
        &serde_json::json!({ "deviceId": serial, "keyboardLayout": layout }),
    )
}

/// Change the polling rate on the right endpoint for the device kind.
pub fn set_polling_rate(serial: &str, kind: DeviceKind, rate: i64) -> Result<(), String> {
    let path = match kind {
        DeviceKind::Keyboard => "keyboard",
        DeviceKind::Mouse => "mouse",
        _ => return Err("Polling rate is not supported for this device".into()),
    };
    send_json(
        "POST",
        &format!("{BASE}/api/{path}/pollingRate"),
        &serde_json::json!({ "deviceId": serial, "pollingRate": rate }),
    )
}

/// Enable or disable RGB cluster sync for a device.
pub fn set_rgb_cluster(serial: &str, on: bool) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/color/setCluster"),
        &serde_json::json!({ "deviceId": serial, "mode": if on { 1 } else { 0 } }),
    )
}

/// Add a device to the dashboard.
pub fn add_dashboard_device(serial: &str) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/dashboard/devices/add"),
        &serde_json::json!({ "deviceId": serial }),
    )
}

/// Remove a device from the dashboard.
pub fn remove_dashboard_device(serial: &str) -> Result<(), String> {
    send_json(
        "DELETE",
        &format!("{BASE}/api/dashboard/devices/delete"),
        &serde_json::json!({ "deviceId": serial }),
    )
}

/// Read the global dashboard settings.
pub fn fetch_dashboard_settings() -> Result<DashboardSettings, String> {
    #[derive(serde::Deserialize, Default)]
    struct Json {
        #[serde(default)]
        celsius: bool,
        #[serde(default, rename = "temperatureBar")]
        temperature_bar: bool,
        #[serde(default, rename = "addDeviceToDashboard")]
        add_device_to_dashboard: bool,
        #[serde(default, rename = "showLabels")]
        show_labels: bool,
        #[serde(default, rename = "rgbOff")]
        rgb_off: bool,
        #[serde(default, rename = "languageCode")]
        language_code: String,
        #[serde(default)]
        theme: String,
        #[serde(default)]
        themes: Vec<String>,
        #[serde(default, rename = "keyboardLayout")]
        keyboard_layout: i64,
        #[serde(default, rename = "keyboardLayouts")]
        keyboard_layouts: std::collections::HashMap<String, String>,
        #[serde(default, rename = "sidebarCollapsed")]
        sidebar_collapsed: bool,
    }
    #[derive(serde::Deserialize)]
    struct Response {
        status: i32,
        #[serde(default)]
        message: String,
        #[serde(default)]
        dashboard: Option<Json>,
    }

    let body = get(&format!("{BASE}/api/dashboard"))?;
    let resp: Response = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    if resp.status != 1 {
        return Err(if resp.message.is_empty() {
            "Unable to read dashboard settings".to_string()
        } else {
            resp.message
        });
    }
    let json = resp
        .dashboard
        .ok_or_else(|| "No dashboard settings returned".to_string())?;
    let mut keyboard_layouts: Vec<(i64, String)> = json
        .keyboard_layouts
        .into_iter()
        .filter_map(|(k, v)| k.parse::<i64>().ok().map(|n| (n, v)))
        .collect();
    keyboard_layouts.sort_by_key(|(n, _)| *n);
    Ok(DashboardSettings {
        celsius: json.celsius,
        temperature_bar: json.temperature_bar,
        add_device_to_dashboard: json.add_device_to_dashboard,
        show_labels: json.show_labels,
        rgb_off: json.rgb_off,
        language_code: json.language_code,
        theme: json.theme,
        themes: json.themes,
        keyboard_layout: json.keyboard_layout,
        keyboard_layouts,
        sidebar_collapsed: json.sidebar_collapsed,
    })
}

/// Save the global dashboard settings.
pub fn save_dashboard_settings(settings: &DashboardSettings) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/dashboard/update"),
        &serde_json::json!({
            "celsius": settings.celsius,
            "temperatureBar": settings.temperature_bar,
            "addDeviceToDashboard": settings.add_device_to_dashboard,
            "languageCode": settings.language_code,
            "showLabels": settings.show_labels,
            "theme": settings.theme,
            "keyboardLayout": settings.keyboard_layout,
            "rgbOff": settings.rgb_off,
        }),
    )
}

/// Persist the sidebar collapsed state.
pub fn set_sidebar_collapsed(collapsed: bool) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/dashboard/sidebar"),
        &serde_json::json!({ "sidebarCollapsed": collapsed }),
    )
}

/// Apply an RGB profile to every device.
pub fn set_global_rgb(profile: &str) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/color/global"),
        &serde_json::json!({ "profile": profile }),
    )
}

/// Apply a solid colour to every device.
pub fn set_all_devices_color(color: [u8; 3]) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/color/all"),
        &serde_json::json!({
            "color": {
                "red": color[0],
                "green": color[1],
                "blue": color[2],
                "brightness": 1
            }
        }),
    )
}

/// Update a key assignment.
pub fn update_key_assignment(
    serial: &str,
    key_index: i32,
    enabled: bool,
    press_and_hold: bool,
    type_code: i32,
    value: Option<i64>,
    value_string: Option<&str>,
    modifier: i32,
    retain_original: bool,
    toggle_delay: u16,
) -> Result<(), String> {
    update_assignment(
        "keyboard",
        serial,
        key_index,
        enabled,
        press_and_hold,
        type_code,
        value,
        value_string,
        modifier,
        retain_original,
        toggle_delay,
    )
}

/// Update a mouse button assignment.
pub fn update_mouse_assignment(
    serial: &str,
    key_index: i32,
    enabled: bool,
    press_and_hold: bool,
    type_code: i32,
    value: Option<i64>,
    value_string: Option<&str>,
    modifier: i32,
    retain_original: bool,
    toggle_delay: u16,
) -> Result<(), String> {
    update_assignment(
        "mouse",
        serial,
        key_index,
        enabled,
        press_and_hold,
        type_code,
        value,
        value_string,
        modifier,
        retain_original,
        toggle_delay,
    )
}

#[allow(clippy::too_many_arguments)]
fn update_assignment(
    kind: &str,
    serial: &str,
    key_index: i32,
    enabled: bool,
    press_and_hold: bool,
    type_code: i32,
    value: Option<i64>,
    value_string: Option<&str>,
    modifier: i32,
    retain_original: bool,
    toggle_delay: u16,
) -> Result<(), String> {
    let mut payload = serde_json::json!({
        "deviceId": serial,
        "keyIndex": key_index,
        "enabled": enabled,
        "pressAndHold": press_and_hold,
        "keyAssignmentType": type_code,
        "keyAssignmentModifier": modifier,
        "keyAssignmentOriginal": retain_original,
        "toggleDelay": toggle_delay,
    });
    if let Some(text) = value_string {
        payload["keyAssignmentValueString"] = serde_json::Value::String(text.to_string());
    } else if let Some(number) = value {
        payload["keyAssignmentValue"] = serde_json::Value::Number(number.into());
    }
    send_json(
        "POST",
        &format!("{BASE}/api/{kind}/updateKeyAssignment"),
        &payload,
    )
}

/// Switch the active keyboard layout profile.
pub fn change_keyboard_profile(serial: &str, name: &str) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/keyboard/profile/change"),
        &serde_json::json!({ "deviceId": serial, "keyboardProfileName": name }),
    )
}

/// Save the current keyboard layout profile.
pub fn save_keyboard_profile(serial: &str) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/keyboard/profile/save"),
        &serde_json::json!({ "deviceId": serial, "keyboardProfileName": "0", "new": false }),
    )
}

/// Save the current keyboard layout as a new profile.
pub fn new_keyboard_profile(serial: &str, name: &str) -> Result<(), String> {
    send_json(
        "PUT",
        &format!("{BASE}/api/keyboard/profile/new"),
        &serde_json::json!({ "deviceId": serial, "keyboardProfileName": name, "new": true }),
    )
}

/// Delete a keyboard layout profile.
pub fn delete_keyboard_profile(serial: &str, name: &str) -> Result<(), String> {
    send_json(
        "DELETE",
        &format!("{BASE}/api/keyboard/profile/delete"),
        &serde_json::json!({ "deviceId": serial, "keyboardProfileName": name }),
    )
}

/// Apply a colour to a key, row or the whole keyboard.
pub fn set_keyboard_color(
    serial: &str,
    key_id: i32,
    option: i32,
    color: [u8; 3],
    keys: &[i32],
) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/keyboard/color"),
        &serde_json::json!({
            "deviceId": serial,
            "keyId": key_id,
            "keyOption": option,
            "keys": keys,
            "color": { "red": color[0], "green": color[1], "blue": color[2] }
        }),
    )
}

/// Read the keyboard performance checkboxes.
pub fn fetch_keyboard_performance(serial: &str) -> Result<Vec<PerformanceItem>, String> {
    #[derive(serde::Deserialize)]
    struct Item {
        #[serde(default)]
        name: String,
        #[serde(default)]
        internal: String,
        #[serde(default)]
        value: bool,
    }
    #[derive(serde::Deserialize)]
    struct Response {
        status: i32,
        #[serde(default)]
        message: String,
        #[serde(default)]
        data: Vec<Item>,
    }

    let body = get(&format!("{BASE}/api/keyboard/getPerformance/{serial}"))?;
    let resp: Response = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    if resp.status != 1 {
        return Err(if resp.message.is_empty() {
            "Unable to read keyboard performance".to_string()
        } else {
            resp.message
        });
    }
    Ok(resp
        .data
        .into_iter()
        .map(|item| PerformanceItem {
            name: item.name,
            internal: item.internal,
            value: item.value,
        })
        .collect())
}

/// Save the keyboard performance checkboxes.
pub fn set_keyboard_performance(serial: &str, items: &[(String, bool)]) -> Result<(), String> {
    let mut payload = serde_json::json!({ "deviceId": serial });
    for (internal, value) in items {
        payload[internal.as_str()] = serde_json::Value::Bool(*value);
    }
    send_json(
        "POST",
        &format!("{BASE}/api/keyboard/setPerformance"),
        &payload,
    )
}

/// Read the keyboard FlashTap configuration.
pub fn fetch_flash_tap(serial: &str) -> Result<FlashTapData, String> {
    #[derive(serde::Deserialize)]
    struct Response {
        status: i32,
        #[serde(default)]
        message: String,
        #[serde(default)]
        data: Option<serde_json::Value>,
    }

    let body = get(&format!("{BASE}/api/keyboard/getFlashTap/{serial}"))?;
    let resp: Response = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    if resp.status != 1 {
        return Err(if resp.message.is_empty() {
            "No FlashTap data for this keyboard".to_string()
        } else {
            resp.message
        });
    }
    let data = resp
        .data
        .ok_or_else(|| "No FlashTap data for this keyboard".to_string())?;

    let active = data.get("active").and_then(|v| v.as_i64()).unwrap_or(0) != 0;
    let mode = data.get("mode").and_then(|v| v.as_i64()).unwrap_or(0);
    let color = data
        .get("color")
        .and_then(json_color)
        .unwrap_or([255, 255, 255]);
    let mut modes: Vec<(i64, String)> = data
        .get("modes")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| {
                    let n = k.parse::<i64>().ok()?;
                    Some((n, v.as_str()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    modes.sort_by_key(|(n, _)| *n);

    let mut keys: Vec<FlashTapKey> = data
        .get("keys")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| {
                    let index = k.parse::<usize>().ok()?;
                    Some(FlashTapKey {
                        index,
                        name: v
                            .get("Name")
                            .and_then(|n| n.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        key_data: v.get("KeyData").and_then(|n| n.as_i64()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    keys.sort_by_key(|key| key.index);

    Ok(FlashTapData {
        active,
        mode,
        modes,
        color,
        keys,
    })
}

/// Keys that can be used as FlashTap slots, as (code, name) pairs.
pub fn fetch_flash_tap_keys(serial: &str) -> Result<Vec<(String, String)>, String> {
    #[derive(serde::Deserialize)]
    struct Response {
        status: i32,
        #[serde(default)]
        message: String,
        #[serde(default)]
        data: std::collections::HashMap<String, String>,
    }

    let body = post_body(
        &format!("{BASE}/api/keyboard/getKeys/"),
        &serde_json::json!({ "deviceId": serial }),
    )?;
    let resp: Response = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    if resp.status != 1 {
        return Err(if resp.message.is_empty() {
            "Unable to read keyboard keys".to_string()
        } else {
            resp.message
        });
    }
    let mut keys: Vec<(i64, String, String)> = resp
        .data
        .into_iter()
        .filter_map(|(code, name)| code.parse::<i64>().ok().map(|n| (n, code, name)))
        .collect();
    keys.sort_by_key(|(n, _, _)| *n);
    Ok(keys
        .into_iter()
        .map(|(_, code, name)| (code, name))
        .collect())
}

/// Save the keyboard FlashTap configuration.
pub fn set_flash_tap(
    serial: &str,
    active: bool,
    mode: i64,
    keys: &[i64],
    color: [u8; 3],
) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/keyboard/setFlashTap"),
        &serde_json::json!({
            "deviceId": serial,
            "flashTapActive": if active { 1 } else { 0 },
            "flashTapKeys": keys,
            "flashTapMode": mode,
            "flashTapColor": { "red": color[0], "green": color[1], "blue": color[2] }
        }),
    )
}

/// Save mouse DPI stage values.
pub fn save_mouse_dpi(serial: &str, stages: &[u16]) -> Result<(), String> {
    let stages: serde_json::Map<String, serde_json::Value> = stages
        .iter()
        .enumerate()
        .map(|(i, value)| (i.to_string(), serde_json::json!(value)))
        .collect();
    send_json(
        "POST",
        &format!("{BASE}/api/mouse/dpi"),
        &serde_json::json!({ "deviceId": serial, "stages": stages }),
    )
}

/// Save the colour assigned to each DPI stage.
pub fn save_mouse_dpi_colors(serial: &str, colors: &[[u8; 3]]) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/mouse/dpiColors"),
        &serde_json::json!({ "deviceId": serial, "colorZones": color_map(colors) }),
    )
}

/// Save the colour of each RGB zone on a mouse.
pub fn save_mouse_zone_colors(serial: &str, colors: &[[u8; 3]]) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/mouse/zoneColors"),
        &serde_json::json!({ "deviceId": serial, "colorZones": color_map(colors) }),
    )
}

fn color_map(colors: &[[u8; 3]]) -> serde_json::Map<String, serde_json::Value> {
    colors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            (
                i.to_string(),
                serde_json::json!({ "red": c[0], "green": c[1], "blue": c[2] }),
            )
        })
        .collect()
}

/// Set the mouse sleep timer (minutes).
pub fn set_mouse_sleep(serial: &str, minutes: i64) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/mouse/sleep"),
        &serde_json::json!({ "deviceId": serial, "sleepMode": minutes }),
    )
}

/// Enable or disable angle snapping.
pub fn set_angle_snapping(serial: &str, on: bool) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/mouse/angleSnapping"),
        &serde_json::json!({ "deviceId": serial, "angleSnapping": if on { 1 } else { 0 } }),
    )
}

/// Enable or disable button response optimisation.
pub fn set_button_optimization(serial: &str, on: bool) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/mouse/buttonOptimization"),
        &serde_json::json!({ "deviceId": serial, "buttonOptimization": if on { 1 } else { 0 } }),
    )
}

/// Set the mouse lift-off height.
pub fn set_lift_height(serial: &str, height: i64) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/mouse/liftHeight"),
        &serde_json::json!({ "deviceId": serial, "liftHeight": height }),
    )
}

/// Enable or disable OpenRGB integration for a device.
pub fn set_open_rgb(serial: &str, on: bool) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/color/setOpenRgbIntegration"),
        &serde_json::json!({ "deviceId": serial, "mode": if on { 1 } else { 0 } }),
    )
}

/// Save the current device settings as a named user profile.
pub fn save_user_profile(serial: &str, name: &str) -> Result<(), String> {
    send_json(
        "PUT",
        &format!("{BASE}/api/userProfile"),
        &serde_json::json!({ "deviceId": serial, "userProfileName": name }),
    )
}

/// Delete a named user profile.
pub fn delete_user_profile(serial: &str, name: &str) -> Result<(), String> {
    send_json(
        "DELETE",
        &format!("{BASE}/api/userProfile/delete"),
        &serde_json::json!({ "deviceId": serial, "userProfileName": name }),
    )
}

/// Send a JSON body with the given method and check the `status` field (1 = ok).
fn send_json(method: &str, url: &str, payload: &serde_json::Value) -> Result<(), String> {
    let request = match method {
        "POST" => ureq::post(url),
        "PUT" => ureq::put(url),
        "DELETE" => ureq::delete(url),
        _ => return Err(format!("unsupported method: {method}")),
    };
    let body = request
        .timeout(TIMEOUT)
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .map_err(|e| format!("request failed: {e}"))?
        .into_string()
        .map_err(|e| format!("read failed: {e}"))?;
    let resp: StatusResponse = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    if resp.status == 1 {
        Ok(())
    } else if resp.message.is_empty() {
        Err("server rejected the change".to_string())
    } else {
        Err(resp.message)
    }
}

// -- macros -----------------------------------------------------------

/// OpenLinkHub macro action types.
pub const ACTION_MEDIA: u8 = 1;
pub const ACTION_KEYBOARD: u8 = 3;
pub const ACTION_DELAY: u8 = 5;
pub const ACTION_TEXT: u8 = 6;
pub const ACTION_MOUSE: u8 = 9;
pub const ACTION_MOUSE_POSITION: u8 = 20;

/// A human-readable label for a macro action type.
pub fn action_type_label(t: u8) -> &'static str {
    match t {
        ACTION_MEDIA => "Media key",
        ACTION_KEYBOARD => "Keyboard",
        ACTION_DELAY => "Delay",
        ACTION_TEXT => "Text",
        ACTION_MOUSE => "Mouse",
        ACTION_MOUSE_POSITION => "Mouse position",
        _ => "Unknown",
    }
}

/// One step inside a macro.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MacroAction {
    #[serde(rename = "actionType", default)]
    pub action_type: u8,
    #[serde(rename = "actionCommand", default)]
    pub action_command: u16,
    #[serde(rename = "actionDelay", default)]
    pub action_delay: u16,
    #[serde(rename = "actionHold", default)]
    pub action_hold: bool,
    #[serde(rename = "actionRepeat", default)]
    pub action_repeat: u8,
    #[serde(rename = "actionRepeatDelay", default)]
    pub action_repeat_delay: u16,
    #[serde(rename = "actionText", default)]
    pub action_text: String,
    #[serde(rename = "mousePositionX", default)]
    pub mouse_position_x: i32,
    #[serde(rename = "mousePositionY", default)]
    pub mouse_position_y: i32,
    #[serde(rename = "mousePositionAbsolute", default)]
    pub mouse_position_absolute: bool,
}

/// A named macro with its settings and steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub repeat: i32,
    #[serde(rename = "repeatDelay", default)]
    pub repeat_delay: i32,
    #[serde(default)]
    pub actions: std::collections::HashMap<String, MacroAction>,
}

/// Return this macro's actions ordered by their numeric index.
pub fn ordered_actions(m: &Macro) -> Vec<(i32, &MacroAction)> {
    let mut idx: Vec<i32> = m
        .actions
        .keys()
        .filter_map(|k| k.parse::<i32>().ok())
        .collect();
    idx.sort_unstable();
    idx.into_iter()
        .filter_map(|i| m.actions.get(&i.to_string()).map(|a| (i, a)))
        .collect()
}

#[derive(Debug, Deserialize)]
struct MacroListResponse {
    #[allow(dead_code)]
    code: i32,
    #[allow(dead_code)]
    status: i32,
    data: Option<std::collections::HashMap<String, Macro>>,
}

/// Fetch all macros, sorted by id.
pub fn fetch_macros() -> Result<Vec<Macro>, String> {
    let body = get(&format!("{BASE}/api/macro/"))?;
    let resp: MacroListResponse = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    let mut macros: Vec<Macro> = resp.data.unwrap_or_default().into_values().collect();
    macros.sort_by_key(|m| m.id);
    Ok(macros)
}

pub fn create_macro(name: &str) -> Result<(), String> {
    send_json(
        "PUT",
        &format!("{BASE}/api/macro/new"),
        &serde_json::json!({ "macroName": name }),
    )
}

pub fn delete_macro(id: i32) -> Result<(), String> {
    send_json(
        "DELETE",
        &format!("{BASE}/api/macro/profile"),
        &serde_json::json!({ "macroId": id }),
    )
}

pub fn update_macro_settings(id: i32, repeat: i32, repeat_delay: i32) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/macro/updateSettings"),
        &serde_json::json!({ "macroId": id, "macroRepeat": repeat, "macroRepeatDelay": repeat_delay }),
    )
}

pub fn add_macro_action(id: i32, a: &MacroAction) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/macro/newValue"),
        &serde_json::json!({
            "macroId": id,
            "macroType": a.action_type,
            "macroValue": a.action_command,
            "macroDelay": a.action_delay,
            "macroText": a.action_text,
            "mousePositionX": a.mouse_position_x,
            "mousePositionY": a.mouse_position_y,
            "mousePositionAbsolute": a.mouse_position_absolute,
        }),
    )
}

#[allow(dead_code)] // wired to the per-action hold/repeat editor next
pub fn update_macro_action(
    id: i32,
    index: i32,
    hold: bool,
    repeat: u8,
    repeat_delay: u16,
) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/macro/updateValue"),
        &serde_json::json!({
            "macroId": id,
            "macroIndex": index,
            "pressAndHold": hold,
            "actionRepeatValue": repeat,
            "actionRepeatDelay": repeat_delay,
        }),
    )
}

/// Rewrite a macro's actions in a new order: delete every current action, then
/// re-add them contiguously (0, 1, 2, ...). This is how "reindex" is done since
/// OpenLinkHub has no reorder endpoint. Per-action hold/repeat are re-applied
/// after each re-add so nothing is lost.
pub fn reorder_macro_actions(
    id: i32,
    current_indices: &[i32],
    ordered: &[MacroAction],
) -> Result<(), String> {
    for idx in current_indices {
        delete_macro_action(id, *idx)?;
    }
    for (new_idx, a) in ordered.iter().enumerate() {
        add_macro_action(id, a)?;
        update_macro_action(
            id,
            new_idx as i32,
            a.action_hold,
            a.action_repeat,
            a.action_repeat_delay,
        )?;
    }
    Ok(())
}

/// Set the global text-typing delay + jitter and the delay-action jitter
/// (persisted by OpenLinkHub).
pub fn set_text_delay(
    delay_ms: i32,
    variance_ms: i32,
    delay_variance_ms: i32,
) -> Result<(), String> {
    send_json(
        "POST",
        &format!("{BASE}/api/settings/textDelay"),
        &serde_json::json!({
            "textDelayMs": delay_ms,
            "textDelayVarianceMs": variance_ms,
            "delayVarianceMs": delay_variance_ms,
        }),
    )
}

pub fn delete_macro_action(id: i32, index: i32) -> Result<(), String> {
    send_json(
        "DELETE",
        &format!("{BASE}/api/macro/value"),
        &serde_json::json!({ "macroId": id, "macroIndex": index }),
    )
}

// -- input keys / buttons ----------------------------------------------

/// One entry in OpenLinkHub's key/button tables. The *map key* (the integer
/// index) is what a macro's `actionCommand` stores; `CommandCode` is the raw
/// USB/HID code the device uses internally.
#[derive(Debug, Clone, Deserialize)]
pub struct InputKey {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "CommandCode", default)]
    pub command_code: u16,
    #[allow(dead_code)]
    #[serde(rename = "Media", default)]
    pub media: bool,
    #[allow(dead_code)]
    #[serde(rename = "Mouse", default)]
    pub mouse: bool,
}

#[derive(Debug, Deserialize)]
struct InputListResponse {
    #[allow(dead_code)]
    code: i32,
    #[allow(dead_code)]
    status: i32,
    data: Option<std::collections::HashMap<u16, InputKey>>,
}

pub fn fetch_key_assignment_modifiers(serial: &str) -> Vec<(i32, String)> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        #[serde(default)]
        data: std::collections::HashMap<String, String>,
    }
    let Ok(body) = get(&format!(
        "{BASE}/api/keyboard/assignmentsModifiers/{serial}"
    )) else {
        return Vec::new();
    };
    let Ok(envelope) = serde_json::from_str::<Envelope>(&body) else {
        return Vec::new();
    };
    let mut items: Vec<(i32, String)> = envelope
        .data
        .into_iter()
        .filter_map(|(k, v)| k.parse::<i32>().ok().map(|n| (n, v)))
        .collect();
    items.sort_by_key(|(n, _)| *n);
    items
}

pub fn fetch_mouse_devices() -> Vec<(String, String)> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        #[serde(default)]
        data: std::collections::HashMap<String, String>,
    }
    let Ok(body) = get(&format!("{BASE}/api/devices/mouse")) else {
        return Vec::new();
    };
    let Ok(envelope) = serde_json::from_str::<Envelope>(&body) else {
        return Vec::new();
    };
    let mut items: Vec<(String, String)> = envelope.data.into_iter().collect();
    items.sort_by(|a, b| a.1.cmp(&b.1));
    items
}

fn fetch_input_map(path: &str) -> Result<Vec<(u16, InputKey)>, String> {
    let body = get(&format!("{BASE}{path}"))?;
    let resp: InputListResponse = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    let mut v: Vec<(u16, InputKey)> = resp.data.unwrap_or_default().into_iter().collect();
    v.sort_by_key(|(i, _)| *i);
    Ok(v)
}

/// Keyboard keys (excluding media), sorted by index.
pub fn fetch_keyboard_keys() -> Result<Vec<(u16, InputKey)>, String> {
    fetch_input_map("/api/input/keyboard")
}

/// Media keys (Volume Up/Down, Play/Pause, ...).
pub fn fetch_media_keys() -> Result<Vec<(u16, InputKey)>, String> {
    fetch_input_map("/api/input/media")
}

/// Mouse buttons (Left/Right/Middle/Back/Forward).
pub fn fetch_mouse_buttons() -> Result<Vec<(u16, InputKey)>, String> {
    fetch_input_map("/api/input/mouse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keyboard_profile() {
        let detail: DeviceDetail = serde_json::from_str(
            r#"{
                "DeviceProfile": {
                    "BrightnessSlider": 80,
                    "RGBProfile": "Rainbow Wave",
                    "PollingRate": 3,
                    "Profile": "Default"
                },
                "PollingRates": {"3": "1000 Hz"}
            }"#,
        )
        .unwrap();
        assert_eq!(detail.polling().as_deref(), Some("1000 Hz"));
        assert_eq!(detail.profile_name(), "Default");
        assert_eq!(detail.brightness(), 80);
        assert_eq!(detail.rgb_profile(), "Rainbow Wave");
        assert!(detail.dpi().is_none());
    }

    #[test]
    fn parses_mouse_dpi() {
        let detail: DeviceDetail = serde_json::from_str(
            r##"{
                "MinDPI": 100,
                "MaxDPI": 18000,
                "DeviceProfile": {
                    "Profile": 1,
                    "Profiles": {
                        "0": {"name": "Default", "Value": 800, "Color": {"red": 255, "green": 0, "blue": 0}},
                        "1": {"name": "FPS", "Value": 1600, "Color": {"red": 0, "green": 128, "blue": 255}}
                    },
                    "ZoneColors": {
                        "0": {"Name": "Logo", "Color": {"Hex": "#ff8000"}},
                        "1": {"Name": "Scroll wheel", "Color": {"red": 0, "green": 255, "blue": 0}}
                    }
                },
                "KeyAssignment": {
                    "2": {"name": "Right Button", "default": true, "actionType": 0, "actionCommand": 0},
                    "32": {"name": "Side Button 1", "default": false, "actionType": 3, "actionCommand": 8, "actionHold": true}
                }
            }"##,
        )
        .unwrap();
        assert_eq!(detail.dpi(), Some(1600));
        assert_eq!(detail.profile_name(), "FPS");
        assert_eq!(detail.dpi_range(), (100, 18000));
        let stages = detail.dpi_stages();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[1].name, "FPS");
        assert_eq!(stages[1].value, 1600);
        assert_eq!(stages[1].color, [0, 128, 255]);
        let zones = detail.zone_colors();
        assert_eq!(zones.len(), 2);
        assert_eq!(zones[0].name, "Logo");
        assert_eq!(zones[0].color, [255, 128, 0]);
        assert_eq!(zones[1].color, [0, 255, 0]);
        let buttons = detail.mouse_buttons();
        assert_eq!(buttons.len(), 2);
        assert_eq!(buttons[0].name, "Right Button");
        assert_eq!(buttons[1].index, 32);
        assert_eq!(buttons[1].action_type, 3);
        assert_eq!(buttons[1].action_command, 8);
        assert!(buttons[1].action_hold);
    }

    #[test]
    fn parses_channels_and_skips_string_maps() {
        let detail: DeviceDetail = serde_json::from_str(
            r#"{
                "devices": {
                    "0": {"name": "Fan 1", "rpm": 420, "HasSpeed": true},
                    "1": "Logo"
                }
            }"#,
        )
        .unwrap();
        let channels = detail.channels();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].rpm, Some(420.0));
        assert!(channels[0].has_speed);
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    #[test]
    fn parses_storage_temperature_list() {
        let env: StorageEnvelope = serde_json::from_str(
            r#"{"code":200,"status":1,"data":[
                {"Key":"hwmon0","Model":"INTEL SSDPEKNW020T8","Temperature":35.0,"TemperatureString":"35.0 °C"},
                {"Key":"hwmon1","Model":"Samsung SSD 970 EVO Plus 2TB","Temperature":44.0,"TemperatureString":"44.0 °C"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(env.data.len(), 2);
        assert_eq!(env.data[0].model, "INTEL SSDPEKNW020T8");
        assert_eq!(env.data[1].temperature_string, "44.0 °C");
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[test]
    fn parses_profiles_layouts_and_cluster() {
        let detail: DeviceDetail = serde_json::from_str(
            r#"{
                "UserProfiles": {
                    "Default": {"Active": true},
                    "FPS": {"Active": false}
                },
                "RGBModes": ["Static", "Breathing"],
                "Layouts": ["US", "UK"],
                "DeviceProfile": {
                    "Layout": "UK",
                    "RGBCluster": true,
                    "RGBProfile": "Breathing"
                }
            }"#,
        )
        .unwrap();
        assert_eq!(detail.profiles(), vec!["Default", "FPS"]);
        assert_eq!(detail.active_profile().as_deref(), Some("Default"));
        assert_eq!(detail.layout().as_deref(), Some("UK"));
        assert!(detail.cluster());
        assert_eq!(detail.rgb_modes.len(), 2);
        assert_eq!(detail.rgb_profile(), "Breathing");
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;

    #[test]
    fn parses_keyboard_layout() {
        let detail: DeviceDetail = serde_json::from_str(
            r#"{
                "DeviceProfile": {
                    "Profile": "k65",
                    "Keyboards": {
                        "k65": {
                            "row": {
                                "0": {"keys": {
                                    "1": {"keyName": "ESC", "width": 70,
                                          "color": {"red": 0, "green": 255, "blue": 0}},
                                    "2": {"keyName": "1", "width": 70,
                                          "color": {"red": 0, "green": 255, "blue": 255}}
                                }},
                                "1": {"keys": {
                                    "1": {"keyName": "SPACE", "width": 520}
                                }}
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();
        let layout = detail.keyboard_layout().unwrap();
        assert_eq!(layout.rows.len(), 2);
        assert_eq!(layout.rows[0].keys.len(), 2);
        assert_eq!(layout.rows[0].keys[0].name, "ESC");
        assert_eq!(layout.rows[0].keys[0].color, [0, 255, 0]);
        assert_eq!(layout.rows[1].keys[0].span, 7.0);
    }
}
