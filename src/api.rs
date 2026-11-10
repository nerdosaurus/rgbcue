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

impl Device {
    /// Best-effort device category, inferred from the product name. The list
    /// endpoint doesn't populate `DeviceType`, so we guess until per-device
    /// detail is wired up.
    pub fn kind(&self) -> &'static str {
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
            "Keyboard"
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
            "Mouse"
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
            "Cooling"
        } else if n.contains("RECEIVER") || n.contains("SLIPSTREAM") || n.contains("DONGLE") {
            "Receiver"
        } else {
            "Device"
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
    let mut devices: Vec<Device> = resp
        .devices
        .into_values()
        .filter(|d| !d.hidden)
        .collect();
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
pub fn set_text_delay(delay_ms: i32, variance_ms: i32, delay_variance_ms: i32) -> Result<(), String> {
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
