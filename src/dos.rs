//! The game drawn with the DOS version's art.
//!
//! The tiles, characters, swords, flames and bubbles are the DOS game's,
//! from PrinceJS's sprite sheets (Unlicense, copied by tools/dos_art.py),
//! put together the way PrinceJS builds a room: each block has a back
//! image and a front image, 60 x 79 pixels at x = 32 * column and
//! y = 63 * row - 13; all the back images go down first (lower rooms
//! first, then rows bottom to top, left to right), then the characters,
//! then all the front images. The game itself (positions, frames, the
//! state of every gate and trap) is ours; DOS x is ours times 8/7.

use crate::game::*;
use crate::level::obj;
use crate::mover::{EMAXVAL, GMAXVAL, SPIKE_EXT};
use std::sync::OnceLock;

#[path = "gen/dos_atlas.rs"]
mod atlas;

pub const W: i32 = 320;
/// A scratch layer is filled with this unused colour, to tell what was
/// drawn on it from what is clear.
const CLEAR: [u8; 3] = [1, 2, 3];
pub const H: i32 = 192;

/// The picture: a colour per pixel, and which pixels are characters (the
/// renderer draws those in Braille).
pub struct Pic {
    pub col: Vec<[u8; 3]>,
    pub kid: Vec<bool>,
}

impl Pic {
    pub(crate) fn new() -> Pic {
        let n = (W * H) as usize;
        Pic { col: vec![[0; 3]; n], kid: vec![false; n] }
    }

    pub(crate) fn put(&mut self, x: i32, y: i32, c: [u8; 3], kid: bool) {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            let i = (y * W + x) as usize;
            self.col[i] = c;
            self.kid[i] = kid;
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c, false);
            }
        }
    }

    /// Swap line y with line H - 1 - y (the upside-down potion).
    pub fn flip_vertical(&mut self) {
        let w = W as usize;
        for y in 0..H as usize / 2 {
            let z = H as usize - 1 - y;
            for x in 0..w {
                self.col.swap(y * w + x, z * w + x);
                self.kid.swap(y * w + x, z * w + x);
            }
        }
    }
}

/// A decoded sprite sheet.
pub(crate) struct Sheet {
    w: usize,
    px: Vec<[u8; 4]>,
    frames: atlas::Frames,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum S {
    Dungeon,
    Palace,
    General,
    Kid,
    Shadow,
    Skeleton,
    Fatguard,
    Jaffar,
    Sword,
    Princess,
    Vizier,
    Mouse,
    Guard(u8),
}

fn decode(png: &[u8], frames: atlas::Frames) -> Sheet {
    let dec = png::Decoder::new(png);
    let mut r = dec.read_info().expect("embedded png");
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).expect("embedded png");
    let (w, h) = (info.width as usize, info.height as usize);
    let bpp = info.line_size / w;
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        let line = &buf[y * info.line_size..];
        for x in 0..w {
            let p = &line[x * bpp..];
            px.push(match bpp {
                4 => [p[0], p[1], p[2], p[3]],
                3 => [p[0], p[1], p[2], 255],
                _ => [p[0], p[0], p[0], 255],
            });
        }
    }
    Sheet { w, px, frames }
}

fn sheet(s: S) -> &'static Sheet {
    static SHEETS: OnceLock<Vec<Sheet>> = OnceLock::new();
    let all = SHEETS.get_or_init(|| {
        let mut v = vec![
            decode(include_bytes!("../assets/dos/dungeon.png"), atlas::DUNGEON),
            decode(include_bytes!("../assets/dos/palace.png"), atlas::PALACE),
            decode(include_bytes!("../assets/dos/general.png"), atlas::GENERAL),
            decode(include_bytes!("../assets/dos/kid.png"), atlas::KID),
            decode(include_bytes!("../assets/dos/shadow.png"), atlas::SHADOW),
            decode(include_bytes!("../assets/dos/skeleton.png"), atlas::SKELETON),
            decode(include_bytes!("../assets/dos/fatguard.png"), atlas::FATGUARD),
            decode(include_bytes!("../assets/dos/jaffar.png"), atlas::JAFFAR),
            decode(include_bytes!("../assets/dos/sword.png"), atlas::SWORD),
            decode(include_bytes!("../assets/dos/princess.png"), atlas::PRINCESS),
            decode(include_bytes!("../assets/dos/vizier.png"), atlas::VIZIER),
            decode(include_bytes!("../assets/dos/mouse.png"), atlas::MOUSE),
        ];
        v.push(decode(include_bytes!("../assets/dos/guard-1.png"), atlas::GUARD_1));
        v.push(decode(include_bytes!("../assets/dos/guard-2.png"), atlas::GUARD_2));
        v.push(decode(include_bytes!("../assets/dos/guard-3.png"), atlas::GUARD_3));
        v.push(decode(include_bytes!("../assets/dos/guard-4.png"), atlas::GUARD_4));
        v.push(decode(include_bytes!("../assets/dos/guard-5.png"), atlas::GUARD_5));
        v.push(decode(include_bytes!("../assets/dos/guard-6.png"), atlas::GUARD_6));
        v.push(decode(include_bytes!("../assets/dos/guard-7.png"), atlas::GUARD_7));
        v
    });
    let i = match s {
        S::Dungeon => 0,
        S::Palace => 1,
        S::General => 2,
        S::Kid => 3,
        S::Shadow => 4,
        S::Skeleton => 5,
        S::Fatguard => 6,
        S::Jaffar => 7,
        S::Sword => 8,
        S::Princess => 9,
        S::Vizier => 10,
        S::Mouse => 11,
        S::Guard(c) => 11 + c.clamp(1, 7) as usize,
    };
    &all[i]
}

/// A frame of a sheet.
#[derive(Clone, Copy)]
pub(crate) struct Img {
    s: &'static Sheet,
    x: usize,
    y: usize,
    pub w: i32,
    pub h: i32,
}

pub(crate) fn img(s: S, name: &str) -> Option<Img> {
    let sh = sheet(s);
    let i = sh.frames.binary_search_by(|f| f.0.cmp(name)).ok()?;
    let (_, x, y, w, h) = sh.frames[i];
    Some(Img { s: sh, x: x as usize, y: y as usize, w: w as i32, h: h as i32 })
}

/// How an image is laid.
#[derive(Clone, Copy)]
struct Lay {
    /// drawn right to left from x (a character facing right)
    mirror: bool,
    /// rows cut off the image's top; it is drawn that much higher
    lift: i32,
    /// draw only this many columns
    width: i32,
    kid: bool,
    /// multiplied into every pixel (Phaser's tint)
    tint: [u8; 3],
    /// the kid's colours turned into the shadow's (his white clothes
    /// black, his hair dark blue), for shadow frames PrinceJS lacks
    shadow: bool,
}

const PLAIN: Lay = Lay { mirror: false, lift: 0, width: 999, kid: false, tint: [255; 3], shadow: false };

impl Pic {
    /// Lay `im` with its top-left (or, mirrored, top-right) at (x, y).
    fn lay(&mut self, im: &Img, x: i32, y: i32, l: Lay) {
        for r in l.lift.max(0)..im.h {
            for c in 0..im.w.min(l.width) {
                let p = im.s.px[(im.y + r as usize) * im.s.w + im.x + c as usize];
                if p[3] < 128 {
                    continue;
                }
                let px = if l.mirror { x - 1 - c } else { x + c };
                let t = |k: usize| (p[k] as u16 * l.tint[k] as u16 / 255) as u8;
                let mut c = [t(0), t(1), t(2)];
                if l.shadow {
                    c = match c {
                        [r, g, _] if r > 240 && g > 240 => [0, 0, 0],
                        [187, 153, 0] => [35, 69, 118],
                        c => c,
                    };
                }
                self.put(px, y + r - l.lift, c, l.kid);
            }
        }
    }

    pub(crate) fn named(&mut self, s: S, name: &str, x: i32, y: i32) {
        if let Some(im) = img(s, name) {
            self.lay(&im, x, y, PLAIN);
        }
    }
}

/// The picture of the screen the player is looking at.
pub fn picture(g: &Game) -> Pic {
    let mut pic = Pic::new();
    let palace = g.level.palace();
    let v = View { g, s: if palace { S::Palace } else { S::Dungeon }, key: if palace { "palace" } else { "dungeon" } };
    let scrn = g.vis_scrn;
    let l = &g.level;
    let (left, up, down) = (l.left(scrn), l.up(scrn), l.down(scrn));
    // every block that shows, in PrinceJS's order: (room, column, row, x, y)
    let mut blocks: Vec<(u8, i32, i32, i32, i32)> = Vec::new();
    let room = |blocks: &mut Vec<_>, s: u8, cols: std::ops::Range<i32>, rows: &[i32], ox: i32, oy: i32| {
        for &r in rows {
            for c in cols.clone() {
                blocks.push((s, c, r, ox + c * 32, oy + r * 63 - 13));
            }
        }
    };
    if down != 0 {
        let dl = l.left(down);
        if dl != 0 {
            room(&mut blocks, dl, 9..10, &[0], -320, 189);
        }
        room(&mut blocks, down, 0..10, &[0], 0, 189);
    }
    if left != 0 {
        room(&mut blocks, left, 9..10, &[2, 1, 0], -320, 0);
    }
    room(&mut blocks, scrn, 0..10, &[2, 1, 0], 0, 0);
    if up != 0 {
        let ul = l.left(up);
        if ul != 0 {
            room(&mut blocks, ul, 9..10, &[2], -320, -189);
        }
        room(&mut blocks, up, 0..10, &[2], 0, -189);
    }
    // no room to the left: a wall; none above: a floor
    if left == 0 {
        for r in [2, 1, 0] {
            pic.named(v.s, &format!("{}_wall_0", v.key), -32, r * 63 - 13);
        }
    }
    for &(s, c, r, x, y) in &blocks {
        v.tile(&mut pic, s, c, r, x, y, false);
    }
    if up == 0 {
        for c in 0..10 {
            pic.named(v.s, &format!("{}_1", v.key), c * 32, -63 - 13);
        }
    }
    for m in &g.mobs {
        v.mob(&mut pic, m);
    }
    v.characters(&mut pic);
    v.mask(&mut pic);
    for &(s, c, r, x, y) in &blocks {
        v.tile(&mut pic, s, c, r, x, y, true);
    }
    if up == 0 {
        for c in 0..10 {
            pic.named(v.s, &format!("{}_1_fg", v.key), c * 32, -63 - 13);
        }
    }
    if g.inverted {
        pic.flip_vertical();
    }
    pic
}

struct View<'a> {
    g: &'a Game,
    s: S,
    key: &'static str,
}

impl View<'_> {
    /// The block's id and state (getobjid1: a plate shows as pressed).
    fn block(&self, scrn: u8, c: i32, r: i32) -> (u8, u8) {
        let l = &self.g.level;
        let (mut s, mut c, mut r) = (scrn, c, r);
        if c < 0 {
            s = l.left(s);
            c += 10;
        } else if c > 9 {
            s = l.right(s);
            c -= 10;
        }
        if s != 0 && r < 0 {
            s = l.up(s);
            r += 3;
        } else if s != 0 && r > 2 {
            s = l.down(s);
            r -= 3;
        }
        if s == 0 {
            return (obj::BLOCK, 0);
        }
        let i = crate::level::Level::index(s, c, r);
        let id = l.bluetype[i] & 0x1f;
        let st = l.bluespec[i];
        let down = || l.linkmap[st as usize] & 0x1f >= 2;
        match id {
            obj::PRESSPLATE if down() => (obj::DPRESSPLATE, st),
            obj::UPRESSPLATE if down() => (obj::FLOOR, 0),
            _ => (id, st),
        }
    }

    fn named(&self, pic: &mut Pic, name: &str, x: i32, y: i32) {
        pic.named(self.s, name, x, y);
    }

    /// One block's back (or front) image, with what the block holds.
    #[allow(clippy::too_many_arguments)]
    fn tile(&self, pic: &mut Pic, scrn: u8, c: i32, r: i32, x: i32, y: i32, front: bool) {
        let (id, st) = self.block(scrn, c, r);
        let k = self.key;
        let e = id as i32;
        let fg = if front { "_fg" } else { "" };
        if front && matches!(id, obj::PANELWIF | obj::PANELWOF) && scrn == self.g.vis_scrn && self.kid_in_panel(c, r) {
            return;
        }
        match id {
            obj::BLOCK => self.wall(pic, scrn, c, r, x, y, st, front),
            obj::SPACE | obj::FLOOR => {
                self.named(pic, &format!("{k}_{e}{fg}"), x, y);
                if !front {
                    self.named(pic, &format!("{k}_{e}_{st}"), x, y);
                }
            }
            obj::SPIKES => {
                // the spikes' height: out and back in over the state's frames
                const OUT: [u8; 10] = [0, 1, 2, 3, 4, 5, 4, 2, 1, 0];
                let n = if st & 0x80 != 0 { OUT[SPIKE_EXT as usize] } else { OUT[st.min(9) as usize] };
                self.named(pic, &format!("{k}_{e}{fg}"), x, y);
                self.named(pic, &format!("{k}_{e}_{n}{fg}"), x, y);
            }
            obj::GATE => {
                self.named(pic, &format!("{k}_{e}{fg}"), x, y);
                let lift = (st.min(GMAXVAL) >> 2) as i32;
                let (name, gx, gy) = if front { (format!("{k}_gate_fg"), x + 32, y + 16) } else { (format!("{k}_gate"), x, y) };
                if let Some(im) = img(self.s, &name) {
                    pic.lay(&im, gx, gy, Lay { lift, ..PLAIN });
                }
            }
            obj::PRESSPLATE | obj::DPRESSPLATE | obj::UPRESSPLATE => {
                if front {
                    self.named(pic, &format!("{k}_{e}_fg"), x, y);
                } else if id == obj::DPRESSPLATE {
                    self.named(pic, &format!("{k}_6_down"), x, y);
                } else {
                    self.named(pic, &format!("{k}_{e}"), x, y);
                }
            }
            obj::LOOSE => {
                // shaking: the board's wobble frames, no front
                // (the state counts the wobble, with bit 7 once it falls)
                let n = if st & 0x7f == 0 { 0 } else { (st & 0x7f).clamp(1, 8) };
                if n == 0 {
                    self.named(pic, &format!("{k}_{e}{fg}"), x, y);
                } else if !front {
                    self.named(pic, &format!("{k}_loose_{n}"), x, y);
                }
            }
            obj::SLICER => {
                // at rest open (5); closing and opening again over 1-5
                let n = match st & 0x7f {
                    s @ 1..=5 => s,
                    _ => 5,
                };
                self.named(pic, &format!("{k}_{e}{fg}"), x, y);
                self.named(pic, &format!("{k}_chopper_{n}{fg}"), x, y);
                if front && st & 0x80 != 0 {
                    pic.named(S::General, &format!("chopper-blood_{n}"), x + 12, y + 41);
                }
            }
            obj::TORCH | 30 => {
                self.named(pic, &format!("{k}_{e}{fg}"), x, y);
                if !front {
                    pic.named(S::General, &format!("fire_{}", st % 9 + 1), x + 40, y + 18);
                }
            }
            obj::FLASK => {
                let kind = (st >> 5).clamp(1, 5);
                let empty = st >> 5 == 0;
                if !front {
                    self.named(pic, &format!("{k}_{e}"), x, y);
                } else if empty {
                    self.named(pic, &format!("{k}_{e}_fg"), x, y);
                } else {
                    self.named(pic, &format!("{k}_{e}_fg_{kind}"), x, y);
                    const COLOR: [&str; 5] = ["red", "red", "green", "green", "blue"];
                    let by = if (2..5).contains(&kind) { 49 } else { 53 };
                    let b = (st & 0x1f) % 7 + 1;
                    pic.named(S::General, &format!("bubble_{b}_{}", COLOR[kind as usize - 1]), x + 25, y + by);
                }
            }
            obj::SWORD => {
                if front {
                    self.named(pic, &format!("{k}_{e}_fg"), x, y);
                } else {
                    let bright = if st == 1 { "_bright" } else { "" };
                    self.named(pic, &format!("{k}_{e}{bright}"), x, y);
                }
            }
            obj::EXIT2 => {
                self.named(pic, &format!("{k}_{e}{fg}"), x, y);
                if !front {
                    // the door, raised by the exit's state
                    let open = (st.min(EMAXVAL) >> 2) as i32;
                    if let Some(im) = img(self.s, &format!("{k}_door")) {
                        let dx = if self.s == S::Palace { 7 } else { 10 };
                        pic.lay(&im, x + dx, y + 12, Lay { lift: open.min(im.h - 8), ..PLAIN });
                    }
                }
            }
            obj::MIRROR => {
                // a floor with the mirror on it (PrinceJS Mirror): the
                // kid's reflection behind the glass, the cover hiding it
                // outside
                self.named(pic, &format!("{k}_1{fg}"), x, y);
                if front {
                    self.named(pic, &format!("{k}_1_fg_mirror"), x + 3, y - 3);
                    return;
                }
                self.named(pic, &format!("{k}_1_mirror"), x + 3, y - 3);
                if self.g.reflect.is_some() && scrn == self.g.vis_scrn {
                    self.character_in(pic, &self.g.kid, Some(x));
                }
                self.named(pic, &format!("{k}_1_mirror_cover"), x - 103, y - 5);
            }
            obj::PILLARBOTTOM => {
                let low = if self.block(scrn, c, r - 1).0 != obj::PILLARTOP { "_low" } else { "" };
                let name = if front { format!("{k}_{e}_fg{low}") } else { format!("{k}_{e}{low}") };
                self.named(pic, &name, x, y);
            }
            obj::PANELWIF | obj::PANELWOF if self.s == S::Palace && st > 0 => {
                self.named(pic, &format!("{k}_{e}_{st}{fg}"), x, y);
            }
            obj::WINDOW2 if self.s == S::Palace => {
                self.named(pic, &format!("{k}_{e}{fg}"), x, y);
                if !front {
                    self.named(pic, &format!("{k}_balcony"), x, y - 4);
                }
            }
            _ => self.named(pic, &format!("{k}_{e}{fg}"), x, y),
        }
    }

    /// The kid bumped into a panel facing right stands inside its picture
    /// (COLL.S puts him there; the Apple and DOS games both hide him behind
    /// its front). He is kept in view: the panel's front is left out while
    /// most of him is in its block.
    fn kid_in_panel(&self, c: i32, r: i32) -> bool {
        let k = &self.g.kid;
        if k.face & 0x80 != 0 || k.scrn != self.g.vis_scrn || k.block_y as i8 as i32 != r {
            return false;
        }
        let (f, _) = crate::bg::setupchar(k);
        let x = (f.x * 8).div_euclid(7);
        // facing right he is drawn leftwards from x, about 20 pixels wide;
        // the panel's picture reaches about 16 pixels into the next block
        let (l, rr) = (x - 20, x);
        let overlap = rr.min(c * 32 + 48) - l.max(c * 32);
        overlap >= 6
    }

    /// A wall block: the dungeon's stones by seed and neighbours, the
    /// palace's coloured courses (LevelBuilder.buildTile).
    #[allow(clippy::too_many_arguments)]
    fn wall(&self, pic: &mut Pic, scrn: u8, c: i32, r: i32, x: i32, y: i32, st: u8, front: bool) {
        let wl = self.block(scrn, c - 1, r).0 == obj::BLOCK;
        let wr = self.block(scrn, c + 1, r).0 == obj::BLOCK;
        let seed = r * 10 + c + scrn as i32;
        if !front {
            if !wr {
                self.named(pic, &format!("{}_wall_{}", self.key, st.min(1)), x, y);
            }
            return;
        }
        if self.s == S::Dungeon {
            let t = format!("{}W{}", if wl { "W" } else { "S" }, if wr { "W" } else { "S" });
            self.named(pic, &format!("{t}_{seed}"), x, y);
        } else {
            let p = wall_pattern(scrn);
            const COLORS: [[u8; 3]; 8] = [
                [0xd8, 0xa8, 0x58],
                [0xe0, 0xa4, 0x5c],
                [0xe0, 0xa8, 0x60],
                [0xd8, 0xa0, 0x54],
                [0xe0, 0xa4, 0x5c],
                [0xd8, 0xa4, 0x58],
                [0xe0, 0xa8, 0x58],
                [0xd8, 0xa8, 0x60],
            ];
            let (cc, rr) = (c as usize, r as usize);
            let at = |sub: usize, col: usize| COLORS[p[44 * rr + 11 * sub + col] as usize];
            pic.rect(x, y + 16, 32, 20, at(0, cc));
            pic.rect(x, y + 36, 16, 21, at(1, cc));
            pic.rect(x + 16, y + 36, 16, 21, at(1, cc + 1));
            pic.rect(x, y + 57, 8, 19, at(2, cc));
            pic.rect(x + 8, y + 57, 24, 19, at(2, cc + 1));
            pic.rect(x, y + 76, 32, 3, at(3, cc));
            self.named(pic, &format!("W_{seed}"), x, y + 16);
        }
    }

    /// A loose floor falling (MOB): the falling board at its place.
    fn mob(&self, pic: &mut Pic, m: &Mob) {
        let g = self.g;
        let y = if m.scrn == g.vis_scrn {
            m.y as i32
        } else if m.scrn == g.level.down(g.vis_scrn) && (m.y >= 0xef || m.y < 17) {
            m.y as i8 as i32 + 189
        } else {
            return;
        };
        // Apple x is in 7-pixel bytes; the board's image sits like a tile
        self.named(pic, &format!("{}_falling", self.key), m.x as i32 * 8, y - 63 - 13 + 3);
    }

    /// A kid hanging facing right goes behind the floor he hangs from:
    /// that block's back image again, cut to the block itself (PrinceJS
    /// maskTile), for the floors QUICKFLOOR marks with open space to their
    /// left (FRAMEADV.S drawfloor). Climbing up he has frames of his own.
    fn mask(&self, pic: &mut Pic) {
        let g = self.g;
        let k = &g.kid;
        // hanging, swinging and the hang at the end of a jump (79)
        let hanging = (87..=100).contains(&k.posn) || k.posn == 79;
        if k.face & 0x80 != 0 || !hanging || k.scrn != g.vis_scrn {
            return;
        }
        let marks = crate::bg::front_floors(g, k);
        for (i, &m) in marks.iter().enumerate() {
            let (c, r) = ((i % 10) as i32, (i / 10) as i32);
            if m != 1 || self.block(g.vis_scrn, c - 1, r).0 != obj::SPACE {
                continue;
            }
            let mut tile = Pic::new();
            tile.col.fill(CLEAR);
            self.tile(&mut tile, g.vis_scrn, c, r, c * 32, r * 63 - 13, false);
            let (x0, y0) = (c * 32, (r * 63 - 13).max(0));
            for y in y0..(r * 63 + 66).min(H) {
                for x in x0..(x0 + 33).min(W) {
                    let i = (y * W + x) as usize;
                    if tile.col[i] != CLEAR {
                        pic.col[i] = tile.col[i];
                        pic.kid[i] = false;
                    }
                }
            }
        }
    }

    fn character(&self, pic: &mut Pic, c: &Char) {
        self.character_in(pic, c, None);
    }

    /// The kid, the guard or shadow, and their swords.
    fn characters(&self, pic: &mut Pic) {
        let g = self.g;
        let mut chars: Vec<&Char> = Vec::new();
        let sh = &g.shad;
        if sh.face != crate::guard::NO_CHAR && sh.scrn == g.vis_scrn && sh.scrn != 0 {
            chars.push(sh);
        }
        if g.kid.scrn == g.vis_scrn && g.kid.scrn != 0 {
            chars.push(&g.kid);
        }
        // sortlist: the one lower on the screen goes behind
        if chars.len() == 2 && chars[0].y < chars[1].y && chars[0].id != 1 {
            chars.swap(0, 1);
        }
        for c in chars {
            self.character(pic, c);
        }
    }

    /// Draw a character; with `mirror_at`, his reflection in the mirror
    /// block at that x instead (PrinceJS Mirror.syncFrame: mirrored about
    /// the glass, shown only near it).
    fn character_in(&self, pic: &mut Pic, c: &Char, mirror_at: Option<i32>) {
        let g = self.g;
        let (f, frame) = crate::bg::setupchar(c);
        let mut right = c.face & 0x80 == 0;
        let posn = if c.id >= 2 && (102..107).contains(&c.posn) { c.posn + 70 } else { c.posn };
        let s = match (c.id, g.level.number) {
            (0, _) => S::Kid,
            (1, _) => S::Shadow,
            (4, _) => S::Skeleton,
            (24, _) => S::Mouse,
            (_, 6) => S::Fatguard,
            (_, 13) => S::Jaffar,
            (_, n) => {
                let colors = atlas::GUARD_COLORS.get(n as usize).copied().unwrap_or(&[]);
                S::Guard(colors.iter().find(|(r, _)| *r == c.scrn).or(colors.first()).map_or(1, |&(_, k)| k))
            }
        };
        // the shadow's missing frames (hanging, climbing, the stairs): the
        // kid's, in the shadow's colours
        let (im, shadow) = match sprite(s, c.id, posn, right) {
            Some(im) => (im, false),
            None if s == S::Shadow => match sprite(S::Kid, 0, posn, right) {
                Some(im) => (im, true),
                None => return,
            },
            None => return,
        };
        let mut x = (f.x * 8).div_euclid(7);
        if let Some(tx) = mirror_at {
            let rx = (x - tx - 55).max(if right { -10 } else { -25 });
            if !(-40..20).contains(&rx) || rx == -40 {
                return;
            }
            x = tx - rx;
            right = !right;
        }
        let y = f.y as i32 + 3 - im.h;
        let kid = true;
        let face = if right { 1 } else { -1 };
        pic.lay(&im, x, y, Lay { kid, shadow, ..PLAIN }.flip(right));
        // the splash on a character just hurt (PrinceJS showSplash; the
        // Apple's impact star), none for the skeleton or the dead
        let hurt = if c.id == 0 { g.chg_kid_str } else { g.chg_opp_str } & 0x80 != 0;
        let dead = matches!(posn, 177 | 178 | 185) || (106..=110).contains(&posn);
        if hurt && !dead && s != S::Skeleton {
            let who = match s {
                S::Kid => "kid",
                S::Shadow => "shadow",
                S::Fatguard => "fatguard",
                S::Jaffar => "jaffar",
                _ => "guard",
            };
            // a guard's splash is white, tinted his colour (Enemy.COLOR)
            const COLOR: [[u8; 3]; 7] = [
                [0x48, 0x90, 0xfc],
                [0xa8, 0x30, 0x00],
                [0xfc, 0x50, 0x00],
                [0x0c, 0x90, 0x00],
                [0x5a, 0x00, 0xfc],
                [0xc8, 0x58, 0xfc],
                [0xfc, 0xfc, 0x00],
            ];
            let tint = if let S::Guard(k) = s { COLOR[k.clamp(1, 7) as usize - 1] } else { [255; 3] };
            if let Some(sp) = img(S::General, &format!("{who}-splash")) {
                pic.lay(&sp, x + 6 * face, f.y as i32 + 3 - 15 - sp.h, Lay { kid, tint, ..PLAIN }.flip(right));
            }
        }
        // the sword in his hand (SETUPSWORD's rule, the DOS sword table)
        let shows = (c.id == 2 && c.life & 0x80 != 0) || (229..238).contains(&c.posn) || c.sword != 0;
        let n = (frame.sword & 0x3f) as usize;
        if shows && n > 0 {
            if let Some(&(id, dx, dy)) = atlas::SWORDTAB.get(n - 1) {
                if let Some(sw) = img(S::Sword, &format!("sword{id}")) {
                    let sx = x + dx as i32 * face;
                    let sy = f.y as i32 + 3 + dy as i32 - sw.h;
                    pic.lay(&sw, sx, sy, Lay { kid, ..PLAIN }.flip(right));
                }
            }
        }
    }
}

/// A character's sprite for frame `posn`. PrinceJS leaves out a few frames
/// whose image another frame shares (the kid's 161 is 150's, 170 and 171
/// are 158's); those borrow that frame's sprite.
fn sprite(s: S, id: u8, posn: u8, right: bool) -> Option<Img> {
    let name = |p: u8| match s {
        S::Kid => {
            let r = if right && (135..=144).contains(&p) { "r" } else { "" };
            format!("kid-{p}{r}")
        }
        S::Shadow => format!("shadow-{p}"),
        S::Skeleton => format!("skeleton-{}", p.wrapping_sub(150)),
        S::Mouse => format!("mouse-{}", p.wrapping_sub(185)),
        S::Fatguard => format!("fatguard-{}", p.wrapping_sub(150)),
        S::Jaffar => format!("jaffar-{}", p.wrapping_sub(150)),
        S::Guard(k) => format!("guard-{}-{}", k.clamp(1, 7), p.wrapping_sub(150)),
        _ => String::new(),
    };
    if let Some(im) = img(s, &name(posn)) {
        return Some(im);
    }
    let image = |p: u8| {
        let f = crate::seq::frame(id, p);
        (f.image, f.sword & 0xc0)
    };
    let want = image(posn);
    if want.0 == 0 {
        return None;
    }
    (1..=240u8).filter(|&p| p != posn && image(p) == want).find_map(|p| img(s, &name(p)))
}

impl Lay {
    /// Sprites face left; facing right they are mirrored about x.
    fn flip(self, right: bool) -> Lay {
        Lay { mirror: right, ..self }
    }
}

/// generateWallPattern: the palace wall's course colours for a room.
fn wall_pattern(room: u8) -> [u8; 132] {
    let mut seed = room as u32;
    let mut prandom = |max: u32| {
        seed = seed.wrapping_mul(214013).wrapping_add(2531011);
        (seed >> 16) % (max + 1)
    };
    prandom(1);
    let mut p = [0u8; 132];
    for row in 0..3 {
        for sub in 0..4 {
            let base = if sub % 2 == 1 { 0 } else { 4 };
            let mut prev = -1i32;
            for col in 0..=10 {
                let mut color;
                loop {
                    color = base + prandom(3) as i32;
                    if color != prev {
                        break;
                    }
                }
                p[44 * row + 11 * sub + col] = color as u8;
                prev = color;
            }
        }
    }
    p
}

/// A character in the princess's room, his feet `dy` lines lower than the
/// game's own rows put them: the princess, the vizier, the mouse or the
/// kid, from their DOS sheets.
pub(crate) fn cut_character(pic: &mut Pic, c: &Char, dy: i32) {
    let (f, _) = crate::bg::setupchar(c);
    let (s, name) = match c.id {
        5 => (S::Princess, format!("princess-{}", c.posn)),
        6 => (S::Vizier, format!("vizier-{}", c.posn.wrapping_sub(47))),
        24 => (S::Mouse, format!("mouse-{}", c.posn.wrapping_sub(185))),
        _ => (S::Kid, format!("kid-{}", c.posn)),
    };
    let Some(im) = img(s, &name) else { return };
    let right = c.face & 0x80 == 0;
    let x = (f.x * 8).div_euclid(7);
    pic.lay(&im, x, f.y as i32 + dy - im.h, Lay { kid: true, ..PLAIN }.flip(right));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kid_pixels(g: &Game) -> usize {
        picture(g).kid.iter().filter(|&&k| k).count()
    }

    #[test]
    fn every_frame_with_an_image_has_a_sprite() {
        // not drawn by anyone: the kid's 186-188 are the mouse's, 206 is
        // unused, the skeleton never collapses (179-185), and the vizier
        // meets no spikes or slicer (177, 178); the shadow borrows the kid's
        let skip = |s: S, p: u8| match s {
            S::Kid | S::Shadow => matches!(p, 186..=188 | 206),
            S::Skeleton => (179..=185).contains(&p),
            S::Jaffar => matches!(p, 177 | 178),
            _ => false,
        };
        let mut missing = Vec::new();
        let sheets = [(S::Kid, 0, 1..=240), (S::Shadow, 1, 1..=240), (S::Guard(1), 2, 150..=189), (S::Skeleton, 4, 150..=189), (S::Fatguard, 2, 150..=189), (S::Jaffar, 2, 150..=189)];
        for (s, id, range) in sheets {
            for p in range {
                let found = sprite(s, id, p, false).is_some() || (s == S::Shadow && sprite(S::Kid, 0, p, false).is_some());
                if crate::seq::frame(id, p).image != 0 && !skip(s, p) && !found {
                    missing.push((id, p));
                }
            }
        }
        assert!(missing.is_empty(), "no sprite for {missing:?}");
    }

    #[test]
    fn the_kid_is_drawn_from_his_sheet() {
        let mut g = Game::new(2);
        for _ in 0..30 {
            g.tick(Controls::default());
        }
        assert!(kid_pixels(&g) > 200, "the standing kid is a person-sized sprite");
    }

    #[test]
    fn the_kid_stays_in_view_at_a_palace_panel() {
        // level 10 screen 1: run right into the tapestry panel
        let mut g = Game::new(10);
        let right = Controls { x: 1, y: 0, btn: false };
        for _ in 0..30 {
            g.tick(Controls::default());
        }
        for _ in 0..36 {
            g.tick(right);
        }
        for _ in 0..10 {
            g.tick(Controls::default());
        }
        assert_eq!(g.kid.block_x, 7);
        assert!(kid_pixels(&g) > 200, "the panel's front leaves him in view");
    }
}
