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

// -- dashboard --------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorTone {
    Warn,
    Good,
    Text,
}

#[derive(Debug, Clone)]
pub struct MonitorStatus {
    pub title: String,
    pub value: String,
    pub detail: String,
    pub tone: MonitorTone,
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
    pub profile: String,
    pub brightness: u8,
    pub rgb_profile: String,
    pub polling: Option<String>,
    #[allow(dead_code)]
    pub dpi: Option<u16>,
}

/// One snapshot of the dashboard: device cards and monitored values.
#[derive(Debug, Clone, Default)]
pub struct Dashboard {
    pub devices: Vec<DeviceStatus>,
    pub monitors: Vec<MonitorStatus>,
}

#[derive(Debug, Deserialize, Default)]
struct DeviceDetail {
    #[serde(default, rename = "DeviceProfile")]
    profile: Option<DeviceProfile>,
    #[serde(default, rename = "PollingRates")]
    polling_rates: HashMap<String, String>,
    #[serde(default)]
    devices: Option<serde_json::Value>,
    #[serde(default, rename = "Connected")]
    connected_cap: Option<bool>,
    #[serde(default)]
    connected: Option<bool>,
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
    #[serde(default, rename = "Label")]
    label: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct Channel {
    #[serde(default, rename = "name")]
    _name: String,
    #[serde(default)]
    rpm: Option<f64>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default, rename = "temperatureString")]
    temperature_string: Option<String>,
    #[serde(default, rename = "HasSpeed")]
    has_speed: bool,
    #[serde(default, rename = "HasTemps")]
    has_temps: bool,
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
        entry.get("Name")?.as_str().map(str::to_string)
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

fn fetch_device_detail(serial: &str) -> Result<DeviceDetail, String> {
    let body = get(&format!("{BASE}/api/devices/{serial}"))?;
    let env: DeviceEnvelope = serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    env.device.ok_or_else(|| "no device payload".to_string())
}

fn fetch_temperature(path: &str) -> Option<String> {
    let body = get(&format!("{BASE}{path}")).ok()?;
    let parsed: StringData = serde_json::from_str(&body).ok()?;
    let value = parsed.data.trim().to_string();
    (!value.is_empty()).then_some(value)
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
    let mut coolant: Option<(f64, String, String)> = None;
    let mut fan_totals: HashMap<String, u64> = HashMap::new();

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

        if matches!(kind, DeviceKind::Pump) {
            for ch in &channels {
                if (ch.has_temps || ch.temperature.unwrap_or(0.0) > 0.0)
                    && let Some(t) = ch.temperature
                    && t > 0.0
                    && coolant.as_ref().map_or(true, |(max, _, _)| t > *max)
                {
                    coolant = Some((
                        t,
                        dev.product.clone(),
                        ch.temperature_string
                            .clone()
                            .unwrap_or_else(|| format!("{t:.1} °C")),
                    ));
                }
            }
        }

        let total: u64 = channels
            .iter()
            .filter(|c| c.has_speed || c.rpm.unwrap_or(0.0) > 0.0)
            .map(|c| c.rpm.unwrap_or(0.0).max(0.0) as u64)
            .sum();
        if total > 0 {
            *fan_totals.entry(dev.product.clone()).or_insert(0) += total;
        }

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
                    (thousands(total), "Fan RPM".into())
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
                    (thousands(dpi as u64), "DPI".into())
                } else if let Some(b) = battery {
                    (format!("{}%", b.level), "Battery".into())
                } else {
                    ("Connected".into(), "Status".into())
                }
            }
            DeviceKind::Other => ("Connected".into(), "Status".into()),
        };

        statuses.push(DeviceStatus {
            product: dev.product.clone(),
            serial: dev.serial.clone(),
            firmware: dev.firmware.clone(),
            connected,
            battery: battery.map(|b| b.level.min(100) as u8),
            kind,
            stat_value,
            stat_label,
            profile,
            brightness,
            rgb_profile,
            polling,
            dpi,
        });
    }

    let mut monitors = Vec::new();
    if let Some(value) = fetch_temperature("/api/cpuTemp") {
        monitors.push(MonitorStatus {
            title: "CPU temp".into(),
            value,
            detail: cpu_model().unwrap_or_else(|| "Processor".into()),
            tone: MonitorTone::Warn,
        });
    }
    if let Some(value) = fetch_temperature("/api/gpuTemp") {
        monitors.push(MonitorStatus {
            title: "GPU temp".into(),
            value,
            detail: "Graphics".into(),
            tone: MonitorTone::Warn,
        });
    }
    if let Some(value) = fetch_temperature("/api/storageTemp") {
        monitors.push(MonitorStatus {
            title: "Storage".into(),
            value,
            detail: storage_model().unwrap_or_else(|| "Storage".into()),
            tone: MonitorTone::Warn,
        });
    }
    if let Some((_, product, value)) = coolant {
        monitors.push(MonitorStatus {
            title: "Coolant".into(),
            value,
            detail: product,
            tone: MonitorTone::Good,
        });
    }
    if let Some((product, total)) = fan_totals.into_iter().max_by_key(|(_, v)| *v) {
        monitors.push(MonitorStatus {
            title: "Fans".into(),
            value: format!("{} RPM", thousands(total)),
            detail: product,
            tone: MonitorTone::Text,
        });
    }

    Ok(Dashboard {
        devices: statuses,
        monitors,
    })
}

/// Group digits with spaces, matching the web UI's `1 240` style.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

fn cpu_model() -> Option<String> {
    let info = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    info.lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, model)| model.trim().to_string())
        .filter(|model| !model.is_empty())
}

fn storage_model() -> Option<String> {
    for entry in std::fs::read_dir("/sys/block").ok()? {
        let path = entry.ok()?.path();
        let name = path.file_name()?.to_string_lossy().to_string();
        if !name.starts_with("nvme") && !name.starts_with("sd") {
            continue;
        }
        if let Ok(model) = std::fs::read_to_string(path.join("device/model")) {
            let model = model.trim();
            if !model.is_empty() {
                return Some(model.to_string());
            }
        }
    }
    None
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
    fn groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1235), "1 235");
        assert_eq!(thousands(18000), "18 000");
    }

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
            r#"{
                "DeviceProfile": {
                    "Profile": 1,
                    "Profiles": {
                        "0": {"Name": "Default", "Value": 800},
                        "1": {"Name": "FPS", "Value": 1600}
                    }
                }
            }"#,
        )
        .unwrap();
        assert_eq!(detail.dpi(), Some(1600));
        assert_eq!(detail.profile_name(), "FPS");
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
