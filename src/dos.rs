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
pub const H: i32 = 192;

/// The picture: a colour per pixel, and which pixels are characters (the
/// renderer draws those in Braille).
pub struct Pic {
    pub col: Vec<[u8; 3]>,
    pub kid: Vec<bool>,
}

impl Pic {
    fn new() -> Pic {
        let n = (W * H) as usize;
        Pic { col: vec![[0; 3]; n], kid: vec![false; n] }
    }

    fn put(&mut self, x: i32, y: i32, c: [u8; 3], kid: bool) {
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
struct Sheet {
    w: usize,
    px: Vec<[u8; 4]>,
    frames: atlas::Frames,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Dungeon,
    Palace,
    General,
    Kid,
    Shadow,
    Skeleton,
    Fatguard,
    Jaffar,
    Sword,
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
        S::Guard(c) => 8 + c.clamp(1, 7) as usize,
    };
    &all[i]
}

/// A frame of a sheet.
#[derive(Clone, Copy)]
struct Img {
    s: &'static Sheet,
    x: usize,
    y: usize,
    w: i32,
    h: i32,
}

fn img(s: S, name: &str) -> Option<Img> {
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
}

const PLAIN: Lay = Lay { mirror: false, lift: 0, width: 999, kid: false };

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
                self.put(px, y + r - l.lift, [p[0], p[1], p[2]], l.kid);
            }
        }
    }

    fn named(&mut self, s: S, name: &str, x: i32, y: i32) {
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
                let n = if st & 0x80 != 0 { (st & 0x7f).clamp(1, 8) } else { 0 };
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

    fn character(&self, pic: &mut Pic, c: &Char) {
        let g = self.g;
        let (f, frame) = crate::bg::setupchar(c);
        let right = c.face & 0x80 == 0;
        let posn = if c.id >= 2 && (102..107).contains(&c.posn) { c.posn + 70 } else { c.posn };
        let (s, name) = match c.id {
            0 => {
                let r = if right && (135..=144).contains(&posn) { "r" } else { "" };
                (S::Kid, format!("kid-{posn}{r}"))
            }
            1 => (S::Shadow, format!("shadow-{posn}")),
            4 => (S::Skeleton, format!("skeleton-{}", posn.wrapping_sub(150))),
            _ => {
                let s = match g.level.number {
                    6 => S::Fatguard,
                    13 => S::Jaffar,
                    n => {
                        let colors = atlas::GUARD_COLORS.get(n as usize).copied().unwrap_or(&[]);
                        S::Guard(colors.iter().find(|(r, _)| *r == c.scrn).or(colors.first()).map_or(1, |&(_, k)| k))
                    }
                };
                let name = match s {
                    S::Fatguard => "fatguard".to_string(),
                    S::Jaffar => "jaffar".to_string(),
                    S::Guard(k) => format!("guard-{}", k.clamp(1, 7)),
                    _ => unreachable!(),
                };
                (s, format!("{name}-{}", posn.wrapping_sub(150)))
            }
        };
        let Some(im) = img(s, &name) else { return };
        let x = (f.x * 8).div_euclid(7);
        let y = f.y as i32 + 3 - im.h;
        let kid = true;
        pic.lay(&im, x, y, Lay { kid, ..PLAIN }.flip(right));
        // the sword in his hand (SETUPSWORD's rule, the DOS sword table)
        let shows = (c.id == 2 && c.life & 0x80 != 0) || (229..238).contains(&c.posn) || c.sword != 0;
        let n = (frame.sword & 0x3f) as usize;
        if shows && n > 0 {
            if let Some(&(id, dx, dy)) = atlas::SWORDTAB.get(n - 1) {
                if let Some(sw) = img(S::Sword, &format!("sword{id}")) {
                    let face = if right { 1 } else { -1 };
                    let sx = x + dx as i32 * face;
                    let sy = f.y as i32 + 3 + dy as i32 - sw.h;
                    pic.lay(&sw, sx, sy, Lay { kid, ..PLAIN }.flip(right));
                }
            }
        }
    }
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
