//! The app icon, drawn in code so the window, the Windows .exe and the Linux desktop entry share one design:
//! a gold tile with five dark level bars rising to the middle, like a meter on an amplifier.

/// Bar heights as a share of the tallest bar, left to right.
pub const BARS: [f32; 5] = [0.36, 0.66, 1.0, 0.66, 0.36];
/// Total width taken by the bars, as a share of the tile side.
pub const BARS_WIDTH: f32 = 0.58;
/// Gap between bars, as a share of the tile side.
pub const BAR_GAP: f32 = 0.065;
/// Height of the tallest bar, as a share of the tile side.
pub const BARS_HEIGHT: f32 = 0.58;
/// Corner radius, as a share of the tile side.
pub const CORNER: f32 = 0.24;

const TOP: [f32; 3] = [0xe8 as f32, 0xab as f32, 0x22 as f32];
const BOTTOM: [f32; 3] = [0xa1 as f32, 0x62 as f32, 0x07 as f32];
const BAR: [f32; 3] = [0x16 as f32, 0x12 as f32, 0x0e as f32];

/// Signed distance from a point to a rounded rectangle given by its centre, half size and corner radius.
fn rounded_rect(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - (hw - r);
    let qy = (py - cy).abs() - (hh - r);
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// Rectangles of the bars inside a tile of side `side` whose top-left corner is the origin:
/// (centre x, centre y, half width, half height).
pub fn bar_rects(side: f32) -> Vec<(f32, f32, f32, f32)> {
    let count = BARS.len() as f32;
    let width = (BARS_WIDTH - BAR_GAP * (count - 1.0)) / count * side;
    let left = (1.0 - BARS_WIDTH) / 2.0 * side;
    BARS.iter()
        .enumerate()
        .map(|(i, h)| {
            let cx = left + width / 2.0 + i as f32 * (width + BAR_GAP * side);
            (cx, side / 2.0, width / 2.0, BARS_HEIGHT * side * h / 2.0)
        })
        .collect()
}

/// Straight-alpha RGBA pixels of a square icon, anti-aliased by 4×4 supersampling.
pub fn rgba(size: u32) -> Vec<u8> {
    const SAMPLES: u32 = 4;
    let s = size as f32;
    // A small transparent margin keeps the tile from touching the edges, as system icons do.
    let margin = (s * 0.03).max(0.5);
    let side = s - margin * 2.0;
    let half = side / 2.0;
    let bars = bar_rects(side);
    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let (mut tile, mut bar) = (0u32, 0u32);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32 - margin;
                    let py = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32 - margin;
                    if rounded_rect(px, py, half, half, half, half, side * CORNER) <= 0.0 {
                        tile += 1;
                        if bars.iter().any(|&(cx, cy, hw, hh)| rounded_rect(px, py, cx, cy, hw, hh, hw) <= 0.0) {
                            bar += 1;
                        }
                    }
                }
            }
            if tile == 0 {
                continue;
            }
            let t = ((y as f32 + 0.5 - margin) / side).clamp(0.0, 1.0);
            let mix = bar as f32 / tile as f32;
            let pixel = &mut out[((y * size + x) * 4) as usize..][..4];
            for c in 0..3 {
                let base = TOP[c] + (BOTTOM[c] - TOP[c]) * t;
                pixel[c] = (base + (BAR[c] - base) * mix).round() as u8;
            }
            pixel[3] = (255 * tile / (SAMPLES * SAMPLES)) as u8;
        }
    }
    out
}

/// A Windows .ico holding the icon at the usual sizes, as uncompressed 32-bit bitmaps.
pub fn ico(sizes: &[u32]) -> Vec<u8> {
    let mut images = Vec::new();
    for &size in sizes {
        let pixels = rgba(size);
        let mask_row = ((size + 31) / 32 * 4) as usize;
        let mut dib = Vec::new();
        for value in [40u32, size, size * 2] {
            dib.extend_from_slice(&value.to_le_bytes());
        }
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&[0u8; 24]);
        // Bitmap rows run bottom to top in BGRA order.
        for y in (0..size).rev() {
            for x in 0..size {
                let p = &pixels[((y * size + x) * 4) as usize..][..4];
                dib.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
        dib.extend(std::iter::repeat(0u8).take(mask_row * size as usize));
        images.push((size, dib));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, dib) in &images {
        let side = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(dib.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += dib.len() as u32;
    }
    for (_, dib) in images {
        out.extend_from_slice(&dib);
    }
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A PNG of the icon for Linux desktop entries. The image data uses uncompressed deflate blocks,
/// which every PNG reader accepts, so no compression library is needed.
pub fn png(size: u32) -> Vec<u8> {
    let pixels = rgba(size);
    let mut raw = Vec::new();
    for row in pixels.chunks(size as usize * 4) {
        raw.push(0u8);
        raw.extend_from_slice(row);
    }
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        zlib.push((i + 1 == blocks.len()) as u8);
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + byte as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut header = Vec::new();
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &header);
    chunk(b"IDAT", &zlib);
    chunk(b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_are_transparent_and_centre_is_a_bar() {
        let size = 64;
        let pixels = rgba(size);
        assert_eq!(pixels[3], 0, "top-left corner");
        let centre = ((size / 2 * size + size / 2) * 4) as usize;
        assert_eq!(pixels[centre + 3], 255);
        assert!(pixels[centre] < 0x30, "the middle bar is dark");
    }

    #[test]
    fn crc_matches_the_png_reference_value() {
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
    }

    #[test]
    fn ico_header_lists_every_size() {
        let data = ico(&[16, 32, 256]);
        assert_eq!(&data[..6], &[0, 0, 1, 0, 3, 0]);
        assert_eq!(data[6], 16);
        assert_eq!(data[6 + 32], 0, "256 px is stored as 0");
    }
}
