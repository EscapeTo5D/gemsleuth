//! 嵌入素材装载(§5.2):include_bytes! → image 解码 → ColorImage 纹理。

use eframe::egui;

pub mod asset_bytes {
    /// 同名替换文件后重新编译即生效,无需改代码。
    pub const GEMS: [&[u8]; 6] = [
        include_bytes!("../../assets/gems/gem_0.png"),
        include_bytes!("../../assets/gems/gem_1.png"),
        include_bytes!("../../assets/gems/gem_2.png"),
        include_bytes!("../../assets/gems/gem_3.png"),
        include_bytes!("../../assets/gems/gem_4.png"),
        include_bytes!("../../assets/gems/gem_5.png"),
    ];
    pub const EXACT: &[u8] = include_bytes!("../../assets/icons/exact.png");
    pub const PARTIAL: &[u8] = include_bytes!("../../assets/icons/partial.png");
    pub const UNKNOWN: &[u8] = include_bytes!("../../assets/icons/unknown.png");
    /// 全窗口背景图(1820×1024),cover 方式铺满。
    pub const BG: &[u8] = include_bytes!("../../assets/Bg_loading_loading_wurt_trailer.png");
}

pub struct Assets {
    pub gems: Vec<egui::TextureHandle>,
    pub exact: egui::TextureHandle,
    pub partial: egui::TextureHandle,
    pub unknown: egui::TextureHandle,
    pub bg: egui::TextureHandle,
}

fn tex(ctx: &egui::Context, name: &str, bytes: &[u8]) -> egui::TextureHandle {
    let img = image::load_from_memory(bytes).expect("内置素材解码失败");
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba),
        egui::TextureOptions::LINEAR,
    )
}

impl Assets {
    pub fn load(ctx: &egui::Context) -> Self {
        let mut gems = Vec::with_capacity(6);
        for (i, b) in asset_bytes::GEMS.iter().enumerate() {
            gems.push(tex(ctx, &format!("gem_{i}"), b));
        }
        Self {
            gems,
            exact: tex(ctx, "exact", asset_bytes::EXACT),
            partial: tex(ctx, "partial", asset_bytes::PARTIAL),
            unknown: tex(ctx, "unknown", asset_bytes::UNKNOWN),
            bg: tex(ctx, "bg", asset_bytes::BG),
        }
    }

    pub fn gem(&self, idx: u8) -> &egui::TextureHandle {
        &self.gems[idx as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::asset_bytes;

    fn png_dims(b: &[u8]) -> (u32, u32) {
        assert_eq!(&b[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "PNG 签名");
        let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
        let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
        (w, h)
    }

    #[test]
    fn embedded_assets_decode_with_expected_dims() {
        // 同时验证 Task 9 手写 PNG 编码器产物可被真正解码
        for (i, b) in asset_bytes::GEMS.iter().enumerate() {
            assert_eq!(png_dims(b), (128, 128), "gem_{i} 尺寸");
            image::load_from_memory(b).expect("gem 解码失败");
        }
        for (name, b) in [
            ("exact", asset_bytes::EXACT),
            ("partial", asset_bytes::PARTIAL),
            ("unknown", asset_bytes::UNKNOWN),
        ] {
            assert_eq!(png_dims(b), (64, 64), "{name} 尺寸");
            image::load_from_memory(b).expect("{name} 解码失败");
        }
        assert_eq!(png_dims(asset_bytes::BG), (1820, 1024), "bg 尺寸");
        image::load_from_memory(asset_bytes::BG).expect("bg 解码失败");
    }
}
