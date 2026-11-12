//! Device artwork bundled with the app.
//!
//! The mouse line drawings in `assets/` were made by the project owner, so
//! they are compiled into the binary. Other iCUE artwork is proprietary and
//! stays outside the repo; those devices keep the drawn icons.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::HashMap;

const DARKSTAR_TOP: &[u8] = include_bytes!("../../assets/darkstar_wireless/top.png");
const DARKSTAR_SIDE: &[u8] = include_bytes!("../../assets/darkstar_wireless/side.png");
const DARKSTAR_SIDE_BODY: &[u8] =
    include_bytes!("../../assets/darkstar_wireless/layers/side_body.png");
const DARKSTAR_SIDE_BUTTONS: &[u8] =
    include_bytes!("../../assets/darkstar_wireless/layers/side_buttons.png");

const SCIMITAR_WIRELESS_TOP: &[u8] = include_bytes!("../../assets/scimitar_wireless/top.png");
const SCIMITAR_WIRELESS_SIDE: &[u8] = include_bytes!("../../assets/scimitar_wireless/side.png");
const SCIMITAR_WIRELESS_SIDE_BODY: &[u8] =
    include_bytes!("../../assets/scimitar_wireless/layers/side_body.png");
const SCIMITAR_WIRELESS_SIDE_BUTTONS: &[u8] =
    include_bytes!("../../assets/scimitar_wireless/layers/side_buttons.png");

const SCIMITAR_WIRED_TOP: &[u8] = include_bytes!("../../assets/scimitar_wired/top.png");
const SCIMITAR_WIRED_SIDE: &[u8] = include_bytes!("../../assets/scimitar_wired/side.png");
const SCIMITAR_WIRED_SIDE_BODY: &[u8] =
    include_bytes!("../../assets/scimitar_wired/layers/side_body.png");
const SCIMITAR_WIRED_SIDE_BUTTONS: &[u8] =
    include_bytes!("../../assets/scimitar_wired/layers/side_buttons.png");

/// Bundled art for one mouse family.
struct Art {
    top: (&'static str, &'static [u8]),
    side: (&'static str, &'static [u8]),
    side_body: (&'static str, &'static [u8]),
    side_buttons: (&'static str, &'static [u8]),
}

static ART_DARKSTAR: Art = Art {
    top: ("darkstar-top", DARKSTAR_TOP),
    side: ("darkstar-side", DARKSTAR_SIDE),
    side_body: ("darkstar-side-body", DARKSTAR_SIDE_BODY),
    side_buttons: ("darkstar-side-buttons", DARKSTAR_SIDE_BUTTONS),
};

static ART_SCIMITAR_WIRELESS: Art = Art {
    top: ("scimitar-wireless-top", SCIMITAR_WIRELESS_TOP),
    side: ("scimitar-wireless-side", SCIMITAR_WIRELESS_SIDE),
    side_body: ("scimitar-wireless-side-body", SCIMITAR_WIRELESS_SIDE_BODY),
    side_buttons: ("scimitar-wireless-side-buttons", SCIMITAR_WIRELESS_SIDE_BUTTONS),
};

static ART_SCIMITAR_WIRED: Art = Art {
    top: ("scimitar-wired-top", SCIMITAR_WIRED_TOP),
    side: ("scimitar-wired-side", SCIMITAR_WIRED_SIDE),
    side_body: ("scimitar-wired-side-body", SCIMITAR_WIRED_SIDE_BODY),
    side_buttons: ("scimitar-wired-side-buttons", SCIMITAR_WIRED_SIDE_BUTTONS),
};

/// Pick the artwork bundle for a product name; `None` keeps the drawn icon.
fn art_for(product: &str) -> Option<&'static Art> {
    let n = product.to_uppercase();
    if n.contains("DARKSTAR") {
        Some(&ART_DARKSTAR)
    } else if n.contains("SCIMITAR") && n.contains("WIRELESS") {
        Some(&ART_SCIMITAR_WIRELESS)
    } else if n.contains("SCIMITAR") {
        Some(&ART_SCIMITAR_WIRED)
    } else {
        None
    }
}

pub struct ArtCache {
    textures: HashMap<&'static str, Option<TextureHandle>>,
}

impl ArtCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Top-down line art for a supported mouse, if present.
    #[allow(dead_code)]
    pub fn mouse_top(&mut self, ctx: &egui::Context, product: &str) -> Option<TextureHandle> {
        let art = art_for(product)?;
        self.load(ctx, art.top.0, art.top.1)
    }

    /// Side line art for a supported mouse, if present.
    pub fn mouse_side(&mut self, ctx: &egui::Context, product: &str) -> Option<TextureHandle> {
        let art = art_for(product)?;
        self.load(ctx, art.side.0, art.side.1)
    }

    /// Side view split into body and side-button layers, for per-zone tinting.
    pub fn mouse_side_layers(
        &mut self,
        ctx: &egui::Context,
        product: &str,
    ) -> Option<(TextureHandle, TextureHandle)> {
        let art = art_for(product)?;
        let body = self.load(ctx, art.side_body.0, art.side_body.1)?;
        let buttons = self.load(ctx, art.side_buttons.0, art.side_buttons.1)?;
        Some((body, buttons))
    }

    fn load(
        &mut self,
        ctx: &egui::Context,
        key: &'static str,
        bytes: &'static [u8],
    ) -> Option<TextureHandle> {
        if let Some(cached) = self.textures.get(key) {
            return cached.clone();
        }
        let texture = load_texture(ctx, key, bytes);
        self.textures.insert(key, texture.clone());
        texture
    }
}

fn load_texture(
    ctx: &egui::Context,
    key: &str,
    bytes: &[u8],
) -> Option<TextureHandle> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture(key.to_string(), pixels, TextureOptions::LINEAR))
}

/// Fit `size` inside `container` without distorting it.
pub fn fit_rect(container: egui::Rect, size: egui::Vec2) -> egui::Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return container;
    }
    let scale = (container.width() / size.x)
        .min(container.height() / size.y)
        .max(0.0);
    egui::Rect::from_center_size(container.center(), size * scale)
}

/// Draw line art tinted with the current effect colour.
pub fn draw_line_art(
    painter: &egui::Painter,
    container: egui::Rect,
    texture: &TextureHandle,
    tint: eframe::egui::Color32,
) {
    let target = fit_rect(container, texture.size_vec2());
    painter.image(
        texture.id(),
        target,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        tint,
    );
}
