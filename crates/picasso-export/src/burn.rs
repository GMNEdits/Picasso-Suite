//! Text burned into the picture (the timecode, the clip name) with a small built-in 5 x 7 font,
//! so nothing needs a font file.

/// The columns of a glyph, five bytes with bit 0 at the top; characters the font lacks are blank.
fn glyph(c: char) -> [u8; 5] {
    match c.to_ascii_uppercase() {
        '0' => [0x3E, 0x51, 0x49, 0x45, 0x3E],
        '1' => [0x00, 0x42, 0x7F, 0x40, 0x00],
        '2' => [0x42, 0x61, 0x51, 0x49, 0x46],
        '3' => [0x21, 0x41, 0x45, 0x4B, 0x31],
        '4' => [0x18, 0x14, 0x12, 0x7F, 0x10],
        '5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        '6' => [0x3C, 0x4A, 0x49, 0x49, 0x30],
        '7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        '8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        '9' => [0x06, 0x49, 0x49, 0x29, 0x1E],
        ':' => [0x00, 0x36, 0x36, 0x00, 0x00],
        '.' => [0x00, 0x60, 0x60, 0x00, 0x00],
        '-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        '_' => [0x40, 0x40, 0x40, 0x40, 0x40],
        '/' => [0x20, 0x10, 0x08, 0x04, 0x02],
        'A' => [0x7E, 0x11, 0x11, 0x11, 0x7E],
        'B' => [0x7F, 0x49, 0x49, 0x49, 0x36],
        'C' => [0x3E, 0x41, 0x41, 0x41, 0x22],
        'D' => [0x7F, 0x41, 0x41, 0x41, 0x3E],
        'E' => [0x7F, 0x49, 0x49, 0x49, 0x41],
        'F' => [0x7F, 0x09, 0x09, 0x09, 0x01],
        'G' => [0x3E, 0x41, 0x49, 0x49, 0x7A],
        'H' => [0x7F, 0x08, 0x08, 0x08, 0x7F],
        'I' => [0x00, 0x41, 0x7F, 0x41, 0x00],
        'J' => [0x20, 0x40, 0x41, 0x3F, 0x01],
        'K' => [0x7F, 0x08, 0x14, 0x22, 0x41],
        'L' => [0x7F, 0x40, 0x40, 0x40, 0x40],
        'M' => [0x7F, 0x02, 0x0C, 0x02, 0x7F],
        'N' => [0x7F, 0x04, 0x08, 0x10, 0x7F],
        'O' => [0x3E, 0x41, 0x41, 0x41, 0x3E],
        'P' => [0x7F, 0x09, 0x09, 0x09, 0x06],
        'Q' => [0x3E, 0x41, 0x51, 0x21, 0x5E],
        'R' => [0x7F, 0x09, 0x19, 0x29, 0x46],
        'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
        'T' => [0x01, 0x01, 0x7F, 0x01, 0x01],
        'U' => [0x3F, 0x40, 0x40, 0x40, 0x3F],
        'V' => [0x1F, 0x20, 0x40, 0x20, 0x1F],
        'W' => [0x3F, 0x40, 0x38, 0x40, 0x3F],
        'X' => [0x63, 0x14, 0x08, 0x14, 0x63],
        'Y' => [0x07, 0x08, 0x70, 0x08, 0x07],
        'Z' => [0x61, 0x51, 0x49, 0x45, 0x43],
        _ => [0; 5],
    }
}

/// How big one font pixel is for a picture this tall.
fn scale(height: usize) -> usize {
    (height / 150).max(1)
}

/// Draws `text` white on a dark band with its top left at (`x`, `y`) in an RGBA picture of
/// `w` x `h`; whatever falls outside the picture is left out.
pub fn draw_text(rgba: &mut [u8], (w, h): (usize, usize), x: usize, y: usize, text: &str) {
    let k = scale(h);
    let chars: Vec<char> = text.chars().take(200).collect();
    let width = chars.len() * 6 * k + 2 * k;
    let height = 9 * k;
    let mut put = |px: usize, py: usize, rgb: [u8; 3], blend: bool| {
        if px >= w || py >= h {
            return;
        }
        if let Some(p) = rgba.get_mut((py * w + px) * 4..(py * w + px) * 4 + 3) {
            for (d, s) in p.iter_mut().zip(rgb) {
                *d = if blend { (u16::from(*d) / 3) as u8 } else { s };
            }
        }
    };
    // The band behind the text: the picture darkened, so the text reads on anything.
    for by in 0..height {
        for bx in 0..width {
            put(x + bx, y + by, [0; 3], true);
        }
    }
    for (n, c) in chars.iter().enumerate() {
        for (col, bits) in glyph(*c).into_iter().enumerate() {
            for row in 0..7 {
                if bits >> row & 1 == 1 {
                    for dy in 0..k {
                        for dx in 0..k {
                            let px = x + k + (n * 6 + col) * k + dx;
                            let py = y + k + row * k + dy;
                            put(px, py, [255; 3], false);
                        }
                    }
                }
            }
        }
    }
}

/// Burns the timecode at the top left and the clip name at the bottom left of a picture.
pub fn burn_in(rgba: &mut [u8], size: (usize, usize), timecode: Option<&str>, name: Option<&str>) {
    let k = scale(size.1);
    let margin = 4 * k;
    if let Some(text) = timecode {
        draw_text(rgba, size, margin, margin, text);
    }
    if let Some(text) = name.filter(|t| !t.is_empty()) {
        let y = size.1.saturating_sub(9 * k + margin);
        draw_text(rgba, size, margin, y, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_drawn_on_a_dark_band_and_only_inside_the_picture() {
        let (w, h) = (200, 150);
        let mut rgba = [200u8, 200, 200, 255].repeat(w * h);
        draw_text(&mut rgba, (w, h), 4, 4, "01:00:00:00");
        let lit = rgba
            .chunks_exact(4)
            .filter(|p| p[0] == 255 && p[1] == 255)
            .count();
        assert!(lit > 50, "white pixels: {lit}");
        let dark = rgba.chunks_exact(4).filter(|p| p[0] < 100).count();
        assert!(dark > lit, "a band behind the text");
        // Far from the text the picture is as it was.
        assert_eq!(&rgba[((h - 1) * w + w - 1) * 4..][..3], &[200, 200, 200]);
        // Off the edge, and a hostile position, draw nothing and do not panic.
        draw_text(&mut rgba, (w, h), usize::MAX / 2, usize::MAX / 2, "X");
        draw_text(&mut rgba, (w, h), w - 3, h - 3, "ABCDEF");
        let mut tiny = vec![0u8; 4];
        draw_text(&mut tiny, (1, 1), 0, 0, "A");
        draw_text(&mut [], (0, 0), 0, 0, "A");
    }

    #[test]
    fn the_burn_in_puts_the_timecode_above_and_the_name_below() {
        let (w, h) = (300, 150);
        let mut rgba = [100u8, 100, 100, 255].repeat(w * h);
        burn_in(&mut rgba, (w, h), Some("00:00:01:00"), Some("Shot one"));
        let row_has_white = |y: usize| (0..w).any(|x| rgba[(y * w + x) * 4] == 255);
        assert!((0..h / 2).any(row_has_white), "timecode in the top half");
        assert!((h / 2..h).any(row_has_white), "name in the bottom half");
        let mut none = [100u8, 100, 100, 255].repeat(w * h);
        burn_in(&mut none, (w, h), None, None);
        assert!(none.chunks_exact(4).all(|p| p[0] == 100));
    }
}
