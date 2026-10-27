//! Apple II image tables and a 280 x 192 hi-res screen to lay them on.
//!
//! Image tables are the raw `IMG.*` files from Mechner's source tree. Format
//! (HIRES.S): byte 0 = image count, then a 2-byte absolute pointer per image
//! (every table was saved from $6000), then per image: width in bytes, height
//! in lines, and the bytes. Each byte holds 7 pixels, bit 0 leftmost; bit 7 is
//! the colour palette bit, which a 1-bit Braille dot has no use for. Rows run
//! from the bottom line up: YCO is an image's bottom edge.
//!
//! The screen keeps one "ink" byte per pixel instead of a bit, so the Braille
//! renderer can colour a cell by what was drawn there (wall, torch, kid).

pub const SCREEN_W: i32 = 280;
pub const SCREEN_H: i32 = 192;
const TABLE_BASE: usize = 0x6000;

#[derive(Clone, Copy)]
pub struct ImageTable {
    data: &'static [u8],
}

#[derive(Clone, Copy)]
pub struct Image<'a> {
    /// width in bytes (7 pixels each)
    pub w: usize,
    pub h: usize,
    data: &'a [u8],
}

impl Image<'_> {
    pub fn width_px(&self) -> i32 {
        self.w as i32 * 7
    }

    /// Pixel `x` (0 = leftmost) of line `row` (0 = bottom line).
    #[inline]
    pub fn bit(&self, x: usize, row: usize) -> bool {
        self.data[row * self.w + x / 7] >> (x % 7) & 1 != 0
    }

    /// The colour palette bit (bit 7) of the byte holding pixel `x`.
    #[inline]
    pub fn pal(&self, x: usize, row: usize) -> bool {
        self.data[row * self.w + x / 7] & 0x80 != 0
    }
}

impl ImageTable {
    pub const fn new(data: &'static [u8]) -> ImageTable {
        ImageTable { data }
    }

    pub fn count(&self) -> u8 {
        self.data[0]
    }

    /// Image number `n` (1-based, as in the source).
    pub fn get(&self, n: u8) -> Option<Image<'static>> {
        if n == 0 || n > self.count() {
            return None;
        }
        let i = 1 + 2 * (n as usize - 1);
        let ptr = self.data[i] as usize | (self.data[i + 1] as usize) << 8;
        let o = ptr.checked_sub(TABLE_BASE)?;
        let (w, h) = (*self.data.get(o)? as usize, *self.data.get(o + 1)? as usize);
        let data = self.data.get(o + 2..o + 2 + w * h)?;
        Some(Image { w, h, data })
    }
}

macro_rules! table {
    ($name:ident, $file:literal) => {
        pub static $name: ImageTable = ImageTable::new(include_bytes!(concat!("../assets/apple2/", $file)));
    };
}

table!(CHTAB1, "IMG.CHTAB1");
table!(CHTAB2, "IMG.CHTAB2");
table!(CHTAB3, "IMG.CHTAB3");
table!(CHTAB4_GD, "IMG.CHTAB4.GD");
table!(CHTAB4_SKEL, "IMG.CHTAB4.SKEL");
table!(CHTAB4_FAT, "IMG.CHTAB4.FAT");
table!(CHTAB4_SHAD, "IMG.CHTAB4.SHAD");
table!(CHTAB4_VIZ, "IMG.CHTAB4.VIZ");
table!(CHTAB5, "IMG.CHTAB5");
table!(BGTAB1_DUN, "IMG.BGTAB1.DUN");
table!(BGTAB2_DUN, "IMG.BGTAB2.DUN");
table!(BGTAB1_PAL, "IMG.BGTAB1.PAL");
table!(BGTAB2_PAL, "IMG.BGTAB2.PAL");

/// Character set loaded as chtable4 on each level (MISC.S chset). Sets 0
/// and 2 are the same guard images, read from either side of the disk.
const CHSET: [u8; 15] = [0, 0, 0, 1, 2, 2, 3, 2, 2, 2, 2, 2, 4, 5, 5];

/// Character table by FCharTable number (0-7; GRAFIX.S chtablist) on
/// level `level`, which decides what chtable4 holds.
pub fn chtable(n: u8, level: u8) -> Option<&'static ImageTable> {
    match n {
        0 => Some(&CHTAB1),
        1 => Some(&CHTAB2),
        2 => Some(&CHTAB3),
        3 => Some(match CHSET.get(level as usize).copied().unwrap_or(0) {
            1 => &CHTAB4_SKEL,
            3 => &CHTAB4_FAT,
            4 => &CHTAB4_SHAD,
            5 => &CHTAB4_VIZ,
            _ => &CHTAB4_GD,
        }),
        4 => Some(&CHTAB5),
        _ => None,
    }
}

/// How an image combines with the screen (HIRES.S OPACITY).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Op {
    /// clear screen pixels where the image has none (within its box)
    And,
    Or,
    /// replace the box outright
    Sta,
    /// clear the image's silhouette widened by a pixel each side, then OR
    /// (MASKTAB in HRTABLES.S), which keeps a dark outline round characters
    Mask,
    /// flip screen pixels under the image (the shadowman)
    Eor,
}

/// What a pixel belongs to, for colouring. Order is draw priority within a cell.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum Ink {
    None = 0,
    Wall = 1,
    Floor = 2,
    Gate = 3,
    Flame = 4,
    Potion = 5,
    Sword = 6,
    Kid = 7,
    Guard = 8,
}

/// Lo-res / double hi-res colours, indexed by a 4-dot NTSC pattern.
pub const NTSC: [[u8; 3]; 16] = [
    [0, 0, 0],
    [227, 30, 96],
    [96, 78, 189],
    [255, 68, 253],
    [0, 163, 96],
    [156, 156, 156],
    [20, 207, 253],
    [208, 195, 255],
    [96, 114, 3],
    [255, 106, 60],
    [156, 156, 156],
    [255, 160, 208],
    [20, 245, 60],
    [208, 221, 141],
    [114, 255, 208],
    [255, 255, 255],
];

pub struct Screen {
    pub px: Vec<Ink>,
    pub pal: Vec<bool>,
}

impl Default for Screen {
    fn default() -> Self {
        let n = (SCREEN_W * SCREEN_H) as usize;
        Screen { px: vec![Ink::None; n], pal: vec![false; n] }
    }
}

impl Screen {
    pub fn clear(&mut self) {
        self.px.fill(Ink::None);
        self.pal.fill(false);
    }

    /// Colour of every pixel as an Apple II shows it on a colour monitor.
    ///
    /// Each hi-res pixel is two dots of a 560-dot NTSC line, shifted half a
    /// pixel when its byte's palette bit is set. Every dot takes one of 16
    /// colours from the 4 dots ending at it (by absolute phase), and a pixel
    /// is the average of the dots around it: the monitor's horizontal blur.
    /// Alternating patterns come out as solid colour, `###.` stipples as a
    /// light tint, and lone gaps between lit pixels stay readable.
    pub fn colors(&self) -> Vec<[u8; 3]> {
        let w = SCREEN_W as usize;
        let mut out = vec![[0u8; 3]; self.px.len()];
        let mut dots = vec![false; 2 * w + 4];
        let mut dcol = vec![[0u16; 3]; 2 * w + 4];
        for y in 0..SCREEN_H as usize {
            let row = y * w;
            dots.fill(false);
            for x in 0..w {
                if self.px[row + x] != Ink::None {
                    let d = 2 * x + self.pal[row + x] as usize;
                    dots[d] = true;
                    dots[d + 1] = true;
                }
            }
            for d in 0..dots.len() {
                let mut v = 0usize;
                for i in 0..4 {
                    if d >= i && dots[d - i] {
                        v |= 1 << ((d - i) & 3);
                    }
                }
                let c = NTSC[v];
                dcol[d] = [c[0] as u16, c[1] as u16, c[2] as u16];
            }
            for x in 0..w {
                let mut acc = [0u16; 3];
                let lo = (2 * x).saturating_sub(1);
                let hi = (2 * x + 3).min(dcol.len());
                for c in &dcol[lo..hi] {
                    for k in 0..3 {
                        acc[k] += c[k];
                    }
                }
                let n = (hi - lo) as u16;
                out[row + x] = [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8];
            }
        }
        out
    }

    #[inline]
    fn set(&mut self, x: i32, y: i32, v: Ink, pal: bool) {
        if (0..SCREEN_W).contains(&x) && (0..SCREEN_H).contains(&y) {
            let i = (y * SCREEN_W + x) as usize;
            self.px[i] = v;
            self.pal[i] = pal;
        }
    }

    /// Lay `img` with its bottom-left pixel at (`x`, `ybot`) in 280-res
    /// pixels. Mirrored images are laid right to left from `x` + 6, the way
    /// MLAY treats XCO as the image's bottom right byte.
    pub fn lay(&mut self, img: &Image, x: i32, ybot: i32, mirror: bool, op: Op, ink: Ink) {
        let wpx = img.width_px();
        for row in 0..img.h {
            let y = ybot - row as i32;
            if !(0..SCREEN_H).contains(&y) {
                continue;
            }
            let sx = |c: i32| if mirror { x + 6 - c } else { x + c };
            match op {
                Op::Or => {
                    for c in 0..wpx {
                        if img.bit(c as usize, row) {
                            self.set(sx(c), y, ink, img.pal(c as usize, row));
                        }
                    }
                }
                Op::And => {
                    for c in 0..wpx {
                        if !img.bit(c as usize, row) {
                            self.set(sx(c), y, Ink::None, false);
                        }
                    }
                }
                Op::Sta => {
                    for c in 0..wpx {
                        let v = if img.bit(c as usize, row) { ink } else { Ink::None };
                        self.set(sx(c), y, v, img.pal(c as usize, row));
                    }
                }
                Op::Eor => {
                    for c in 0..wpx {
                        if img.bit(c as usize, row) {
                            let (px, py) = (sx(c), y);
                            if (0..SCREEN_W).contains(&px) {
                                let i = (py * SCREEN_W + px) as usize;
                                let v = if self.px[i] == Ink::None { ink } else { Ink::None };
                                self.set(px, py, v, img.pal(c as usize, row));
                            }
                        }
                    }
                }
                Op::Mask => {
                    for c in 0..wpx {
                        let on = |k: i32| k >= 0 && k < wpx && img.bit(k as usize, row);
                        if on(c - 1) || on(c) || on(c + 1) {
                            self.set(sx(c), y, Ink::None, false);
                        }
                    }
                    for c in 0..wpx {
                        if img.bit(c as usize, row) {
                            self.set(sx(c), y, ink, img.pal(c as usize, row));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_parses() {
        for t in [&CHTAB1, &CHTAB2, &CHTAB3, &CHTAB4_GD, &CHTAB4_SKEL, &CHTAB4_FAT, &CHTAB4_SHAD, &CHTAB4_VIZ, &CHTAB5, &BGTAB1_DUN, &BGTAB2_DUN, &BGTAB1_PAL, &BGTAB2_PAL] {
            for n in 1..=t.count() {
                assert!(t.get(n).is_some(), "image {n}");
            }
        }
    }

    #[test]
    fn standing_kid_is_a_person_sized_image() {
        // frame 15 (stand) uses chtable1 image $0f
        let img = CHTAB1.get(0x0f).unwrap();
        assert_eq!((img.w, img.h), (2, 41));
        // feet on the bottom line, head at the top
        assert!((0..14).any(|x| img.bit(x, 0)));
        assert!((0..14).any(|x| img.bit(x, 40)));
    }
}
