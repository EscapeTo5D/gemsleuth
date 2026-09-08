//! 占位素材生成器(零第三方依赖,手写最小 PNG 编码)。
//! 运行:`cargo run --example gen_assets -- <要重生成的文件路径...>`
//! 例:`cargo run --example gen_assets -- assets/icons/unknown.png`
//! gem_0..5 已替换为真实宝石图标,默认不再重生成,只有显式点名才会覆盖。

use std::env;
use std::fs;
use std::io;
use std::path::Path;

// ---------- PNG 编码:zlib 存储块(无压缩)+ adler32 + crc32 ----------

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// zlib 流,全部用存储(不压缩)块:格式合法即可,体积无关紧要。
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    if raw.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    } else {
        let chunks: Vec<&[u8]> = raw.chunks(65535).collect();
        for (i, c) in chunks.iter().enumerate() {
            let last = i == chunks.len() - 1;
            let len = c.len() as u16;
            out.push(if last { 1 } else { 0 });
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(c);
        }
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for y in 0..height {
        raw.push(0); // 每行 filter = None
        for x in 0..width {
            raw.extend_from_slice(&pixel(x, y));
        }
    }
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8bit, RGBA, 无压缩/滤波/隔行
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

// ---------- 绘制(圆形占位 / 空槽磁贴) ----------

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 4] {
    let m = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), 255]
}

fn circle_png(size: u32, fill: [u8; 3], highlight: bool, ring: Option<([u8; 3], f64, f64)>) -> Vec<u8> {
    let c = size as f64 / 2.0;
    let r = c - 2.0;
    png(size, size, |x, y| {
        let (fx, fy) = (x as f64 + 0.5 - c, y as f64 + 0.5 - c);
        let d2 = fx * fx + fy * fy;
        if d2 > r * r {
            return [0, 0, 0, 0]; // 透明背景
        }
        if let Some((ring_rgb, r0, r1)) = ring {
            let rr = d2.sqrt();
            if rr >= r * r0 && rr <= r * r1 {
                return [ring_rgb[0], ring_rgb[1], ring_rgb[2], 255];
            }
        }
        if highlight {
            let (hx, hy) = (fx + r * 0.35, fy + r * 0.35);
            if hx * hx + hy * hy <= (r * 0.45) * (r * 0.45) {
                return mix(fill, [255, 255, 255], 0.45); // 左上高光
            }
        }
        [fill[0], fill[1], fill[2], 255]
    })
}

/// 空槽占位:全透明。空槽在 UI 里用与色盘宝石同款的 Button 磁贴呈现,
/// 方形效果来自按钮本身的深色底/圆角/描边,内部无需再画任何图形。
fn transparent_png(size: u32) -> Vec<u8> {
    png(size, size, |_, _| [0, 0, 0, 0])
}

// ---------- 主流程 ----------

fn assert_png(b: &[u8], w: u32, h: u32) {
    assert_eq!(&b[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "PNG 签名");
    assert_eq!(u32::from_be_bytes(b[16..20].try_into().unwrap()), w, "宽度");
    assert_eq!(u32::from_be_bytes(b[20..24].try_into().unwrap()), h, "高度");
}

fn write(path: &str, bytes: &[u8]) -> io::Result<()> {
    fs::write(Path::new(path), bytes)?;
    println!("{path} ({} 字节)", bytes.len());
    Ok(())
}

fn main() -> io::Result<()> {
    // 颜色索引:0红 1蓝 2紫 3橙 4黄 5绿(规格 §5.2,GUI 固定六色)
    const GEM_COLORS: [[u8; 3]; 6] = [
        [229, 57, 53],  // 红
        [30, 136, 229], // 蓝
        [142, 36, 170], // 紫
        [251, 140, 0],  // 橙
        [253, 216, 53], // 黄
        [67, 160, 71],  // 绿
    ];

    let wanted: Vec<String> = env::args().skip(1).collect();
    if wanted.is_empty() {
        println!("用法: cargo run --example gen_assets -- <要重生成的文件路径...>");
        println!("  例: cargo run --example gen_assets -- assets/icons/unknown.png");
        println!("注意: gem_0..5 与 exact/partial 已是真实素材,不点名不会覆盖");
        return Ok(());
    }

    fs::create_dir_all("assets/gems")?;
    fs::create_dir_all("assets/icons")?;

    for (i, rgb) in GEM_COLORS.iter().enumerate() {
        let path = format!("assets/gems/gem_{i}.png");
        if !wanted.contains(&path) {
            continue;
        }
        let bytes = circle_png(128, *rgb, true, None);
        assert_png(&bytes, 128, 128);
        write(&path, &bytes)?;
    }

    // exact/partial 已是真实游戏图标(方底裁圆版);unknown = 空槽磁贴内部(全透明,
    // 方形效果由 UI 按钮磁贴提供)。以下仅为兜底占位,不点名不会覆盖真实素材。
    let icons = [
        ("assets/icons/exact.png", circle_png(64, [30, 136, 229], false, None)),   // 蓝标占位
        ("assets/icons/partial.png", circle_png(64, [255, 193, 7], false, None)),  // 金标占位
        ("assets/icons/unknown.png", transparent_png(64)),
    ];
    for (path, bytes) in icons {
        if !wanted.contains(&path.to_string()) {
            continue;
        }
        assert_png(&bytes, 64, 64);
        write(path, &bytes)?;
    }
    Ok(())
}
