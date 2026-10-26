//! Look-up tables read from `.cube` files.
//!
//! The `.cube` format is plain text, as published by Adobe ("Cube LUT Specification", version
//! 1.0): keyword lines (`TITLE`, `LUT_1D_SIZE` or `LUT_3D_SIZE`, `DOMAIN_MIN`, `DOMAIN_MAX`),
//! comments starting with `#`, then one line of three numbers per table entry. In a 3D table red
//! changes fastest, then green, then blue. Some programs write `LUT_1D_INPUT_RANGE` or
//! `LUT_3D_INPUT_RANGE` with two numbers instead of the domain lines; those are read too.
//!
//! Lookups between entries are linear (1D) or trilinear (3D).

use std::fmt;

use serde::{Deserialize, Serialize};

/// The largest 3D table read: 65 entries per side, the size most grading programs write at
/// most (274,625 entries).
pub const MAX_3D_SIZE: usize = 65;
/// The largest 1D table read.
pub const MAX_1D_SIZE: usize = 65_536;

/// Whether a table maps each channel on its own or the three together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LutKind {
    /// One curve per channel.
    OneD,
    /// A cube of colours.
    ThreeD,
}

/// A look-up table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lut {
    /// The name in the file, or the file's name.
    pub title: String,
    pub kind: LutKind,
    /// Entries per side (3D) or entries (1D).
    pub size: usize,
    /// The input values that map to the first and last entries.
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
    /// `size` entries (1D) or `size`³ entries (3D), red fastest.
    pub table: Vec<[f32; 3]>,
}

/// Why a `.cube` file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LutError {
    /// Neither `LUT_1D_SIZE` nor `LUT_3D_SIZE` came before the first entry.
    NoSize,
    /// The file gives both sizes, or one twice.
    TwoSizes,
    /// The size is too small or too large to read.
    BadSize(usize),
    /// A line that is not a keyword, a comment or three numbers (1-based line number).
    BadLine(usize),
    /// The domain's maximum is not above its minimum.
    BadDomain,
    /// The file has a different number of entries than its size says.
    WrongCount { expected: usize, found: usize },
}

impl fmt::Display for LutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LutError::NoSize => write!(f, "the file does not say how large the table is"),
            LutError::TwoSizes => write!(f, "the file gives more than one table size"),
            LutError::BadSize(n) => write!(
                f,
                "a table of size {n} is not supported (3D up to {MAX_3D_SIZE}, 1D up to {MAX_1D_SIZE})"
            ),
            LutError::BadLine(n) => write!(f, "line {n} could not be read"),
            LutError::BadDomain => write!(f, "the domain's maximum is not above its minimum"),
            LutError::WrongCount { expected, found } => {
                write!(f, "expected {expected} entries, found {found}")
            }
        }
    }
}

impl std::error::Error for LutError {}

/// Three numbers, all finite.
fn triple<'a>(mut words: impl Iterator<Item = &'a str>) -> Option<[f32; 3]> {
    let mut out = [0.0; 3];
    for v in &mut out {
        *v = words
            .next()?
            .parse::<f32>()
            .ok()
            .filter(|x| x.is_finite())?;
    }
    words.next().is_none().then_some(out)
}

impl Lut {
    /// Reads the text of a `.cube` file. `fallback_title` names the table when the file does
    /// not (usually the file's name).
    pub fn parse_cube(text: &str, fallback_title: &str) -> Result<Lut, LutError> {
        let mut title: Option<String> = None;
        let mut kind_size: Option<(LutKind, usize)> = None;
        let mut domain_min = [0.0f32; 3];
        let mut domain_max = [1.0f32; 3];
        let mut table: Vec<[f32; 3]> = Vec::new();
        let mut expected = 0usize;

        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut words = line.split_whitespace();
            let Some(first) = words.next() else { continue };
            if first.starts_with(|c: char| c.is_ascii_alphabetic()) {
                let mut set_size = |kind: LutKind, words: &mut std::str::SplitWhitespace| {
                    if kind_size.is_some() {
                        return Err(LutError::TwoSizes);
                    }
                    let size: usize = words
                        .next()
                        .and_then(|w| w.parse().ok())
                        .ok_or(LutError::BadLine(n + 1))?;
                    let (ok, count) = match kind {
                        LutKind::OneD => ((2..=MAX_1D_SIZE).contains(&size), size),
                        LutKind::ThreeD => ((2..=MAX_3D_SIZE).contains(&size), size * size * size),
                    };
                    if !ok {
                        return Err(LutError::BadSize(size));
                    }
                    kind_size = Some((kind, size));
                    expected = count;
                    Ok(())
                };
                match first {
                    "TITLE" => {
                        let rest = line["TITLE".len()..].trim().trim_matches('"').trim();
                        title = Some(rest.to_string());
                    }
                    "LUT_1D_SIZE" => set_size(LutKind::OneD, &mut words)?,
                    "LUT_3D_SIZE" => set_size(LutKind::ThreeD, &mut words)?,
                    "DOMAIN_MIN" => domain_min = triple(words).ok_or(LutError::BadLine(n + 1))?,
                    "DOMAIN_MAX" => domain_max = triple(words).ok_or(LutError::BadLine(n + 1))?,
                    "LUT_1D_INPUT_RANGE" | "LUT_3D_INPUT_RANGE" => {
                        let lo = words.next().and_then(|w| w.parse::<f32>().ok());
                        let hi = words.next().and_then(|w| w.parse::<f32>().ok());
                        let (Some(lo), Some(hi)) = (lo, hi) else {
                            return Err(LutError::BadLine(n + 1));
                        };
                        if !lo.is_finite() || !hi.is_finite() {
                            return Err(LutError::BadLine(n + 1));
                        }
                        domain_min = [lo; 3];
                        domain_max = [hi; 3];
                    }
                    // Keywords of other programs are allowed and ignored.
                    _ => {}
                }
                continue;
            }
            let entry = triple(line.split_whitespace()).ok_or(LutError::BadLine(n + 1))?;
            if kind_size.is_none() {
                return Err(LutError::NoSize);
            }
            if table.len() >= expected {
                return Err(LutError::WrongCount {
                    expected,
                    found: table.len() + 1,
                });
            }
            if table.is_empty() {
                table.reserve_exact(expected);
            }
            table.push(entry);
        }

        let Some((kind, size)) = kind_size else {
            return Err(LutError::NoSize);
        };
        if table.len() != expected {
            return Err(LutError::WrongCount {
                expected,
                found: table.len(),
            });
        }
        if (0..3).any(|c| domain_max[c] <= domain_min[c]) {
            return Err(LutError::BadDomain);
        }
        Ok(Lut {
            title: title
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| fallback_title.to_string()),
            kind,
            size,
            domain_min,
            domain_max,
            table,
        })
    }

    /// True when the table has as many entries as its size says and every number is usable;
    /// a project file that fails this has its table dropped instead of trusted.
    pub fn is_valid(&self) -> bool {
        let expected = match self.kind {
            LutKind::OneD if (2..=MAX_1D_SIZE).contains(&self.size) => self.size,
            LutKind::ThreeD if (2..=MAX_3D_SIZE).contains(&self.size) => {
                self.size * self.size * self.size
            }
            _ => return false,
        };
        self.table.len() == expected
            && (0..3).all(|c| {
                self.domain_min[c].is_finite()
                    && self.domain_max[c].is_finite()
                    && self.domain_max[c] > self.domain_min[c]
            })
            && self.table.iter().flatten().all(|v| v.is_finite())
    }

    /// Where `v` falls in the table for channel `c`: the lower entry and the weight of the next.
    fn locate(&self, c: usize, v: f32) -> (usize, f32) {
        let span = self.domain_max[c] - self.domain_min[c];
        let t = ((v - self.domain_min[c]) / span).clamp(0.0, 1.0);
        let last = self.size - 1;
        let x = if t.is_finite() { t * last as f32 } else { 0.0 };
        let i = (x as usize).min(last.saturating_sub(1));
        (i, (x - i as f32).clamp(0.0, 1.0))
    }

    fn entry(&self, i: usize) -> [f32; 3] {
        self.table.get(i).copied().unwrap_or([0.0; 3])
    }

    /// Looks up one colour.
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        if self.size < 2 {
            return rgb;
        }
        match self.kind {
            LutKind::OneD => [0, 1, 2].map(|c| {
                let (i, f) = self.locate(c, rgb[c]);
                let a = self.entry(i)[c];
                let b = self.entry(i + 1)[c];
                a + (b - a) * f
            }),
            LutKind::ThreeD => {
                let n = self.size;
                let (r, fr) = self.locate(0, rgb[0]);
                let (g, fg) = self.locate(1, rgb[1]);
                let (b, fb) = self.locate(2, rgb[2]);
                let at = |dr: usize, dg: usize, db: usize| {
                    self.entry((r + dr) + (g + dg) * n + (b + db) * n * n)
                };
                let lerp =
                    |a: [f32; 3], b: [f32; 3], f: f32| [0, 1, 2].map(|c| a[c] + (b[c] - a[c]) * f);
                let c00 = lerp(at(0, 0, 0), at(1, 0, 0), fr);
                let c10 = lerp(at(0, 1, 0), at(1, 1, 0), fr);
                let c01 = lerp(at(0, 0, 1), at(1, 0, 1), fr);
                let c11 = lerp(at(0, 1, 1), at(1, 1, 1), fr);
                lerp(lerp(c00, c10, fg), lerp(c01, c11, fg), fb)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    /// A 3D cube that maps every colour to itself.
    fn identity_cube(n: usize) -> String {
        let mut s = format!("TITLE \"Identity\"\nLUT_3D_SIZE {n}\n");
        let m = (n - 1) as f32;
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    s += &format!("{} {} {}\n", r as f32 / m, g as f32 / m, b as f32 / m);
                }
            }
        }
        s
    }

    #[test]
    fn identity_3d_keeps_colours() {
        let lut = Lut::parse_cube(&identity_cube(5), "x").unwrap();
        assert_eq!(lut.title, "Identity");
        assert_eq!(
            (lut.kind, lut.size, lut.table.len()),
            (LutKind::ThreeD, 5, 125)
        );
        for rgb in [
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.3, 0.6, 0.9],
            [0.13, 0.77, 0.5],
        ] {
            assert!(close(lut.apply(rgb), rgb), "{rgb:?}");
        }
        assert!(lut.is_valid());
    }

    #[test]
    fn red_runs_fastest_in_3d_tables() {
        // A 2-cube whose output is the input with red and blue swapped.
        let mut text = String::from("LUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    text += &format!("{b} {g} {r}\n");
                }
            }
        }
        let lut = Lut::parse_cube(&text, "swap").unwrap();
        assert!(close(lut.apply([1.0, 0.0, 0.0]), [0.0, 0.0, 1.0]));
        assert!(close(lut.apply([0.25, 0.5, 0.75]), [0.75, 0.5, 0.25]));
        assert_eq!(lut.title, "swap");
    }

    #[test]
    fn one_d_tables_interpolate_per_channel() {
        let text = "# an inverting curve\nLUT_1D_SIZE 3\n1 1 1\n0.5 0.5 0.5\n0 0 0\n";
        let lut = Lut::parse_cube(text, "invert").unwrap();
        assert_eq!(lut.kind, LutKind::OneD);
        assert!(close(lut.apply([0.0, 0.25, 1.0]), [1.0, 0.75, 0.0]));
    }

    #[test]
    fn the_domain_moves_the_input() {
        let text = "LUT_1D_SIZE 2\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 2 2 2\n0 0 0\n1 1 1\n";
        let lut = Lut::parse_cube(text, "d").unwrap();
        assert!(close(lut.apply([1.0, 2.0, 4.0]), [0.5, 1.0, 1.0]));
        let ranged = "LUT_3D_INPUT_RANGE 0 2\nLUT_1D_SIZE 2\n0 0 0\n1 1 1\n";
        let lut = Lut::parse_cube(ranged, "r").unwrap();
        assert_eq!(lut.domain_max, [2.0; 3]);
    }

    #[test]
    fn malformed_files_are_refused() {
        assert_eq!(Lut::parse_cube("", "x"), Err(LutError::NoSize));
        assert_eq!(Lut::parse_cube("0 0 0\n", "x"), Err(LutError::NoSize));
        assert_eq!(
            Lut::parse_cube("LUT_3D_SIZE 1000\n", "x"),
            Err(LutError::BadSize(1000))
        );
        assert_eq!(
            Lut::parse_cube("LUT_3D_SIZE 2\nLUT_1D_SIZE 2\n", "x"),
            Err(LutError::TwoSizes)
        );
        assert_eq!(
            Lut::parse_cube("LUT_1D_SIZE 2\n0 0 0\n", "x"),
            Err(LutError::WrongCount {
                expected: 2,
                found: 1
            })
        );
        assert_eq!(
            Lut::parse_cube("LUT_1D_SIZE 2\n0 0 0\n1 1 1\n1 1 1\n", "x"),
            Err(LutError::WrongCount {
                expected: 2,
                found: 3
            })
        );
        assert_eq!(
            Lut::parse_cube("LUT_1D_SIZE 2\n0 0 nan\n1 1 1\n", "x"),
            Err(LutError::BadLine(2))
        );
        assert_eq!(
            Lut::parse_cube("LUT_1D_SIZE 2\n0 0\n", "x"),
            Err(LutError::BadLine(2))
        );
        assert_eq!(
            Lut::parse_cube("LUT_1D_SIZE 2\nDOMAIN_MAX 0 0 0\n0 0 0\n1 1 1\n", "x"),
            Err(LutError::BadDomain)
        );
        // Unknown keywords from other programs are fine.
        assert!(Lut::parse_cube("LUT_IN_VIDEO_RANGE\nLUT_1D_SIZE 2\n0 0 0\n1 1 1\n", "x").is_ok());
    }

    #[test]
    fn invalid_tables_are_detected() {
        let mut lut = Lut::parse_cube(&identity_cube(2), "x").unwrap();
        lut.table.pop();
        assert!(!lut.is_valid());
        // Lookups on a broken table do not panic.
        let _ = lut.apply([1.0, 1.0, 1.0]);
        let mut lut = Lut::parse_cube(&identity_cube(2), "x").unwrap();
        lut.table[0][1] = f32::NAN;
        assert!(!lut.is_valid());
    }
}
