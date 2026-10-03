//! Game state: the "Char data" block and everything the kid routines share.
//!
//! Variable names follow the 6502 source (CharX, CharFace, clrF, ...) so the
//! port can be read side by side with CTRL.S, CTRLSUBS.S and COLL.S. Values
//! are bytes with wrapping arithmetic wherever the original relied on it:
//! CharX/CharY are screen coordinates that roll over at the edges and the
//! cut-to-next-screen logic depends on exactly that.

use crate::level::{Level, Rd};
use crate::seq::{self, seq as s, Frame};

pub const FACE_LEFT: u8 = 0xff;
pub const FACE_RIGHT: u8 = 0;
pub const ALIVE: u8 = 0xff;

/// One frame is 1/12 s, the speed the original runs at.
pub const FPS: u32 = 12;
pub const MINUTES: u32 = 60;

/// TABLES.S, indexed by block y (-1..3) + 1.
pub const BLOCK_BOT: [u8; 5] = [2, 65, 128, 191, 254];
pub const FLOOR_Y: [u8; 5] = [248, 55, 118, 181, 244];
pub const BLOCK_AY: [u8; 5] = [255, 62, 125, 188, 251];

pub const SCRN_LEFT: u8 = 58;
pub const ANGLE: u8 = 7;

/// Character variables (EQ.S "Char data").
#[derive(Clone, Copy, Debug, Default)]
pub struct Char {
    pub posn: u8,
    pub x: u8,
    pub y: u8,
    pub face: u8,
    pub block_x: u8,
    pub block_y: u8,
    pub action: u8,
    pub xvel: u8,
    pub yvel: u8,
    pub seq: u16,
    pub scrn: u8,
    pub repeat: u8,
    pub id: u8,
    pub sword: u8,
    pub life: u8,
}

/// Player input as GENCTRL sees it, already turned to face the character:
/// `jstk_x` < 0 is forward. `clr` flags: 0 no press, -1 unused fresh press,
/// 1 press already acted on (SPECIALK.S CLRJSTK).
#[derive(Clone, Copy, Debug, Default)]
pub struct Joy {
    pub jstk_x: i8,
    pub jstk_y: i8,
    pub btn: bool,
    pub clr_f: i8,
    pub clr_b: i8,
    pub clr_u: i8,
    pub clr_d: i8,
    pub clr_btn: i8,
}

/// Raw controls this frame in screen terms (left = -1), before facing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    pub x: i8,
    pub y: i8,
    pub btn: bool,
}

/// FChar data: where and what to draw.
#[derive(Clone, Copy, Debug, Default)]
pub struct FChar {
    /// 280-res x of the image (see SETUPCHAR)
    pub x: i32,
    pub y: u8,
    pub face: u8,
    pub image: u8,
    pub table: u8,
}

/// GETEDGES results (140-res).
#[derive(Clone, Copy, Debug, Default)]
pub struct Edges {
    pub left: u8,
    pub right: u8,
    pub cd_left: u8,
    pub cd_right: u8,
    pub imheight: u8,
}

/// Transitional object (MOVER.S TROB).
#[derive(Clone, Copy, Debug)]
pub struct Trob {
    pub loc: u8,
    pub scrn: u8,
    pub direc: i8,
}

/// Mobile object: a falling floor piece (MOVER.S MOB).
#[derive(Clone, Copy, Debug)]
pub struct Mob {
    /// block x * 4 (byte column, as in the original)
    pub x: u8,
    pub y: u8,
    pub scrn: u8,
    pub vel: u8,
    pub level: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sound {
    Footstep,
    SmackWall,
    Splat,
    Impaled,
    GateDown,
    GateSlam,
    RaisingGate,
    PlateDown,
    PlateUp,
    LooseCrash,
    Drink,
    Upstairs,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Playing,
    /// Kid is dead; the countdown is frames until the message shows.
    Dead(u8),
    /// Climbed the stairs; next level loads when this reaches 0.
    LevelDone(u8),
    TimeUp,
    Won,
}

pub struct Game {
    pub level: Level,
    pub kid: Char,
    pub joy: Joy,
    pub frame: Frame,
    pub fchar: FChar,
    pub edges: Edges,
    /// last RDBLOCK result (tempscrn/tempblockx/y and the blueprint index)
    pub rd: Rd,
    // CTRL.S scratch
    pub stunned: u8,
    pub rjumpflag: u8,
    pub weightless: u8,
    pub jarabove: i8,
    pub alertguard: bool,
    pub gotsword: bool,
    pub infrontx: i32,
    pub behindx: i32,
    pub abovey: i32,
    pub blockid: u8,
    pub savekidx: u8,
    // COLL.S buffers
    pub sn_this: [u8; 10],
    pub sn_last: [u8; 10],
    pub sn_above: [u8; 10],
    pub sn_below: [u8; 10],
    pub cd_this: [u8; 10],
    pub cd_last: [u8; 10],
    pub cd_above: [u8; 10],
    pub cd_below: [u8; 10],
    pub block_y_last: u8,
    pub blockedge: u8,
    // MOVER.S
    pub trobs: Vec<Trob>,
    pub mobs: Vec<Mob>,
    pub exitopen: bool,
    // strength and time
    pub kid_strength: u8,
    pub max_strength: u8,
    pub frames_left: u32,
    pub vis_scrn: u8,
    pub phase: Phase,
    pub next_level: bool,
    pub sounds: Vec<Sound>,
    pub message: Option<(String, u32)>,
    pub rng: u32,
    pub ticks: u64,
    /// last potion drunk (REMOVEOBJ lastpotion; 0xff = the sword)
    pub potion: u8,
    pub dead_frames: u32,
    /// upside-down potion
    pub inverted: bool,
}

impl Game {
    pub fn new(level: u8) -> Game {
        let mut g = Game {
            level: Level::load(level),
            kid: Char::default(),
            joy: Joy::default(),
            frame: Frame::default(),
            fchar: FChar::default(),
            edges: Edges::default(),
            rd: Rd { id: 0, scrn: 0, bx: 0, by: 0, idx: None },
            stunned: 0,
            rjumpflag: 0,
            weightless: 0,
            jarabove: 0,
            alertguard: false,
            gotsword: false,
            infrontx: 0,
            behindx: 0,
            abovey: 0,
            blockid: 0,
            savekidx: 0,
            sn_this: [0xff; 10],
            sn_last: [0xff; 10],
            sn_above: [0xff; 10],
            sn_below: [0xff; 10],
            cd_this: [0; 10],
            cd_last: [0; 10],
            cd_above: [0; 10],
            cd_below: [0; 10],
            block_y_last: 0,
            blockedge: 0,
            trobs: Vec::new(),
            mobs: Vec::new(),
            exitopen: false,
            kid_strength: 3,
            max_strength: 3,
            frames_left: MINUTES * 60 * FPS,
            vis_scrn: 1,
            phase: Phase::Playing,
            next_level: false,
            sounds: Vec::new(),
            message: None,
            rng: 0x1234_5678,
            ticks: 0,
            potion: 0,
            dead_frames: 0,
            inverted: false,
        };
        g.start_level(level, false);
        g
    }

    pub fn rnd(&mut self) -> u8 {
        // xorshift; the original's RND is a simple LFSR too
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as u8
    }

    pub fn add_sound(&mut self, s: Sound) {
        log::debug!("sound {:?}", s);
        self.sounds.push(s);
    }

    /// Load level `n` and put the kid at its start (SUBS.S STARTKID).
    pub fn start_level(&mut self, n: u8, keep_time: bool) {
        let n = n.min(crate::level::NUM_LEVELS - 1);
        log::info!("start level {n}");
        self.level = Level::load(n);
        crate::mover::set_initials(&mut self.level);
        self.trobs.clear();
        self.mobs.clear();
        self.exitopen = false;
        self.phase = Phase::Playing;
        self.next_level = false;
        self.stunned = 0;
        self.dead_frames = 0;
        self.inverted = false;
        if !keep_time {
            self.frames_left = MINUTES * 60 * FPS;
        }
        self.kid_strength = self.max_strength;
        let (scrn, block, face) = self.level.kid_start();
        let mut k = Char { scrn, ..Char::default() };
        k.block_x = block % 10;
        k.block_y = block / 10;
        k.x = getblockej(k.block_x as i8).wrapping_add(ANGLE + 7);
        k.face = face ^ 0xff;
        k.life = ALIVE;
        self.kid = k;
        self.vis_scrn = scrn;
        self.sn_last = [0xff; 10];
        self.sn_this = [0xff; 10];
        self.block_y_last = k.block_y;

        match n {
            1 => {
                // Special start for level 1: slam the gate on screen 5, fall in
                self.rd = self.level.rdblock(5, 2, 0);
                self.pushpp();
                self.jumpseq(s::STEPFALL);
            }
            13 => self.jumpseq(s::RUNNING),
            _ => self.jumpseq(s::TURN),
        }
        // STARTKID1
        let by = self.kid.block_y as i8;
        self.kid.y = FLOOR_Y[(by + 1) as usize];
        self.kid.xvel = 0;
        self.kid.yvel = 0;
        self.kid.sword = 0;
        self.weightless = 0;
        self.jarabove = 0;
        self.animchar();
        self.trigger_screen_objects();
        self.show_message(format!("LEVEL {n}"), 3 * FPS);
    }

    pub fn show_message(&mut self, s: String, frames: u32) {
        self.message = Some((s, frames));
    }

    pub fn minutes_left(&self) -> u32 {
        self.frames_left.div_ceil(60 * FPS)
    }

    /// One frame (TOPCTRL.S NextFrame), `c` is this frame's raw input.
    pub fn tick(&mut self, c: Controls) {
        self.ticks += 1;
        self.sounds.clear();
        if let Some((_, t)) = &mut self.message {
            *t = t.saturating_sub(1);
            if *t == 0 {
                self.message = None;
            }
        }
        match self.phase {
            Phase::LevelDone(n) => {
                if n == 0 {
                    let next = self.level.number + 1;
                    if next >= crate::level::NUM_LEVELS {
                        self.phase = Phase::Won;
                    } else {
                        self.start_level(next, true);
                    }
                } else {
                    self.phase = Phase::LevelDone(n - 1);
                }
                return;
            }
            Phase::TimeUp | Phase::Won => return,
            _ => {}
        }
        self.animmobs();
        self.animtrans();
        self.dokid(c);
        self.cutcheck();

        if self.next_level {
            self.next_level = false;
            self.phase = Phase::LevelDone(FPS as u8);
            log::info!("level {} done", self.level.number);
            return;
        }
        // Level 6: falling off screen 1 takes you to the next level
        if self.level.number == 6 && self.kid.scrn == 1 && self.kid.y < 20 {
            self.phase = Phase::LevelDone(1);
            return;
        }
        if self.level.number < 13 || (self.level.number == 13 && !self.exitopen) {
            self.frames_left = self.frames_left.saturating_sub(1);
            if self.frames_left == 0 && self.level.number < 13 {
                self.phase = Phase::TimeUp;
                self.show_message("TIME HAS EXPIRED".into(), u32::MAX);
                log::info!("time up");
            } else if self.frames_left % (60 * FPS) == 0 {
                let m = self.minutes_left();
                self.show_message(format!("{m} MINUTE{} LEFT", if m == 1 { "" } else { "S" }), 2 * FPS);
            }
        }
        if self.kid.life & 0x80 == 0 {
            self.dead_frames += 1;
            if self.phase == Phase::Playing {
                self.phase = Phase::Dead(0);
                log::info!("kid died at posn {}", self.kid.posn);
            }
        }
    }

    /// Continue after death: restart the level, clock keeps running.
    pub fn restart_level(&mut self) {
        let n = self.level.number;
        let t = self.frames_left;
        self.start_level(n, true);
        self.frames_left = t;
    }

    /// DoKid (TOPCTRL.S)
    fn dokid(&mut self, c: Controls) {
        self.rereadblocks();
        if self.kid.scrn == 0 {
            // kill0: fell off the map
            self.kid.life = 0;
        }
        self.playerctrl(c);
        if self.weightless != 0 {
            self.weightless -= 1;
        }
        if self.kid.scrn == 0 {
            return;
        }
        self.animchar();
        self.gravity();
        self.addfall();
        self.setupchar();
        self.rereadblocks();
        self.getedges();
        let (cl, cr) = self.checkbarr();
        self.collisions(cl, cr);
        self.checkfloor();
        self.checkpress();
        self.checkspikes();
        self.checkimpale();
        self.shakeloose();
    }

    /// CUTCHECK / cutchar / CUT (AUTO.S): follow the kid off the edge.
    fn cutcheck(&mut self) {
        self.setupchar();
        self.getedges();
        let Some(dir) = self.cutchar() else { return };
        let l = &self.level;
        let k = &mut self.kid;
        match dir {
            0 => {
                k.scrn = l.left(k.scrn);
                k.x = k.x.wrapping_add(140);
            }
            1 => {
                k.scrn = l.right(k.scrn);
                k.x = k.x.wrapping_sub(140);
            }
            2 => {
                k.scrn = l.up(k.scrn);
                k.block_y = k.block_y.wrapping_add(3);
                k.y = k.y.wrapping_add(189);
            }
            _ => {
                k.scrn = l.down(k.scrn);
                k.block_y = k.block_y.wrapping_sub(3);
                k.y = k.y.wrapping_sub(189);
            }
        }
        log::debug!("cut {dir} to screen {}", self.kid.scrn);
        if self.kid.scrn != 0 {
            self.vis_scrn = self.kid.scrn;
            self.trigger_screen_objects();
        }
    }

    fn cutchar(&mut self) -> Option<u8> {
        const TOP_PL: u8 = 10;
        const TOP_MI: u8 = (-16i8) as u8;
        const BOT: u8 = 191 + 24;
        let y = self.kid.y;
        let falling = matches!(self.kid.action, 5 | 4 | 3);
        if !falling && (y < TOP_PL || y >= TOP_MI) {
            return Some(2);
        }
        if y >= BOT {
            if self.level.number == 6 && self.kid.scrn == 1 {
                return None;
            }
            return Some(3);
        }
        let p = self.kid.posn;
        if (135..150).contains(&p) || (110..120).contains(&p) || (150..163).contains(&p) || (166..169).contains(&p) {
            return None;
        }
        if self.kid.action == 7 {
            return None;
        }
        const LEFT_CUT: u8 = SCRN_LEFT - 4;
        const RIGHT_CUT: u8 = SCRN_LEFT + 140 - 1 + 4;
        if self.kid.face & 0x80 != 0 {
            let l = self.edges.left;
            if l <= LEFT_CUT {
                return Some(0);
            }
            if l >= SCRN_LEFT + 140 {
                return Some(1);
            }
            return None;
        }
        let (s, by) = (self.kid.scrn, self.kid.block_y as i8 as i32);
        let id = self.rdblock(s, 9, by);
        let panel = matches!(id, crate::level::obj::PANELWIF | crate::level::obj::PANELWOF);
        if !panel && self.edges.right >= RIGHT_CUT {
            return Some(1);
        }
        if self.edges.right <= SCRN_LEFT - 1 {
            return Some(0);
        }
        None
    }

    /// JUMPSEQ
    pub fn jumpseq(&mut self, n: u8) {
        self.kid.seq = seq::start(n);
    }
}

/// GETBLOCKEJ: screen x of a block's left edge.
pub fn getblockej(bx: i8) -> u8 {
    (-12i32 + 14 * (bx as i32 + 5)) as u8
}

/// GETBLOCKX: block (-5..14) and pixel within it (0..13) for an x-coord.
pub fn getblockx(a: u8) -> (i8, u8) {
    if a < 2 {
        (-5, 12 + a)
    } else {
        let v = a as i32 - 2;
        ((v / 14 - 4) as i8, (v % 14) as u8)
    }
}

/// GETBLOCKXP: same, for objects on the center plane.
pub fn getblockxp(a: u8) -> (i8, u8) {
    getblockx(a.wrapping_sub(ANGLE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_tables_match_tables_s() {
        assert_eq!(getblockej(0), 58);
        assert_eq!(getblockej(9), 58 + 126);
        assert_eq!(getblockx(58), (0, 0));
        assert_eq!(getblockx(0), (-5, 12));
        assert_eq!(getblockx(255), (14, 1));
        assert_eq!(getblockxp(65), (0, 0));
    }
}
