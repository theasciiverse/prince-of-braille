//! Drawing a screen: a port of FRAMEADV.S (SURE and the piece routines)
//! using the piece tables from BGDATA.S.
//!
//! Every block is built from sections: A (the block itself, lower-left at
//! BlockLeft, BlockBot-3), B (the part of the block to the LEFT that shows
//! in this block), C (the part of the block below-left that pokes up), D
//! (the floor's front edge) and a front piece drawn over characters.

use crate::apple::{self, Image, Ink, Op, Screen};
use crate::game::*;
use crate::level::obj;
use crate::mover::{EMAXVAL, FFALLING, GMAXVAL, SPIKE_EXT, TORCH_LAST};

// BGDATA.S, indexed by piece id 0-29
const MASKA: [u8; 30] = hex(
    "00,03,03,03,03,03,03,03,03,00,03,03,00,03,03,03,03,00,00,03,00,03,00,03,00,03,00,00,00,00",
);
const PIECEA: [u8; 30] = hex(
    "00,01,05,07,0a,01,01,0a,10,00,01,00,00,14,20,4b,01,00,00,01,00,97,00,01,00,a7,a9,aa,ac,ad",
);
const PIECEAY: [i8; 30] = [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -4, -4, -4];
const MASKB: [u8; 30] = hex(
    "00,04,04,04,04,04,04,00,04,00,04,00,00,04,04,04,00,04,04,04,04,04,04,00,04,04,00,00,00,00",
);
const PIECEB: [u8; 30] = hex(
    "00,02,06,08,0b,1b,02,9e,1a,1c,02,00,9e,4a,21,1b,4d,4e,02,51,84,98,02,91,92,02,00,00,00,00",
);
const PIECEBY: [i8; 30] = [0, 0, 0, 0, 0, 1, 0, 3, 0, 3, 0, 0, 3, 0, 0, -1, 0, 0, 0, -1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const BSTRIPE: [u8; 30] = hex(
    "00,47,47,00,00,47,47,00,00,00,47,47,00,00,47,47,00,00,47,00,00,00,47,00,00,47,00,00,00,00",
);
const PIECEC: [u8; 30] = hex(
    "00,00,00,09,0c,00,00,9f,00,1d,00,00,9f,00,00,00,4f,50,00,00,85,00,00,93,94,00,00,00,00,00",
);
const PIECED: [u8; 30] = hex(
    "00,15,15,15,15,18,19,16,15,00,15,00,17,15,2e,4c,15,15,15,15,86,15,15,15,15,15,ab,00,00,00",
);
const FRONTI: [u8; 30] = hex(
    "00,00,00,45,46,00,00,46,48,49,87,00,46,0f,13,00,00,00,00,00,83,00,00,00,00,a8,00,ae,ae,ae",
);
const FRONTY: [i8; 30] = [0, 0, 0, -1, 0, 0, 0, 0, -1, 3, -3, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, -36, -36, -36];
const FRONTX: [u8; 30] = hex(
    "00,00,00,01,03,00,00,03,01,01,02,00,03,01,00,00,00,00,00,00,00,00,00,00,00,01,00,00,00,00",
);

const GATEBOT_STA: u8 = 0x43;
const GATEBOT_ORA: u8 = 0x44;
const GATE_B1: u8 = 0x37;
const GATECMASK: u8 = 0x0d;
const GATE8C: [u8; 8] = [0x2f, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36];
const GATE8B: [u8; 8] = [0x3e, 0x3d, 0x3c, 0x3b, 0x3a, 0x39, 0x38, 0x37];
const STAIRS: u8 = 0x6b;
const DOOR: u8 = 0x6c;
const DOORMASK: u8 = 0x6d;
const TOPREPAIR: u8 = 0x6e;
const ARCHPANEL: u8 = 0xa1;
const SPIKEA: [u8; 10] = [0x00, 0x22, 0x24, 0x26, 0x28, 0x2a, 0x28, 0x24, 0x22, 0x00];
const SPIKEB: [u8; 10] = [0x00, 0x23, 0x25, 0x27, 0x29, 0x2b, 0x29, 0x25, 0x23, 0x00];
const LOOSEB: u8 = 0x1b;
const LOOSEA: [u8; 11] = [0x01, 0x1e, 0x01, 0x1f, 0x1f, 0x01, 0x01, 0x01, 0x1f, 0x1f, 0x1f];
const LOOSEBY: [i8; 11] = [0, 1, 0, -1, -1, 0, 0, 0, -1, -1, -1];
const LOOSED: [u8; 11] = [0x15, 0x2c, 0x15, 0x2d, 0x2d, 0x15, 0x15, 0x15, 0x2d, 0x2d, 0x2d];
const SPECIALFLASK: u8 = 0x95;
const SWORDGLEAM1: u8 = 0xb3;
const SWORDGLEAM0: u8 = 0x99;
const PANELB: [u8; 3] = [0x9e, 0x9a, 0x81];
const PANELC: [u8; 3] = [0x9f, 0x9b, 0x82];
const PANELB0: u8 = 0x9e;
const PANELC0: u8 = 0x9f;
const SPACEB: [u8; 4] = [0x00, 0xa3, 0xa5, 0xa6];
const SPACEBY: [i8; 4] = [0, -20, -20, 0];
const FLOORB: [u8; 4] = [0x02, 0xa2, 0xa4, 0xa4];
const BLOCKB: [u8; 2] = [0x84, 0x6f];
const BLOCKC: [u8; 2] = [0x85, 0x85];
const BLOCKD: [u8; 2] = [0x86, 0x86];
const BLOCKFR: [u8; 2] = [0x83, 0x83];
// GAMEBG.S
const TORCHFLAME: [u8; 18] = [
    0x52, 0x53, 0x54, 0x55, 0x56, 0x61, 0x62, 0x63, 0x64, 0x52, 0x54, 0x56, 0x52, 0x53, 0x54, 0x55, 0x56, 0x61,
];
// Slicer animation frames (BGDATA.S)
const SLICERSEQ: [u8; 7] = [4, 3, 1, 2, 5, 4, 4];
const SLICERTOP: [u8; 5] = [0x00, 0x58, 0x5a, 0x5c, 0x5e];
const SLICERBOT: [u8; 5] = [0x57, 0x59, 0x5b, 0x5d, 0x5f];
const SLICERBOT2: [u8; 5] = [0x8e, 0x8f, 0x90, 0x5d, 0x5f];
const SLICERGAP: [u8; 5] = [0, 38, 46, 53, 55];
const SLICERFRNT: [u8; 5] = [0x65, 0x66, 0x67, 0x68, 0x69];
const BUBBLE: [u8; 9] = [0xb2, 0xaf, 0xb0, 0xb1, 0xb0, 0xaf, 0xb1, 0xb0, 0xaf];

const fn hexdigit(c: u8) -> u8 {
    if c <= b'9' {
        c - b'0'
    } else {
        c - b'a' + 10
    }
}

const fn hex<const N: usize>(s: &str) -> [u8; N] {
    let b = s.as_bytes();
    let mut out = [0u8; N];
    let (mut i, mut n) = (0, 0);
    while i + 1 < b.len() {
        out[n] = hexdigit(b[i]) << 4 | hexdigit(b[i + 1]);
        n += 1;
        i += 3;
    }
    out
}

/// What we draw with.
struct Painter<'a> {
    scr: &'a mut Screen,
    bg1: &'static apple::ImageTable,
    bg2: &'static apple::ImageTable,
}

impl Painter<'_> {
    fn image(&self, id: u8) -> Option<Image<'static>> {
        if id & 0x80 != 0 {
            self.bg2.get(id & 0x7f)
        } else {
            self.bg1.get(id)
        }
    }

    /// `xco` in bytes (7 px), `offset` in pixels, `yco` bottom line.
    fn add(&mut self, id: u8, xco: i32, yco: i32, op: Op, ink: Ink) {
        self.add_off(id, xco, 0, yco, op, ink);
    }

    fn add_off(&mut self, id: u8, xco: i32, off: i32, yco: i32, op: Op, ink: Ink) {
        if id == 0 {
            return;
        }
        if let Some(img) = self.image(id) {
            self.scr.lay(&img, xco * 7 + off, yco, false, op, ink);
        }
    }
}

/// Per-block drawing context (the FRAMEADV.S locals).
#[derive(Clone, Copy, Default)]
struct Blk {
    objid: u8,
    state: u8,
    preced: u8,
    spreced: u8,
    below: u8,
    sbelow: u8,
    xco: i32,
    ay: i32,
    dy: i32,
    colno: i32,
    rowno: i32,
}

/// Y-coords are bytes in the original; keep the wrap for the screen above.
fn yb(v: u8) -> i32 {
    if v >= 0xf0 {
        v as i32 - 256
    } else {
        v as i32
    }
}

pub struct View {
    pub palace: bool,
}

/// getobjid1: what a block looks like (depressed plates, null screen).
fn objid(g: &Game, scrn: u8, bx: i32, by: i32) -> (u8, u8) {
    if scrn == 0 {
        return (obj::SPACE, 0);
    }
    let i = crate::level::Level::index(scrn, bx, by);
    let id = g.level.bluetype[i] & 0x1f;
    let state = g.level.bluespec[i];
    let down = || g.level.linkmap[state as usize] & 0x1f >= 2;
    match id {
        obj::PRESSPLATE => (if down() { obj::DPRESSPLATE } else { obj::PRESSPLATE }, state),
        obj::UPRESSPLATE => {
            if down() {
                (obj::FLOOR, 0)
            } else {
                (obj::UPRESSPLATE, state)
            }
        }
        _ => (id, state),
    }
}

/// Draw the visible screen, the kid and the front pieces.
pub fn draw(scr: &mut Screen, g: &Game) {
    if let Some(cut) = &g.cut {
        crate::room::draw(scr, g, cut);
        return;
    }
    scr.clear();
    let palace = g.level.palace();
    let mut p = Painter {
        scr,
        bg1: if palace { &apple::BGTAB1_PAL } else { &apple::BGTAB1_DUN },
        bg2: if palace { &apple::BGTAB2_PAL } else { &apple::BGTAB2_DUN },
    };
    let v = View { palace };
    let scrn = g.vis_scrn;
    let l = &g.level;
    let left = l.left(scrn);
    let above = l.up(scrn);
    let below = l.down(scrn);

    // getprev: rightmost column of the screen to the left
    let prev = |row: i32| -> (u8, u8) {
        if left == 0 {
            (obj::BLOCK, 0)
        } else {
            objid(g, left, 9, row)
        }
    };
    // getbelow: the 10 blocks below and one to the left
    let getbelow = |s: u8, sleft: u8, row: i32| -> [(u8, u8); 10] {
        let mut out = [(obj::SPACE, 0u8); 10];
        if row < 2 {
            out[0] = if left == 0 { (obj::BLOCK, 0) } else { objid(g, left, 9, row + 1) };
            for (x, o) in out.iter_mut().enumerate().skip(1) {
                *o = objid(g, s, x as i32 - 1, row + 1);
            }
        } else {
            let sb = l.down(s);
            for (x, o) in out.iter_mut().enumerate().skip(1) {
                *o = if sb == 0 { (obj::FLOOR, 0) } else { objid(g, sb, x as i32 - 1, 0) };
            }
            let sbl = if sleft == 0 { 0 } else { l.down(sleft) };
            out[0] = if sbl == 0 {
                if l.number == 12 {
                    (obj::SPACE, 0)
                } else {
                    (obj::BLOCK, 0)
                }
            } else {
                objid(g, sbl, 9, 0)
            };
        }
        out
    };

    let mut fronts: Vec<Blk> = Vec::new();
    // drawexitb's doortop, for CROPCHAR
    let mut doortop = None;
    for row in (0..3).rev() {
        let dy = BLOCK_BOT[row as usize + 1] as i32;
        let (mut preced, mut spreced) = prev(row);
        let bl = getbelow(scrn, left, row);
        let _ = below;
        for col in 0..10 {
            let (id, st) = objid(g, scrn, col, row);
            let b = Blk {
                objid: id,
                state: st,
                preced,
                spreced,
                below: bl[col as usize].0,
                sbelow: bl[col as usize].1,
                xco: col * 4,
                ay: dy - 3,
                dy,
                colno: col,
                rowno: row,
            };
            redblocksure(&mut p, &v, g, &b);
            if id == obj::EXIT && b.xco < 36 && b.dy >= 67 {
                let gateposn = (st.min(EMAXVAL) >> 2) as i32;
                doortop = Some(b.ay - 14 - gateposn);
            }
            fronts.push(b);
            preced = id;
            spreced = st;
        }
    }
    // bottom row of the screen above: D-sections only
    if scrn != 0 {
        let aleft = l.left(above);
        let bl = getbelow_above(g, scrn, left);
        let mut preced = 0u8;
        let mut spreced = 0u8;
        for col in 0..10 {
            let (id, st) = if above == 0 { (obj::FLOOR, 0) } else { objid(g, above, col, 2) };
            let b = Blk {
                objid: id,
                state: st,
                preced,
                spreced,
                below: bl[col as usize].0,
                sbelow: bl[col as usize].1,
                xco: col * 4,
                ay: -1,
                dy: 2,
                colno: col,
                rowno: -1,
            };
            let _ = aleft;
            drawc(&mut p, &b);
            drawmc(&mut p, &b);
            drawb(&mut p, &v, &b);
            drawd(&mut p, &b);
            drawmd(&mut p, &b);
            fronts.push(b);
            preced = id;
            spreced = st;
        }
    }

    // falling floors
    for m in &g.mobs {
        draw_mob(&mut p, g, m);
    }

    // the reflection, the shadowman or guard, then the kid (addchars)
    draw_reflection(p.scr, g);
    let mut objs = Objs::default();
    add_shad(&mut objs, g, doortop);
    add_kid(&mut objs, g, doortop);

    // the mid plane, block by block as FAST draws it: characters off the
    // screen's blocks first, then each block's marked floorpiece and the
    // characters indexed to it, bottom row to top, left to right; the
    // impact stars last
    objs.draw(&mut p, g, 30);
    for b in fronts.iter().filter(|b| b.rowno >= 0) {
        let i = (b.rowno * 10 + b.colno) as usize;
        match objs.floors[i] {
            1 => drawfloor(&mut p, b),
            2 => drawhalf(&mut p, &v, b),
            _ => {}
        }
        objs.draw(&mut p, g, i as u8);
    }
    objs.draw(&mut p, g, 0xff);

    // front pieces over everything
    for b in &fronts {
        drawfrnt(&mut p, &v, g, b);
    }

    // the upside-down potion (HIRES.S INVERTY swaps the line addresses)
    if g.inverted {
        p.scr.flip_vertical();
    }
}

/// For the screen-above row, BELOW is this screen's top row (one to the left).
fn getbelow_above(g: &Game, scrn: u8, left: u8) -> [(u8, u8); 10] {
    let mut out = [(obj::SPACE, 0u8); 10];
    out[0] = if left == 0 { (obj::BLOCK, 0) } else { objid(g, left, 9, 0) };
    for (x, o) in out.iter_mut().enumerate().skip(1) {
        *o = objid(g, scrn, x as i32 - 1, 0);
    }
    out
}

fn redblocksure(p: &mut Painter, v: &View, g: &Game, b: &Blk) {
    drawc(p, b);
    drawmc(p, b);
    drawb(p, v, b);
    drawmb(p, g, b);
    drawd(p, b);
    drawmd(p, b);
    drawa(p, b);
    drawma(p, b);
}

fn checkc(id: u8) -> bool {
    matches!(id, obj::SPACE | obj::PILLARTOP | obj::PANELWOF) || id >= obj::ARCHTOP1
}

fn drawc(p: &mut Painter, b: &Blk) {
    if !checkc(b.objid) {
        return;
    }
    // C-section of piece below & to left
    let x = b.below;
    let img = if x == obj::BLOCK {
        BLOCKC[if (b.sbelow as usize) < BLOCKC.len() { b.sbelow as usize } else { 0 }]
    } else {
        let c = PIECEC[x as usize % 30];
        if c == PANELC0 {
            if (b.sbelow as usize) < PANELC.len() {
                PANELC[b.sbelow as usize]
            } else {
                0
            }
        } else {
            c
        }
    };
    p.add(img, b.xco, b.dy, Op::Or, Ink::Wall);
    // mask B-section of piece to left
    let m = MASKB[b.preced as usize % 30];
    p.add(m, b.xco, b.dy, Op::And, Ink::None);
}

fn drawmc(p: &mut Painter, b: &Blk) {
    if !matches!(b.objid, obj::SPACE | obj::PANELWOF | obj::PILLARTOP) {
        return;
    }
    if b.below == obj::GATE {
        // drawgatec
        p.add(GATECMASK, b.xco, b.dy, Op::And, Ink::None);
        let st = b.sbelow.min(GMAXVAL);
        let gp = st >> 2;
        let y = (gp & 7) as usize;
        p.add(GATE8C[y], b.xco, b.dy, Op::Or, Ink::Gate);
    }
}

fn drawb(p: &mut Painter, v: &View, b: &Blk) {
    if b.objid == obj::BLOCK {
        return;
    }
    let x = b.preced as usize % 30;
    let (img, y) = match b.preced {
        obj::SPACE => {
            let s = b.spreced as usize;
            if s >= SPACEB.len() {
                return;
            }
            (SPACEB[s], SPACEBY[s] as i32)
        }
        obj::FLOOR => {
            let s = if (b.spreced as usize) < FLOORB.len() { b.spreced as usize } else { 0 };
            (FLOORB[s], 0)
        }
        obj::BLOCK => {
            let s = if (b.spreced as usize) < BLOCKB.len() { b.spreced as usize } else { 0 };
            (BLOCKB[s], PIECEBY[x] as i32)
        }
        _ => {
            let pb = PIECEB[x];
            if pb == PANELB0 {
                let s = b.spreced as usize;
                if s >= PANELB.len() {
                    return;
                }
                (PANELB[s], PIECEBY[x] as i32)
            } else {
                (pb, PIECEBY[x] as i32)
            }
        }
    };
    p.add(img, b.xco, b.ay + y, Op::Or, Ink::Wall);
    if v.palace && !matches!(b.preced, obj::SPACE | obj::FLOOR | obj::BLOCK) {
        let st = BSTRIPE[x];
        p.add(st, b.xco, b.ay - 32, Op::Or, Ink::Wall);
    }
}

fn drawmb(p: &mut Painter, g: &Game, b: &Blk) {
    match b.preced {
        obj::GATE => drawgateb(p, b),
        obj::SPIKES => {
            let x = if b.spreced & 0x80 != 0 { SPIKE_EXT } else { b.spreced.min(9) };
            p.add(SPIKEB[x as usize], b.xco, b.ay - 1, Op::Or, Ink::Floor);
        }
        obj::LOOSE => {
            let y = loosey(b.spreced);
            p.add(LOOSEB, b.xco, b.ay + LOOSEBY[y] as i32, Op::Or, Ink::Floor);
        }
        obj::TORCH => {
            if b.xco != 0 && b.spreced <= TORCH_LAST {
                // flame, from bgtable1
                p.add_off(TORCHFLAME[b.spreced as usize], b.xco + 1, 0, b.ay - 43, Op::Sta, Ink::Flame);
            }
        }
        obj::EXIT => drawexitb(p, g, b),
        _ => {}
    }
}

fn loosey(state: u8) -> usize {
    if state & 0x80 == 0 {
        (state as usize).min(LOOSEA.len() - 1)
    } else {
        let s = state & 0x7f;
        if s < FFALLING + 1 {
            s as usize
        } else {
            1
        }
    }
}

fn drawd(p: &mut Painter, b: &Blk) {
    let (img, op) = match b.objid {
        obj::BLOCK => {
            let s = if (b.state as usize) < BLOCKD.len() { b.state as usize } else { 0 };
            (BLOCKD[s], Op::Sta)
        }
        obj::PANELWOF => (PIECED[obj::PANELWOF as usize], Op::Or),
        id => (PIECED[id as usize % 30], Op::Sta),
    };
    p.add(img, b.xco, b.dy, op, Ink::Floor);
}

fn drawmd(p: &mut Painter, b: &Blk) {
    if b.objid == obj::LOOSE {
        let y = loosey(b.state);
        p.add(LOOSED[y], b.xco, b.dy, Op::Sta, Ink::Floor);
    }
}

fn piecea(b: &Blk) -> u8 {
    if b.objid == obj::LOOSE {
        LOOSEA[loosey(b.state)]
    } else {
        PIECEA[b.objid as usize % 30]
    }
}

fn drawa(p: &mut Painter, b: &Blk) {
    if b.preced == obj::ARCHTOP1 && b.objid == obj::PANELWOF {
        p.add(ARCHPANEL, b.xco, b.ay + PIECEAY[b.objid as usize] as i32, Op::Or, Ink::Wall);
        return;
    }
    if matches!(b.preced, obj::PANELWIF | obj::PANELWOF | obj::PILLARTOP | obj::BLOCK) {
        p.add(MASKA[b.objid as usize % 30], b.xco, b.ay, Op::And, Ink::None);
    }
    let ink = if matches!(b.objid, obj::FLOOR | obj::LOOSE | obj::SPIKES | obj::PRESSPLATE | obj::DPRESSPLATE | obj::UPRESSPLATE | obj::RUBBLE) {
        Ink::Floor
    } else {
        Ink::Wall
    };
    p.add(piecea(b), b.xco, b.ay + PIECEAY[b.objid as usize % 30] as i32, Op::Or, ink);
}

fn drawma(p: &mut Painter, b: &Blk) {
    match b.objid {
        obj::SPIKES => {
            let x = if b.state & 0x80 != 0 { SPIKE_EXT } else { b.state.min(9) };
            p.add(SPIKEA[x as usize], b.xco, b.ay - 1, Op::Or, Ink::Floor);
        }
        obj::FLASK => {
            // bubbles (SETUPFLASK), from bgtable2
            let kind = b.state & 0xe0;
            let mut off = 2;
            let mut y = b.ay;
            let empty = kind == 0;
            if kind == 0x40 || kind > 0x40 {
                if kind > 0x40 {
                    off += 1;
                }
                y -= 4;
            }
            let f = if empty { 0 } else { (b.state & 0x1f) as usize };
            let f = if f > 8 { 0 } else { f };
            let img = BUBBLE[f] & 0x7f;
            if let Some(i) = p.bg2.get(img) {
                p.scr.lay(&i, (b.xco + 2) * 7 + off, y - 14, false, Op::Sta, potion_ink(kind));
            }
        }
        obj::SLICER => {
            // drawslicera: bottom jaw (smeared once bloodied), then the top
            let x = slicer_frame(b.state);
            let bot = if b.state & 0x80 != 0 { SLICERBOT2[x] } else { SLICERBOT[x] };
            p.add(bot, b.xco, b.ay, Op::Or, Ink::Gate);
            p.add(SLICERTOP[x], b.xco, b.ay - SLICERGAP[x] as i32, Op::Or, Ink::Gate);
        }
        obj::SWORD => {
            let img = if b.state == 1 { SWORDGLEAM1 } else { SWORDGLEAM0 };
            p.add(img, b.xco, b.ay, Op::Sta, Ink::Sword);
        }
        _ => {}
    }
}

/// Index into the slicer piece tables for a slicer state.
fn slicer_frame(state: u8) -> usize {
    let n = (state & 0x7f).min(crate::mover::SLICER_RET);
    SLICERSEQ[n as usize] as usize - 1
}

fn potion_ink(_kind: u8) -> Ink {
    Ink::Potion
}

fn drawexitb(p: &mut Painter, g: &Game, b: &Blk) {
    if b.xco >= 36 {
        return;
    }
    if g.vis_scrn != g.level.kid_start().0 {
        p.add(STAIRS, b.xco + 1, b.ay - 12, Op::Sta, Ink::Wall);
    }
    // door, bottom to top
    let blockthr = b.dy - 67;
    if blockthr < -64 {
        return;
    }
    let gateposn = (b.spreced.min(EMAXVAL) >> 2) as i32;
    let mut y = b.ay - 14 - gateposn;
    while y >= blockthr.max(0) {
        p.add(DOORMASK, b.xco + 1, y, Op::And, Ink::None);
        p.add(DOOR, b.xco + 1, y, Op::Or, Ink::Gate);
        y -= 4;
    }
    let ty = b.ay - 64;
    if ty >= 0 {
        p.add(TOPREPAIR, b.xco + 1, ty, Op::Sta, Ink::Wall);
    }
}

fn gate_setup(b: &Blk) -> (i32, i32) {
    let blockthr = b.dy - 62;
    let gp = (b.spreced.min(GMAXVAL) >> 2) as i32 + 1;
    (blockthr, b.ay - gp)
}

fn drawgateb(p: &mut Painter, b: &Blk) {
    let (blockthr, gatebot) = gate_setup(b);
    if gatebot + 12 >= b.ay {
        p.add(GATEBOT_ORA, b.xco, gatebot - 2, Op::Or, Ink::Gate);
    } else {
        p.add(GATEBOT_STA, b.xco, gatebot, Op::Sta, Ink::Gate);
    }
    let mut y = gatebot - 12;
    loop {
        if y < -8 {
            return;
        }
        if y - 7 < blockthr {
            break;
        }
        p.add(GATE_B1, b.xco, y, Op::Sta, Ink::Gate);
        y -= 8;
    }
    let h = y - blockthr + 1;
    if (1..=8).contains(&h) {
        p.add(GATE8B[h as usize - 1], b.xco, y, Op::Sta, Ink::Gate);
    }
}

/// Gate bars drawn over the kid when he's in the gate's block.
fn drawgatebf(p: &mut Painter, b: &Blk) {
    let (blockthr, gatebot) = gate_setup(b);
    p.add(GATEBOT_ORA, b.xco, gatebot - 2, Op::Or, Ink::Gate);
    let mut y = gatebot - 12;
    while y >= -8 && y - 7 >= blockthr {
        p.add(GATE_B1, b.xco, y, Op::Or, Ink::Gate);
        y -= 8;
    }
}

fn drawfrnt(p: &mut Painter, v: &View, g: &Game, b: &Blk) {
    if b.preced == obj::GATE && b.rowno == g.kid.block_y as i8 as i32 && b.colno - 1 == g.kid.block_x as i8 as i32 && g.vis_scrn == g.kid.scrn {
        drawgatebf(p, b);
    }
    let id = b.objid;
    if id == obj::SLICER {
        // drawslicerf
        let x = slicer_frame(b.state);
        p.add(SLICERFRNT[x], b.xco, b.ay, Op::Mask, Ink::Gate);
        return;
    }
    let mut img = FRONTI[id as usize % 30];
    if id == obj::FLASK {
        let k = b.state & 0xe0;
        if k != 0xa0 && k >= 0x40 {
            img = SPECIALFLASK;
        }
    }
    if img == 0 {
        return;
    }
    if id == obj::BLOCK {
        let s = if (b.state as usize) < BLOCKFR.len() { b.state as usize } else { 0 };
        img = BLOCKFR[s];
    }
    let y = b.ay + FRONTY[id as usize % 30] as i32;
    let x = b.xco + FRONTX[id as usize % 30] as i32;
    let sta = id >= obj::ARCHTOP2 || (!v.palace && id == obj::POSTS) || id == obj::BLOCK;
    let ink = if id == obj::FLASK { Ink::Potion } else { Ink::Wall };
    if sta {
        p.add(img, x, y, Op::Sta, ink);
    } else {
        p.add(img, x, y, Op::Mask, ink);
    }
}

fn draw_mob(p: &mut Painter, g: &Game, m: &Mob) {
    let y = if m.scrn == g.vis_scrn {
        if m.y >= 192 + 17 {
            return;
        }
        m.y as i32
    } else if m.scrn == g.level.down(g.vis_scrn) && (m.y >= (-17i8) as u8 || m.y < 17) {
        yb(m.y) + 192
    } else {
        return;
    };
    let x = m.x as i32;
    let f = FFALLING as usize;
    p.add(MASKA[obj::FLOOR as usize], x, y - 3, Op::And, Ink::None);
    p.add(LOOSEA[f], x, y - 3, Op::Or, Ink::Floor);
    p.add(LOOSED[f], x, y, Op::Sta, Ink::Floor);
    p.add(LOOSEB, x + 4, y - 4, Op::Or, Ink::Floor);
}

/// A character, his sword or an impact star in the object list
/// (GAMEBG.S ADDCHAROBJ): its frame, how it is laid, its crop box
/// (FCharCL/CU/CR/CD in pixels) and the block it is drawn with.
#[derive(Clone, Copy)]
struct Obj {
    f: FChar,
    op: Op,
    ink: Ink,
    shift: i32,
    crop: (i32, i32, i32, i32),
    /// INDEXCHAR's block 0-29, 30 off the screen's blocks, $ff with the stars
    index: u8,
    /// the shadowman is always in front (FRAMEADV.S compare)
    shad: bool,
}

#[derive(Default)]
struct Objs {
    list: Vec<Obj>,
    /// QUICKFLOOR's marks per block: 1 floorbuf, 2 halfbuf
    floors: [u8; 30],
}

const ZEROCROP: (i32, i32, i32, i32) = (0, 0, apple::SCREEN_W, apple::SCREEN_H);

impl Objs {
    /// drawobjs: the objects with this index, sorted back to front
    /// (sortlist: an object lower on the screen goes behind one higher
    /// up, the shadowman in front of all).
    fn draw(&self, p: &mut Painter, g: &Game, index: u8) {
        let mut sort: Vec<&Obj> = self.list.iter().filter(|o| o.index == index).collect();
        loop {
            let mut switched = false;
            for x in (1..sort.len()).rev() {
                let (a, b) = (sort[x], sort[x - 1]);
                if !a.shad && a.f.y > b.f.y {
                    sort.swap(x, x - 1);
                    switched = true;
                }
            }
            if !switched {
                break;
            }
        }
        for o in sort {
            let Some(img) = apple::chtable(o.f.table, g.level.number).and_then(|t| t.get(o.f.image)) else { continue };
            p.scr.lay_clip(&img, o.f.x + o.shift, o.f.y as i32, o.f.face & 0x80 == 0, o.op, o.ink, o.crop);
        }
    }
}

/// SETUPKID: the kid, cropped and indexed, with his sword and impact star.
fn add_kid(objs: &mut Objs, g: &Game, doortop: Option<i32>) {
    if g.kid.scrn != g.vis_scrn || g.kid.scrn == 0 {
        return;
    }
    // after the merge on level 12 the kid flashes between kid and shadow
    let op = if g.mergetimer > 0 && g.mergetimer & 1 != 0 { Op::Eor } else { Op::Mask };
    add_char(objs, g, &g.kid, op, Ink::Kid, 0, 0, doortop, g.chg_kid_str & 0x80 != 0);
}

/// SETUPSHAD: a guard is drawn normally (shifted a pixel for the other
/// uniform colour, GAMEBG.S DrawShifted), the shadowman EORed.
fn add_shad(objs: &mut Objs, g: &Game, doortop: Option<i32>) {
    let c = &g.shad;
    if c.face == crate::guard::NO_CHAR || c.scrn != g.vis_scrn {
        return;
    }
    if c.id == 1 {
        // clipped at the left as he jumps out of the level 4 mirror
        let left = if g.level.number == 4 && c.scrn == 4 { (4 * 4 + 1) * 7 } else { 0 };
        add_char(objs, g, c, Op::Eor, Ink::Guard, 0, left, doortop, g.chg_opp_str & 0x80 != 0);
    } else {
        let shift = if c.id == 24 { 0 } else { g.guard_color as i32 & 1 };
        add_char(objs, g, c, Op::Mask, Ink::Guard, shift, 0, doortop, g.chg_opp_str & 0x80 != 0);
    }
}

#[allow(clippy::too_many_arguments)]
fn add_char(objs: &mut Objs, g: &Game, c: &Char, op: Op, ink: Ink, shift: i32, left: i32, doortop: Option<i32>, hurt: bool) {
    let (f, frame) = setupchar(c);
    let Some(e) = draw_edges(g, c, &f, frame.check) else { return };
    let index = indexchar(c, &e);
    quickfloor(c, &e, &mut objs.floors);
    let mut crop = cropchar(g, c, &f, &e, doortop);
    crop.0 = crop.0.max(left);
    let shad = c.id == 1 && op == Op::Eor;
    objs.list.push(Obj { f, op, ink, shift, crop, index, shad });
    if hurt {
        if let Some(mut star) = comix(c, &f) {
            star.index = 0xff;
            objs.list.push(star);
        }
    }
    if let Some(sf) = sword(c, &f, &frame) {
        objs.list.push(Obj { f: sf, op: Op::Mask, ink: Ink::Sword, shift: 0, crop, index, shad: false });
    }
}

/// GETEDGES (CTRLSUBS.S) for drawing: the blocks a character's image
/// covers.
struct DrawEdges {
    topej: u8,
    topblock: i32,
    bottomblock: i32,
    leftblock: i32,
    rightblock: i32,
    cd_left: u8,
    cd_right: u8,
}

/// GETBLOCKY: block row of screen line `a` (-1 above row 0).
fn getblocky(a: u8) -> i32 {
    (0..4).rev().find(|&x| a >= BLOCK_TOP[x + 1]).map_or(-1, |x| x as i32)
}

fn draw_edges(g: &Game, c: &Char, f: &FChar, check: u8) -> Option<DrawEdges> {
    let img = apple::chtable(f.table, g.level.number)?.get(f.image)?;
    let imwidth = ((img.w as u16 * 7 + 1) >> 1) as u8;
    let x140 = ((f.x >> 1) as u8).wrapping_add(SCRN_LEFT);
    let left = if c.face & 0x80 != 0 { x140 } else { x140.wrapping_sub(imwidth) };
    let right = left.wrapping_add(imwidth);
    let mut topej = f.y.wrapping_sub(img.h as u8).wrapping_add(1);
    if topej >= 192 {
        topej = 0;
    }
    let topblock = match getblocky(topej) {
        3 => -1,
        b => b,
    };
    let thin = if check & crate::seq::FTHINMARK != 0 { crate::subs::THINNER } else { 0 };
    Some(DrawEdges {
        topej,
        topblock,
        bottomblock: getblocky(f.y),
        leftblock: getblockx(left).0 as i32,
        rightblock: getblockx(right).0 as i32,
        cd_left: left.wrapping_add(thin),
        cd_right: right.wrapping_sub(thin),
    })
}

/// INDEXBLOCK: block number 0-29; a block above the screen gives its
/// column, one off the screen 30.
fn indexblock(x: i32, y: i32) -> (u8, bool) {
    if y < 0 {
        (x as u8, false)
    } else if y >= 3 || !(0..10).contains(&x) {
        (30, false)
    } else {
        ((y * 10 + x) as u8, true)
    }
}

/// INDEXCHAR: the block a character is drawn with. The test for falling
/// compares CharPosn, not CharAction, with 2, 3, 4 and 6 (A still holds
/// the position there); kept as the original has it.
fn indexchar(c: &Char, e: &DrawEdges) -> u8 {
    let (mut x, y) = if c.action == 1 {
        (e.leftblock, e.bottomblock)
    } else {
        (c.block_x as i8 as i32, c.block_y as i8 as i32)
    };
    if (135..149).contains(&c.posn) || matches!(c.posn, 2 | 3 | 4 | 6) {
        x -= 1;
    }
    indexblock(x, y).0
}

/// QUICKFLOOR: mark the floorpieces a hanging, falling or climbing
/// character may overlap, to be drawn again in front of him.
fn quickfloor(c: &Char, e: &DrawEdges, marks: &mut [u8; 30]) {
    let mark = if (135..149).contains(&c.posn) {
        2
    } else if c.action == 1 {
        if !(78..80).contains(&c.posn) {
            return;
        }
        1
    } else if matches!(c.action, 2 | 3 | 4 | 6) {
        1
    } else {
        return;
    };
    let mut x = e.rightblock;
    loop {
        for y in [e.bottomblock, e.topblock] {
            if let (i, true) = indexblock(x, y) {
                marks[i as usize] = marks[i as usize].max(mark);
            }
            if e.topblock == e.bottomblock {
                break;
            }
        }
        if x == e.leftblock {
            break;
        }
        x -= 1;
        if x < 0 {
            break;
        }
    }
}

/// CROPCHAR (CTRLSUBS.S): crop a character climbing the stairs to the
/// door, one under a solid floor at the top, and one standing left of a
/// panel or solid block at the right.
fn cropchar(g: &Game, c: &Char, f: &FChar, e: &DrawEdges, doortop: Option<i32>) -> (i32, i32, i32, i32) {
    let mut crop = ZEROCROP;
    if (224..229).contains(&c.posn) {
        if let Some(d) = doortop {
            if d + 2 < f.y as i32 {
                crop.1 = d + 2;
            }
        }
        return crop;
    }
    let l = &g.level;
    let s = c.scrn;
    let by = c.block_y as i8 as i32;
    let solid = |id: u8| id == obj::BLOCK || !crate::level::is_space(id);

    // under a solid floor: crop the top
    if solid(l.rdblock(s, e.leftblock, e.topblock).id) {
        let jumpup = c.action == 0 && matches!(c.posn, 79 | 81);
        if jumpup || solid(l.rdblock(s, e.rightblock, e.topblock).id) {
            let top = BLOCK_TOP[(by + 1).clamp(0, 4) as usize];
            if by == 0 || (top < f.y && top.wrapping_sub(FLOORHEIGHT) < e.topej) {
                crop.1 = top as i32;
            }
        }
    }

    // standing left of a panel: crop the right
    let bx = getblockx(e.cd_left).0 as i32;
    let foot = l.rdblock(s, bx, by);
    if matches!(foot.id, obj::PANELWOF | obj::PANELWIF) {
        let wall = if c.face & 0x80 == 0 && c.action == 2 {
            Some(foot) // hanging facing right: no need to check his head
        } else {
            let head = l.rdblock(s, bx, e.topblock);
            matches!(head.id, obj::BLOCK | obj::PANELWOF | obj::PANELWIF).then_some(head)
        };
        if let Some(w) = wall {
            crop.2 = (w.bx * 4 + 4) * 7;
            return crop;
        }
    }

    // left of a solid block
    let bx = getblockx(e.cd_right).0 as i32;
    if l.rdblock(s, bx, by).id == obj::BLOCK {
        let head = l.rdblock(s, bx, e.topblock);
        if head.id == obj::BLOCK && head.scrn == s {
            crop.2 = head.bx * 4 * 7;
        }
    }
    crop
}

/// GAMEEQ.S floorheight
const FLOORHEIGHT: u8 = 15;

/// drawfloor (FRAMEADV.S): a floorpiece drawn again in front of a
/// character, where there is empty space to its left.
fn drawfloor(p: &mut Painter, b: &Blk) {
    if b.preced != obj::SPACE {
        return;
    }
    drawflr(p, b);
}

fn drawflr(p: &mut Painter, b: &Blk) {
    p.add(MASKA[b.objid as usize % 30], b.xco, b.ay, Op::And, Ink::None);
    drawa(p, b);
    drawma(p, b);
    drawd(p, b);
}

/// drawhalf: drawfloor for a kid climbing up, only the front half of the
/// piece where there is one.
fn drawhalf(p: &mut Painter, v: &View, b: &Blk) {
    const CUMASK: u8 = 0x11;
    const CUPIECE: u8 = 0x12;
    const CUPOST: u8 = 0x0e;
    if b.preced != obj::SPACE {
        return;
    }
    let img = match b.objid {
        obj::FLOOR | obj::TORCH | obj::DPRESSPLATE | obj::EXIT => CUPIECE,
        obj::POSTS | obj::ARCHBOT if v.palace => CUPOST,
        _ => return drawflr(p, b),
    };
    let y = if b.objid == obj::DPRESSPLATE { b.ay + 1 } else { b.ay };
    p.add(CUMASK, b.xco, y, Op::And, Ink::None);
    p.add(img, b.xco, y, Op::Or, Ink::Floor);
    drawd(p, b);
}

/// The kid's reflection in a mirror, cropped to the mirror.
fn draw_reflection(scr: &mut Screen, g: &Game) {
    let Some((c, left)) = g.reflect else { return };
    let (f, _) = setupchar(&c);
    let by = c.block_y as i8 as i32;
    let top = BLOCK_TOP[(by + 1).clamp(0, 4) as usize];
    if top >= f.y {
        return;
    }
    lay_char_crop(scr, g, &f, Op::Mask, Ink::Kid, left, top as i32);
}

fn lay_char_crop(scr: &mut Screen, g: &Game, f: &FChar, op: Op, ink: Ink, left: i32, top: i32) {
    let Some(img) = apple::chtable(f.table, g.level.number).and_then(|t| t.get(f.image)) else { return };
    scr.lay_crop(&img, f.x, f.y as i32, f.face & 0x80 == 0, op, ink, left, top);
}

/// ADDFCHARX: FCharX + `a` in the direction the image faces.
fn addfcharx(f: &FChar, a: u8) -> i32 {
    let a = a as i8 as i32;
    if f.face & 0x80 != 0 {
        f.x - a
    } else {
        f.x + a
    }
}

/// SETUPSWORD: the sword in the character's hand, when it shows.
fn sword(c: &Char, f: &FChar, frame: &crate::seq::Frame) -> Option<FChar> {
    let shows = (c.id == 2 && c.life & 0x80 != 0) || (229..238).contains(&c.posn) || c.sword != 0;
    if !shows {
        return None;
    }
    let n = frame.sword & 0x3f;
    if n == 0 {
        return None;
    }
    let (image, dx, dy) = crate::seq::sword(n);
    if image == 0 {
        return None;
    }
    Some(FChar { x: addfcharx(f, dx), y: f.y.wrapping_add(dy), face: f.face, image, table: 2 })
}

/// SETUPCOMIX: the impact star on a character who has just been hurt.
fn comix(c: &Char, f: &FChar) -> Option<Obj> {
    const STARIMAGE: u8 = 0x41;
    let mut sf = *f;
    match c.posn {
        178 => return None,
        177 => {
            sf.x = addfcharx(&sf, (-5i8) as u8);
        }
        185 | 106..=110 => {
            sf.y = sf.y.wrapping_add(4);
            sf.x = addfcharx(&sf, 5);
        }
        _ => {
            let dy: i8 = if c.id == 0 { -15 } else { -11 };
            sf.y = sf.y.wrapping_add(dy as u8);
            sf.x = addfcharx(&sf, 5);
        }
    }
    // colour by pixel parity: kid red, opponents blue
    let who = if c.id == 0 { 0 } else { 1 };
    if (who ^ sf.x ^ (sf.face as i32)) & 1 == 0 {
        sf.x += 1;
    }
    sf.image = STARIMAGE;
    sf.table = 0;
    Some(Obj { f: sf, op: Op::Mask, ink: Ink::Flame, shift: 0, crop: ZEROCROP, index: 0xff, shad: false })
}

/// SETUPCHAR for drawing, on a copy so drawing never mutates Game.
pub(crate) fn setupchar(c: &Char) -> (FChar, crate::seq::Frame) {
    let frame = crate::seq::frame(c.id, c.posn);
    let (table, image) = crate::seq::decode_image(&frame);
    let left = c.face & 0x80 != 0;
    let x = if left { c.x.wrapping_sub(frame.dx) } else { c.x.wrapping_add(frame.dx) };
    let a = x.wrapping_sub(SCRN_LEFT);
    let mut px = a as i32 * 2;
    if px >= 256 && (px & 0xff) >= 0xf0 {
        px -= 512;
    }
    if (frame.check ^ c.face) & 0x80 == 0 {
        px += 1;
    }
    (FChar { x: px, y: frame.dy.wrapping_add(c.y), face: c.face, image, table }, frame)
}

#[cfg(test)]
mod crop_tests {
    use super::*;

    #[test]
    fn getblocky_follows_blocktop() {
        assert_eq!(getblocky(0), -1);
        assert_eq!(getblocky(3), 0);
        assert_eq!(getblocky(66), 1);
        assert_eq!(getblocky(191), 2);
        assert_eq!(getblocky(192), 3);
    }

    #[test]
    fn indexblock_numbers_blocks_and_columns_above() {
        assert_eq!(indexblock(3, 1), (13, true));
        assert_eq!(indexblock(4, -1), (4, false));
        assert_eq!(indexblock(10, 1), (30, false));
        assert_eq!(indexblock(0, 3), (30, false));
    }

    #[test]
    fn a_hanging_kid_marks_the_floors_he_overlaps() {
        let c = Char { posn: 140, action: 1, ..Char::default() };
        let e = DrawEdges { topej: 70, topblock: 1, bottomblock: 2, leftblock: 3, rightblock: 4, cd_left: 0, cd_right: 0 };
        let mut marks = [0u8; 30];
        quickfloor(&c, &e, &mut marks);
        let marked: Vec<usize> = (0..30).filter(|&i| marks[i] != 0).collect();
        assert_eq!(marked, vec![13, 14, 23, 24]);
        assert!(marks.iter().all(|&m| m == 0 || m == 2), "climbing up marks halves");
    }
}
