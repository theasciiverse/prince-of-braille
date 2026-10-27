//! Level blueprints: the raw 2304-byte `LEVEL*` files (EQ.S `blueprnt`).
//!
//! | bytes | name     | meaning                                       |
//! |-------|----------|-----------------------------------------------|
//! | 720   | BLUETYPE | 24 screens x 30 blocks: object id (low 5 bits)|
//! | 720   | BLUESPEC | 24 x 30: object state (gate height, ...)      |
//! | 256   | LINKLOC  | pressure plate link targets                   |
//! | 256   | LINKMAP  | pressure plate link timers/flags              |
//! | 96    | MAP      | 24 x 4: screen to the left, right, up, down   |
//! | 256   | INFO     | start positions (kid, sword, guards)          |

pub const NUM_LEVELS: u8 = 15;
const IDMASK: u8 = 0b0001_1111;

/// Block object ids (MOVEDATA.S / BGDATA.S).
#[allow(dead_code)]
pub mod obj {
    pub const SPACE: u8 = 0;
    pub const FLOOR: u8 = 1;
    pub const SPIKES: u8 = 2;
    pub const POSTS: u8 = 3;
    pub const GATE: u8 = 4;
    pub const DPRESSPLATE: u8 = 5;
    pub const PRESSPLATE: u8 = 6;
    pub const PANELWIF: u8 = 7;
    pub const PILLARBOTTOM: u8 = 8;
    pub const PILLARTOP: u8 = 9;
    pub const FLASK: u8 = 10;
    pub const LOOSE: u8 = 11;
    pub const PANELWOF: u8 = 12;
    pub const MIRROR: u8 = 13;
    pub const RUBBLE: u8 = 14;
    pub const UPRESSPLATE: u8 = 15;
    pub const EXIT: u8 = 16;
    pub const EXIT2: u8 = 17;
    pub const SLICER: u8 = 18;
    pub const TORCH: u8 = 19;
    pub const BLOCK: u8 = 20;
    pub const BONES: u8 = 21;
    pub const SWORD: u8 = 22;
    pub const WINDOW: u8 = 23;
    pub const WINDOW2: u8 = 24;
    pub const ARCHBOT: u8 = 25;
    pub const ARCHTOP1: u8 = 26;
    pub const ARCHTOP2: u8 = 27;
    pub const ARCHTOP3: u8 = 28;
    pub const ARCHTOP4: u8 = 29;
}

static LEVELS: [&[u8]; NUM_LEVELS as usize] = [
    include_bytes!("../assets/apple2/LEVEL0"),
    include_bytes!("../assets/apple2/LEVEL1"),
    include_bytes!("../assets/apple2/LEVEL2"),
    include_bytes!("../assets/apple2/LEVEL3"),
    include_bytes!("../assets/apple2/LEVEL4"),
    include_bytes!("../assets/apple2/LEVEL5"),
    include_bytes!("../assets/apple2/LEVEL6"),
    include_bytes!("../assets/apple2/LEVEL7"),
    include_bytes!("../assets/apple2/LEVEL8"),
    include_bytes!("../assets/apple2/LEVEL9"),
    include_bytes!("../assets/apple2/LEVEL10"),
    include_bytes!("../assets/apple2/LEVEL11"),
    include_bytes!("../assets/apple2/LEVEL12"),
    include_bytes!("../assets/apple2/LEVEL13"),
    include_bytes!("../assets/apple2/LEVEL14"),
];

/// Background set per level (MISC.S bgset1): 0/2 dungeon, 1 palace.
const BGSET: [u8; 15] = [0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 1, 1, 2, 2, 1];

#[derive(Clone)]
pub struct Level {
    pub number: u8,
    pub bluetype: [u8; 720],
    pub bluespec: [u8; 720],
    pub linkloc: [u8; 256],
    pub linkmap: [u8; 256],
    map: [u8; 96],
    pub info: [u8; 256],
}

/// Offsets of the per-screen guard tables in INFO (EQ.S), each 24 bytes
/// indexed by screen - 1.
pub mod gd {
    pub const BLOCK: usize = 71;
    pub const FACE: usize = 95;
    pub const X: usize = 119;
    pub const SEQL: usize = 143;
    pub const PROG: usize = 167;
    pub const SEQH: usize = 191;
}

/// What RDBLOCK hands back: the object plus where it really lives once
/// offscreen references are traced to their home screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rd {
    pub id: u8,
    pub scrn: u8,
    pub bx: i32,
    pub by: i32,
    /// index into BLUETYPE/BLUESPEC, None on the null screen (solid rock)
    pub idx: Option<usize>,
}

impl Level {
    pub fn load(number: u8) -> Level {
        let d = LEVELS[number as usize];
        let mut l = Level {
            number,
            bluetype: [0; 720],
            bluespec: [0; 720],
            linkloc: [0; 256],
            linkmap: [0; 256],
            map: [0; 96],
            info: [0; 256],
        };
        l.bluetype.copy_from_slice(&d[0..720]);
        l.bluespec.copy_from_slice(&d[720..1440]);
        l.linkloc.copy_from_slice(&d[1440..1696]);
        l.linkmap.copy_from_slice(&d[1696..1952]);
        l.map.copy_from_slice(&d[1952..2048]);
        l.info.copy_from_slice(&d[2048..2304]);
        l
    }

    pub fn palace(&self) -> bool {
        BGSET[self.number as usize] == 1
    }

    pub fn kid_start(&self) -> (u8, u8, u8) {
        (self.info[64], self.info[65], self.info[66])
    }

    /// GdStart table `t` (a `gd::` offset) for screen `scrn` (1-24).
    pub fn gd(&self, t: usize, scrn: u8) -> u8 {
        if scrn == 0 || scrn > 24 {
            return 0xff;
        }
        self.info[t + scrn as usize - 1]
    }

    pub fn set_gd(&mut self, t: usize, scrn: u8, v: u8) {
        if (1..=24).contains(&scrn) {
            self.info[t + scrn as usize - 1] = v;
        }
    }

    fn link(&self, scrn: u8, dir: usize) -> u8 {
        if scrn == 0 || scrn > 24 {
            return 0;
        }
        self.map[(scrn as usize - 1) * 4 + dir]
    }
    pub fn left(&self, s: u8) -> u8 {
        self.link(s, 0)
    }
    pub fn right(&self, s: u8) -> u8 {
        self.link(s, 1)
    }
    pub fn up(&self, s: u8) -> u8 {
        self.link(s, 2)
    }
    pub fn down(&self, s: u8) -> u8 {
        self.link(s, 3)
    }

    pub fn index(scrn: u8, bx: i32, by: i32) -> usize {
        (scrn as usize - 1) * 30 + by as usize * 10 + bx as usize
    }

    /// RDBLOCK (CTRLSUBS.S): read block (bx, by) relative to screen `scrn`,
    /// following the map off any edge. Screen 0 is solid rock.
    pub fn rdblock(&self, scrn: u8, bx: i32, by: i32) -> Rd {
        let (mut s, mut x, mut y) = (scrn, bx, by);
        loop {
            if x < 0 {
                x += 10;
                s = self.left(s);
            } else if x >= 10 {
                x -= 10;
                s = self.right(s);
            } else if y < 0 {
                y += 3;
                s = self.up(s);
            } else if y >= 3 {
                y -= 3;
                s = self.down(s);
            } else {
                break;
            }
        }
        if s == 0 {
            return Rd { id: obj::BLOCK, scrn: 0, bx: x, by: y, idx: None };
        }
        let i = Self::index(s, x, y);
        Rd { id: self.bluetype[i] & IDMASK, scrn: s, bx: x, by: y, idx: Some(i) }
    }

    pub fn spec(&self, rd: &Rd) -> u8 {
        rd.idx.map_or(0, |i| self.bluespec[i])
    }

    pub fn set_type(&mut self, idx: usize, id: u8) {
        self.bluetype[idx] = (self.bluetype[idx] & !IDMASK) | id;
    }
}

/// CMPSPACE: can you fall through it? (A solid block has no floor to stand on.)
pub fn is_space(id: u8) -> bool {
    matches!(id, obj::SPACE | obj::PILLARTOP | obj::PANELWOF | obj::BLOCK) || id >= obj::ARCHTOP1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_levels_load() {
        for n in 0..NUM_LEVELS {
            let l = Level::load(n);
            let (s, b, _) = l.kid_start();
            assert!((1..=24).contains(&s), "level {n} start screen {s}");
            assert!(b < 30, "level {n} start block {b}");
        }
    }

    #[test]
    fn rdblock_follows_the_map() {
        let l = Level::load(1);
        // level 1, screen 1: left neighbour is screen 5
        let rd = l.rdblock(1, -1, 0);
        assert_eq!((rd.scrn, rd.bx, rd.by), (5, 9, 0));
        // nothing to the right of screen 1: solid rock
        assert_eq!(l.rdblock(1, 10, 0).id, obj::BLOCK);
    }
}
