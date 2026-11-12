//! Device artwork bundled with the app.
//!
//! The Scimitar line drawings in `assets/scimitar_wireless/` were made by the
//! project owner, so they are compiled into the binary. Other iCUE artwork is
//! proprietary and stays outside the repo; those devices keep the drawn icons.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::HashMap;

const MOUSE_TOP_PNG: &[u8] = include_bytes!("../../assets/scimitar_wireless/top.png");
const MOUSE_SIDE_PNG: &[u8] = include_bytes!("../../assets/scimitar_wireless/side.png");
const MOUSE_SIDE_BODY_PNG: &[u8] =
    include_bytes!("../../assets/scimitar_wireless/layers/side_body.png");
const MOUSE_SIDE_BUTTONS_PNG: &[u8] =
    include_bytes!("../../assets/scimitar_wireless/layers/side_buttons.png");

pub struct ArtCache {
    textures: HashMap<&'static str, Option<TextureHandle>>,
}

impl ArtCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Top-down line art for a Scimitar-class mouse, if present.
    #[allow(dead_code)]
    pub fn mouse_top(&mut self, ctx: &egui::Context, product: &str) -> Option<TextureHandle> {
        if !is_wireless_scimitar(product) {
            return None;
        }
        self.load(ctx, "scimitar-top", MOUSE_TOP_PNG)
    }

    /// Side line art for a Scimitar-class mouse, if present.
    pub fn mouse_side(&mut self, ctx: &egui::Context, product: &str) -> Option<TextureHandle> {
        if !is_wireless_scimitar(product) {
            return None;
        }
        self.load(ctx, "scimitar-side", MOUSE_SIDE_PNG)
    }

    /// Side view split into body and side-button layers, for per-zone tinting.
    pub fn mouse_side_layers(
        &mut self,
        ctx: &egui::Context,
        product: &str,
    ) -> Option<(TextureHandle, TextureHandle)> {
        if !is_wireless_scimitar(product) {
            return None;
        }
        let body = self.load(ctx, "scimitar-side-body", MOUSE_SIDE_BODY_PNG)?;
        let buttons = self.load(ctx, "scimitar-side-buttons", MOUSE_SIDE_BUTTONS_PNG)?;
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

fn is_wireless_scimitar(product: &str) -> bool {
    let name = product.to_uppercase();
    name.contains("SCIMITAR") && name.contains("WIRELESS")
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
