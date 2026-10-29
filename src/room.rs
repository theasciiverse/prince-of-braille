//! Drawing the princess's room.
//!
//! The picture is the DOS version's, in its own colours: the room and the
//! bed, the hourglass and its sand, the stars in the window and the two
//! pillars in front (from PrinceJS, see tools/dos_room.py), at the places
//! SDLPoP and PrinceJS put them. The characters and the torch flames are
//! the Apple's images, drawn over it with the animation the scenes run
//! (GAMEBG.S PMASK, PSETUPFLAME).
//!
//! DOS x is the Apple's scaled by 8/7 (SDLPoP calc_screen_x_coord), and the
//! DOS room stands its characters 16 lines lower (the vizier at y 167, the
//! Apple's at 151). So the picture is drawn from its line 8 down and the
//! characters 8 lines lower than the Apple had them, which fits the whole
//! 192 lines and puts their feet where the DOS game does.

use crate::apple::{self, Ink, Op, Screen, SCREEN_H, SCREEN_W};
use crate::cut::Cut;
use crate::game::*;

static DOS: &[u8] = include_bytes!("../assets/dos/room.bin");
/// images in room.bin
const ROOM: usize = 0;
const PILLAR: usize = 1;
const CLOCK: usize = 2;
const SAND: usize = 9;
/// the room's first line on screen
const TOP: i32 = 8;
/// characters and flames go this much lower than the Apple placed them
const DY: i32 = 16 - TOP;

// SDLPoP draw_hourglass, PrinceJS Clock: the hourglass and its sand
const CLOCKX: i32 = 152;
const CLOCKY: i32 = 141;
const SANDX: i32 = CLOCKX + 8;
const SANDY: i32 = CLOCKY + 16;
// PrinceJS Scene: the stars and the front pillars
const STARS: [(i32, i32); 6] = [(20, 97), (16, 104), (23, 110), (17, 116), (24, 120), (18, 128)];
const PILLARS: [(i32, i32); 2] = [(59, 120), (240, 120)];
/// the torch cups' centres in the picture, and SDLPoP's flame line (116)
const TORCHX: [i32; 2] = [99, 217];
const TORCHY: i32 = 116;
/// the DOS night sky's blue
const SKY: [u8; 3] = [0, 24, 96];

const PMASKDY: [i32; 2] = [-4, -33];
const PMASKI: [u8; 2] = [0x2c, 0x22];
const PTORCHFLAME: [u8; 18] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 3, 5, 7, 1, 4, 9, 2, 8, 6];
/// chtable6 as FCharTable numbers it
const CH6: u8 = 5;

/// The picture behind and in front of the scene, one colour per screen pixel.
pub struct Backdrop {
    pub back: Vec<[u8; 3]>,
    pub front: Vec<Option<[u8; 3]>>,
}

/// An image in room.bin: width, height, palette indices (0 transparent).
struct Pic {
    w: i32,
    h: i32,
    px: &'static [u8],
}

fn palette() -> &'static [u8] {
    &DOS[1..1 + DOS[0] as usize * 3]
}

fn pic(mut n: usize) -> Pic {
    let mut at = 1 + DOS[0] as usize * 3;
    loop {
        let w = u16::from_le_bytes([DOS[at], DOS[at + 1]]) as usize;
        let h = u16::from_le_bytes([DOS[at + 2], DOS[at + 3]]) as usize;
        if n == 0 {
            return Pic { w: w as i32, h: h as i32, px: &DOS[at + 4..at + 4 + w * h] };
        }
        at += 4 + w * h;
        n -= 1;
    }
}

/// A 320 x 200 DOS picture being put together.
struct Canvas(Vec<u8>);

const DOS_W: i32 = 320;
const DOS_H: i32 = 200;

impl Canvas {
    fn blit(&mut self, p: &Pic, x: i32, y: i32) {
        for r in 0..p.h {
            for c in 0..p.w {
                let v = p.px[(r * p.w + c) as usize];
                let (px, py) = (x + c, y + r);
                if v != 0 && (0..DOS_W).contains(&px) && (0..DOS_H).contains(&py) {
                    self.0[(py * DOS_W + px) as usize] = v;
                }
            }
        }
    }

    /// Screen pixel (x, y): the DOS pixels under it that aren't clear,
    /// averaged (black if all are), so the night sky's dither of blue and
    /// clear comes out as solid blue; and whether most of them are clear.
    fn sample(&self, pal: &[u8], x: i32, y: i32) -> ([u8; 3], bool) {
        let dy = y + TOP;
        if !(0..DOS_H).contains(&dy) {
            return ([0; 3], true);
        }
        let (x0, x1) = (x * 8 / 7, ((x + 1) * 8 / 7).max(x * 8 / 7 + 1));
        let (mut acc, mut n, mut clear) = ([0u32; 3], 0u32, 0u32);
        for dx in x0..x1.min(DOS_W) {
            let v = self.0[(dy * DOS_W + dx) as usize] as usize;
            if v == 0 {
                clear += 1;
                continue;
            }
            for k in 0..3 {
                acc[k] += pal[(v - 1) * 3 + k] as u32;
            }
            n += 1;
        }
        let n1 = n.max(1);
        ([(acc[0] / n1) as u8, (acc[1] / n1) as u8, (acc[2] / n1) as u8], clear > n)
    }
}

/// The DOS picture for this frame of the scene.
pub fn backdrop(cut: &Cut) -> Backdrop {
    let room = pic(ROOM);
    let mut back = Canvas(room.px.to_vec());
    // the hourglass: the Apple's nine states (0 full, 8 empty) onto the
    // DOS picture's seven, and the sand falling while it runs
    if let Some(gs) = cut.glass {
        let gs = gs.min(8) as usize;
        back.blit(&pic(CLOCK + (gs * 6 + 4) / 8), CLOCKX, CLOCKY);
        if cut.sand >= 0 && gs < 8 {
            back.blit(&pic(SAND + cut.sand as usize % 3), SANDX, SANDY);
        }
    }
    let pal = palette();
    let mut front = Canvas(vec![0; (DOS_W * DOS_H) as usize]);
    let pillar = pic(PILLAR);
    for &(x, y) in &PILLARS {
        front.blit(&pillar, x, y);
    }
    let n = (SCREEN_W * SCREEN_H) as usize;
    let mut out = Backdrop { back: vec![[0; 3]; n], front: vec![None; n] };
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            let i = (y * SCREEN_W + x) as usize;
            out.back[i] = match back.sample(pal, x, y).0 {
                // the night sky, lifted just past where the renderer turns
                // near-black cells black
                SKY => [0, 32, 112],
                c => c,
            };
            let (c, clear) = front.sample(pal, x, y);
            out.front[i] = (!clear).then_some(c);
        }
    }
    // the stars: dim, and white while one twinkles (pstars)
    for (n, &(sx, sy)) in STARS.iter().enumerate() {
        let lit = cut.stars[n % 4] > 0 && n < 4;
        let (x, y) = (sx * 7 / 8, sy - TOP);
        out.back[(y * SCREEN_W + x) as usize] = if lit { [255; 3] } else { [85; 3] };
    }
    out
}

/// The scene in the DOS art: the room, the hourglass, the stars, the
/// torch flames, the characters from their DOS sheets and the pillars in
/// front, as a picture for the DOS renderer.
pub fn picture(g: &Game, cut: &Cut) -> crate::dos::Pic {
    use crate::dos::{self, S};
    let mut back = Canvas(pic(ROOM).px.to_vec());
    if let Some(gs) = cut.glass {
        let gs = gs.min(8) as usize;
        back.blit(&pic(CLOCK + (gs * 6 + 4) / 8), CLOCKX, CLOCKY);
        if cut.sand >= 0 && gs < 8 {
            back.blit(&pic(SAND + cut.sand as usize % 3), SANDX, SANDY);
        }
    }
    let pal = palette();
    let colour = |v: u8| -> Option<[u8; 3]> {
        let v = v as usize;
        (v != 0).then(|| [pal[(v - 1) * 3], pal[(v - 1) * 3 + 1], pal[(v - 1) * 3 + 2]])
    };
    let mut out = dos::Pic::new();
    for y in 0..dos::H {
        for x in 0..dos::W {
            if let Some(c) = colour(back.0[((y + TOP) * DOS_W + x) as usize]) {
                // the night sky, lifted just past where the renderer turns
                // near-black cells black
                out.put(x, y, if c == SKY { [0, 32, 112] } else { c }, false);
            }
        }
    }
    for (n, &(sx, sy)) in STARS.iter().enumerate() {
        let lit = cut.stars[n % 4] > 0 && n < 4;
        out.put(sx, sy - TOP, if lit { [255; 3] } else { [85; 3] }, false);
    }
    // the torch flames over the picture's cups
    for (t, &cx) in TORCHX.iter().enumerate() {
        let name = format!("fire_{}", cut.torch[t] % 9 + 1);
        if let Some(f) = dos::img(S::General, &name) {
            out.named(S::General, &name, cx - f.w / 2, TORCHY - TOP - f.h);
        }
    }
    // the characters, the lower one on the screen behind (sortlist)
    let mut chars: Vec<&Char> = [&g.kid, &g.shad].into_iter().filter(|c| c.posn != 0).collect();
    if chars.len() == 2 && chars[1].y > chars[0].y {
        chars.swap(0, 1);
    }
    for c in chars {
        dos::cut_character(&mut out, c, DY + 1);
    }
    let pillar = pic(PILLAR);
    for &(px, py) in &PILLARS {
        for r in 0..pillar.h {
            for c in 0..pillar.w {
                if let Some(col) = colour(pillar.px[(r * pillar.w + c) as usize]) {
                    out.put(px + c, py + r - TOP, col, false);
                }
            }
        }
    }
    out
}

/// The scene's Apple images: the torch flames and the characters.
pub fn draw(scr: &mut Screen, g: &Game, cut: &Cut) {
    scr.clear();
    let ch6 = apple::chtable(CH6, cut.level);
    // pburn: the torch flames, over the picture's torch cups
    for (t, &cx) in TORCHX.iter().enumerate() {
        let f = PTORCHFLAME.get(cut.torch[t] as usize).copied().unwrap_or(1);
        if let Some(i) = ch6.and_then(|t| t.get(f)) {
            scr.lay(&i, cx * 7 / 8 - i.width_px() / 2, TORCHY - TOP, false, Op::Sta, Ink::Flame);
        }
    }
    // the mid plane: kid, vizier or mouse, then the princess, sorted as
    // sortlist does (the lower one on the screen goes behind)
    let mut chars: Vec<&Char> = [&g.kid, &g.shad].into_iter().filter(|c| c.posn != 0).collect();
    if chars.len() == 2 && chars[1].y > chars[0].y {
        chars.swap(0, 1);
    }
    for c in chars {
        let (f, _) = crate::bg::setupchar(c);
        let Some(img) = apple::chtable(f.table, cut.level).and_then(|t| t.get(f.image)) else { continue };
        let mirror = f.face & 0x80 == 0;
        let ink = if matches!(c.id, 0 | 5) { Ink::Kid } else { Ink::Guard };
        let y = f.y as i32 + DY;
        scr.lay(&img, f.x, y, mirror, Op::Mask, ink);
        // PMASK: the princess's face and hair in some positions
        if c.id == 5 {
            let m = match c.posn {
                19 => Some(0),
                1 | 18 => Some(1),
                _ => None,
            };
            if let Some(m) = m {
                if let Some(i) = ch6.and_then(|t| t.get(PMASKI[m])) {
                    scr.lay(&i, f.x, y + PMASKDY[m], mirror, Op::And, Ink::None);
                }
            }
        }
    }
}
