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
/// SPECIALK.S: the clock counts frames, `min` of them to a "minute" and
/// `sec` = min/60 to a second, with a limit of `t` minutes.
pub const MIN_FRAMES: u32 = 725;
pub const SEC_FRAMES: u32 = MIN_FRAMES / 60;
pub const MINUTES: u32 = 60;
/// SPECIALK.S timetable: FrameCount at which each time message is due,
/// the last one five frames after t = 0 (game over).
const TIMETABLE: [u32; 17] = [
    0,
    5 * MIN_FRAMES,
    10 * MIN_FRAMES,
    15 * MIN_FRAMES,
    20 * MIN_FRAMES,
    25 * MIN_FRAMES,
    30 * MIN_FRAMES,
    35 * MIN_FRAMES,
    40 * MIN_FRAMES,
    45 * MIN_FRAMES,
    50 * MIN_FRAMES,
    55 * MIN_FRAMES,
    56 * MIN_FRAMES,
    57 * MIN_FRAMES,
    58 * MIN_FRAMES,
    59 * MIN_FRAMES + 1,
    60 * MIN_FRAMES + 5,
];
/// SUBS.S timemsgtimer
const TIMEMSGTIMER: u32 = 20;

/// TABLES.S, indexed by block y (-1..3) + 1.
pub const BLOCK_BOT: [u8; 5] = [2, 65, 128, 191, 254];
pub const FLOOR_Y: [u8; 5] = [248, 55, 118, 181, 244];
pub const BLOCK_AY: [u8; 5] = [255, 62, 125, 188, 251];
pub const BLOCK_TOP: [u8; 5] = [196, 3, 66, 129, 192];

pub const SCRN_LEFT: u8 = 58;
pub const SCRN_RIGHT: u8 = SCRN_LEFT + 140 - 1;
pub const BOT_CUT_EDGE: u8 = 191 + 24;
const MOUSETIMER: u8 = 150;
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
    SwordClash,
    JawsClash,
    RaisingExit,
    MirrorCrack,
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
    /// a scene in the princess's room (`cut`)
    Cut,
}

pub struct Game {
    pub level: Level,
    /// Char: the character being moved right now (a copy of `kid` or
    /// `shad`, loaded and saved the way CTRLSUBS.S LOADKID/SAVESHAD do)
    pub ch: Char,
    /// Op: the opponent of whoever is in `ch`
    pub op: Char,
    pub kid: Char,
    /// the shadowman or other opponent; face = NO_CHAR when there is none
    pub shad: Char,
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
    // fighting (AUTO.S, GAMEEQ.S)
    pub enemy_alert: u8,
    pub guardprog: u8,
    pub guard_color: u8,
    pub justblocked: u8,
    pub gdtimer: u8,
    pub refract: u8,
    pub offguard: bool,
    pub droppedout: bool,
    pub heroic: bool,
    pub createshad: bool,
    pub shadowaction: u8,
    /// level 12: frames of kid and shadow flashing after they merge; -1 after
    pub mergetimer: i8,
    pub playcount: u8,
    pub prerecptr: usize,
    /// white or coloured screen flashes left (lightning, lightcolor)
    pub lightning: u8,
    pub lightcolor: u8,
    /// level 3: past the first gate, so a restart starts there
    pub milestone: bool,
    /// the kid's reflection in a mirror this frame, and the left edge
    /// (screen pixels) it is cropped to
    pub reflect: Option<(Char, i32)>,
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
    /// set when the exit opens; some levels use it as a counter or flag
    pub exitopen: u8,
    // strength and time
    pub kid_strength: u8,
    pub max_strength: u8,
    pub opp_strength: u8,
    pub max_opp_str: u8,
    /// this frame's strength changes, applied by CHGMETERS (as bytes)
    pub chg_kid_str: u8,
    pub chg_opp_str: u8,
    /// FrameCount: frames played so far (the clock counts up)
    pub frame_count: u32,
    /// NextTimeMsg, as an index into TIMETABLE
    pub next_time_msg: usize,
    /// timerequest (SUBS.S SHOWTIME): 0 no, 1-2 auto, 4 vizier dead
    pub time_request: u8,
    pub vis_scrn: u8,
    /// CUTTIMER: frames to wait before the next screen cut
    pub cuttimer: u8,
    pub phase: Phase,
    pub next_level: bool,
    pub sounds: Vec<Sound>,
    pub message: Option<(String, u32)>,
    /// the princess's room scene being played
    pub cut: Option<crate::cut::Cut>,
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
            ch: Char::default(),
            op: Char::default(),
            kid: Char::default(),
            shad: Char { face: crate::guard::NO_CHAR, id: 2, ..Char::default() },
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
            enemy_alert: 0,
            guardprog: 0,
            guard_color: 0,
            justblocked: 0,
            gdtimer: 0,
            refract: 0,
            offguard: false,
            droppedout: false,
            heroic: false,
            createshad: false,
            shadowaction: 0,
            mergetimer: 0,
            playcount: 0,
            prerecptr: 0,
            lightning: 0,
            lightcolor: 0,
            milestone: false,
            reflect: None,
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
            exitopen: 0,
            kid_strength: 3,
            max_strength: 3,
            opp_strength: 0,
            max_opp_str: 0,
            chg_kid_str: 0,
            chg_opp_str: 0,
            frame_count: 0,
            next_time_msg: 0,
            time_request: 0,
            vis_scrn: 1,
            cuttimer: 0,
            phase: Phase::Playing,
            next_level: false,
            sounds: Vec::new(),
            message: None,
            cut: None,
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

    /// Load level `n` and put the kid at its start (TOPCTRL.S RESTART,
    /// SUBS.S STARTKID).
    pub fn start_level(&mut self, n: u8, keep_time: bool) {
        let n = n.min(crate::level::NUM_LEVELS - 1);
        log::info!("start level {n}");
        if n != self.level.number {
            self.milestone = false;
        }
        self.level = Level::load(n);
        crate::mover::set_initials(&mut self.level);
        self.initialguards();
        self.trobs.clear();
        self.mobs.clear();
        self.exitopen = 0;
        self.phase = Phase::Playing;
        self.next_level = false;
        self.stunned = 0;
        self.dead_frames = 0;
        self.inverted = false;
        self.lightning = 0;
        self.mergetimer = 0;
        self.enemy_alert = 0;
        self.createshad = false;
        self.heroic = false;
        self.chg_kid_str = 0;
        self.chg_opp_str = 0;
        self.opp_strength = 0;
        self.playcount = 0;
        self.prerecptr = 0;
        // Level 1 starts without the sword
        self.gotsword = n != 1;
        self.shad = Char { id: 2, face: crate::guard::NO_CHAR, ..Char::default() };
        if !keep_time {
            self.frame_count = 0;
            self.next_time_msg = 0;
        }
        self.startkid(n);
        self.vis_scrn = 0;
        self.prepcut(self.kid.scrn);
        self.show_message(format!("LEVEL {n}"), 3 * FPS);
    }

    /// STARTKID
    fn startkid(&mut self, n: u8) {
        let (mut scrn, mut block, mut face) = self.level.kid_start();
        if n == 3 && self.milestone {
            // past the first gate: start just inside it, loose floor gone
            face = 0xff;
            scrn = 2;
            block = 6;
            let i = Level::index(7, 4, 0);
            self.level.set_type(i, crate::level::obj::SPACE);
        }
        let mut k = Char { scrn, ..Char::default() };
        k.block_x = block % 10;
        k.block_y = block / 10;
        k.x = getblockej(k.block_x as i8).wrapping_add(ANGLE + 7);
        k.face = face ^ 0xff;
        self.ch = k;
        self.kid_strength = self.max_strength;

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
        let by = self.ch.block_y as i8;
        self.ch.y = FLOOR_Y[(by + 1) as usize];
        self.ch.life = ALIVE;
        self.ch.id = 0;
        self.ch.xvel = 0;
        self.ch.yvel = 0;
        self.weightless = 0;
        self.jarabove = 0;
        self.droppedout = false;
        self.ch.sword = 0;
        self.offguard = false;
        self.animchar();
        // Level 7 (weightless): the kid falls into screen 17 from above
        if n == 7 && self.ch.scrn == 17 {
            self.cut(3);
        }
        self.sn_last = [0xff; 10];
        self.sn_this = [0xff; 10];
        self.block_y_last = self.ch.block_y;
        self.save_kid();
    }

    /// flashon (TOPCTRL.S): this frame's flash colour, as a lo-res colour
    /// number (lightning, or red when the kid has just been hurt).
    pub fn flash(&self) -> Option<u8> {
        if self.lightning > 0 {
            Some(self.lightcolor & 0x0f)
        } else if self.chg_kid_str & 0x80 != 0 {
            Some(1) // $11: red
        } else {
            None
        }
    }

    pub fn show_message(&mut self, s: String, frames: u32) {
        self.message = Some((s, frames));
    }

    /// GETMINLEFT (SPECIALK.S): minutes left, and seconds left during the
    /// final minute (`None` before it).
    pub fn time_left(&self) -> (u32, Option<u32>) {
        let f = self.frame_count;
        let min = MINUTES.saturating_sub(f / MIN_FRAMES);
        if min >= 2 {
            return (min, None);
        }
        let past = f.saturating_sub((MINUTES - 1) * MIN_FRAMES);
        (min, Some(60u32.saturating_sub(past / SEC_FRAMES)))
    }

    fn time_message(&self) -> String {
        match self.time_left() {
            (m, None) => format!("{m} MINUTES LEFT"),
            (_, Some(1)) => "1 SECOND LEFT".into(),
            (_, Some(s)) => format!("{s} SECONDS LEFT"),
        }
    }

    /// KEEPTIME (SPECIALK.S): the clock runs only while the kid is alive,
    /// and puts up the next time message when it is due and the screen is
    /// free of other messages.
    fn keeptime(&mut self) {
        if self.kid.life & 0x80 == 0 {
            return;
        }
        self.frame_count += 1;
        if let Some(&due) = TIMETABLE.get(self.next_time_msg) {
            if self.frame_count >= due && self.message.is_none() {
                self.next_time_msg += 1;
                self.time_request = 2;
            }
        }
    }

    /// SHORTENTIME (SPECIALK.S): skipping a level cuts the time left to
    /// 15 minutes, unless it is already that short.
    pub fn shortentime(&mut self) {
        if self.next_time_msg < 10 {
            self.next_time_msg = 9;
            self.frame_count = TIMETABLE[9];
        }
    }

    /// SHOWTIME (SUBS.S): show the time if requested, and a constant
    /// countdown through the final minute.
    fn showtime(&mut self) {
        if self.time_request == 0 || self.kid.life & 0x80 == 0 {
            return;
        }
        match self.time_left() {
            (_, Some(0)) => {}
            (_, Some(s)) if self.level.number < 14 => {
                // countdown: stays up while two or more seconds are left
                if s < 2 {
                    self.time_request = 0;
                    if self.message.as_ref().is_some_and(|(m, _)| m.ends_with(" LEFT")) {
                        self.message = None;
                    }
                } else if self.message.is_none() || self.message.as_ref().is_some_and(|(m, _)| m.ends_with(" LEFT")) {
                    self.message = Some((self.time_message(), 2));
                    self.time_request = 1;
                }
            }
            _ => {
                if self.message.is_some() {
                    return;
                }
                let t = if self.time_request >= 4 { TIMEMSGTIMER + 5 } else { TIMEMSGTIMER };
                self.show_message(self.time_message(), t);
                self.time_request = 0;
            }
        }
    }

    /// One frame (TOPCTRL.S MainLoop and NextFrame), `c` is this frame's
    /// raw input.
    pub fn tick(&mut self, c: Controls) {
        self.ticks += 1;
        self.sounds.clear();
        self.chg_kid_str = 0;
        self.chg_opp_str = 0;
        if self.lightning > 0 {
            self.lightning -= 1;
        }
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
                    } else if let Some(n) = crate::cut::cut_before(next) {
                        // a princess cut before certain levels
                        self.playcut(n, next, crate::cut::After::Level(next));
                    } else {
                        self.start_level(next, true);
                        // LoadNextLevel: show the time left
                        if next != 14 {
                            self.time_request = 1;
                        }
                    }
                } else {
                    self.phase = Phase::LevelDone(n - 1);
                }
                return;
            }
            Phase::TimeUp | Phase::Won => return,
            Phase::Cut => return self.cut_tick(c),
            _ => {}
        }
        self.misctimers();
        self.animmobs();
        self.animtrans();
        self.bonesrise();
        self.checkalert();
        self.dokid(c);
        self.doshad();
        self.checkstrike();
        self.checkstab();
        self.addsfx();
        self.chgmeters();
        if let Some(scrn) = self.cutcheck() {
            self.prepcut(scrn);
        }
        self.cutguard();
        self.reflection();

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
        // Level 12: leaving screen 23 goes straight on to level 13
        if self.level.number == 12 && self.kid.scrn == 23 {
            self.phase = Phase::LevelDone(0);
            return;
        }
        // the clock stops on level 14, and on level 13 once the vizier is dead
        if self.level.number < 13 || (self.level.number == 13 && self.exitopen == 0) {
            self.keeptime();
        }
        self.showtime();
        // You have one chance to finish level 13 after time runs out
        if self.level.number < 13 && self.frame_count >= TIMETABLE[TIMETABLE.len() - 1] {
            // YouLose: the tragic ending
            log::info!("time up");
            self.playcut(6, self.level.number, crate::cut::After::TimeUp);
            return;
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
        self.start_level(n, true);
    }

    /// Stand the kid on block (bx, by) of `scrn` (for headless runs and
    /// tests) and show that screen.
    pub fn place_kid(&mut self, scrn: u8, bx: u8, by: u8, face: u8) {
        self.load_kid();
        self.ch.scrn = scrn;
        self.ch.block_x = bx;
        self.ch.block_y = by;
        self.ch.face = face;
        self.ch.x = getblockej(bx as i8).wrapping_add(ANGLE + 7);
        self.ch.y = FLOOR_Y[by as usize + 1];
        self.ch.action = 0;
        self.ch.yvel = 0;
        self.jumpseq(s::STAND);
        self.animchar();
        self.save_kid();
        self.shad.face = crate::guard::NO_CHAR;
        self.message = None;
        self.prepcut(scrn);
    }

    /// REFLECTION (MISC.S): the kid's reflection when he stands before a
    /// mirror; once he has jumped through it, it comes to life as the
    /// shadowman (CreateShad).
    fn reflection(&mut self) {
        self.reflect = None;
        self.load_kid();
        self.getframeinfo();
        if self.createshad {
            self.getreflect();
            self.createshad = false;
            self.ch.id = 1;
            self.add_sound(Sound::MirrorCrack);
            self.save_shad();
            self.max_opp_str = self.max_strength;
            self.opp_strength = self.max_strength;
            self.kid_strength = 1;
            log::info!("the reflection comes to life");
            return;
        }
        if self.getunderft() != crate::level::obj::MIRROR {
            return;
        }
        let bx = self.ch.block_x;
        if self.getreflect() & 0x80 != 0 {
            return; // kid is on the wrong side of the mirror
        }
        let left = (bx as i32 * 4 + 1) * 7;
        self.reflect = Some((self.ch, left));
    }

    /// getreflect: Char becomes the kid's reflection; returns its
    /// distance from the mirror.
    fn getreflect(&mut self) -> u8 {
        let mirrx = getblockej(self.ch.block_x as i8).wrapping_add(ANGLE + 3);
        let mut d = self.getdist();
        if !self.face_left() {
            d = (d ^ 0xff).wrapping_add(14);
        }
        let dmirr = d.wrapping_sub(2);
        self.ch.x = mirrx.wrapping_mul(2).wrapping_sub(self.ch.x);
        self.ch.face ^= 0xff;
        dmirr
    }

    /// misctimers (TOPCTRL.S)
    fn misctimers(&mut self) {
        if self.mergetimer > 0 {
            self.mergetimer -= 1;
            if self.mergetimer == 0 {
                self.mergetimer = -1;
            }
        }
        // Level 8: wait on screen 16 long enough once the exit is open,
        // and the mouse comes to open the gate
        if self.level.number == 8 && self.kid.scrn == 16 && self.exitopen != 0 {
            if self.exitopen == MOUSETIMER {
                self.mouserescue();
                self.exitopen += 1;
            } else if self.exitopen < MOUSETIMER {
                self.exitopen += 1;
            }
        }
    }

    /// DoKid (TOPCTRL.S)
    fn dokid(&mut self, c: Controls) {
        self.load_kid_wop();
        self.rereadblocks();
        self.unholy();
        self.kill0();
        self.playerctrl(c);
        if self.inverted && self.ch.life & 0x80 == 0 {
            // the screen flips back the right way up when you die
            self.inverted = false;
        }
        if self.weightless != 0 {
            self.weightless -= 1;
        }
        if self.ch.scrn != 0 {
            self.animchar();
            self.gravity();
            self.addfall();
            self.setupchar();
            self.rereadblocks();
            self.getedges();
            self.firstguard();
            let (cl, cr) = self.checkbarr();
            self.collisions(cl, cr);
            self.checkgate();
            self.checkfloor();
            self.checkpress();
            self.checkspikes();
            self.checkimpale();
            self.checkslice();
            self.shakeloose();
        }
        self.save_kid();
    }

    /// kill0: on the null screen (fell off the map) the kid dies.
    fn kill0(&mut self) {
        if self.ch.life & 0x80 == 0 || self.ch.scrn != 0 {
            return;
        }
        self.add_sound(Sound::Splat);
        self.decstr(100);
        self.ch.life = 0;
        self.ch.posn = 185;
    }

    /// CUTCHECK / cutchar (AUTO.S): follow the kid off the edge, and take
    /// an en garde guard along or leave him where he is. Returns the
    /// screen to cut to.
    fn cutcheck(&mut self) -> Option<u8> {
        if self.cuttimer != 0 {
            self.cuttimer -= 1;
            return None;
        }
        self.load_kid();
        self.setupchar();
        self.getedges();
        let dir = self.cutchar()?;
        match dir {
            0 => self.milestone3(),
            1 => self.stealsword(),
            _ => {}
        }
        self.cut(dir);
        self.save_kid();
        log::debug!("cut {dir} to screen {}", self.kid.scrn);
        if self.shad.face != crate::guard::NO_CHAR {
            self.cutguards(dir);
        }
        Some(self.kid.scrn)
    }

    /// PrepCut: show screen `scrn` and wake up what is on it.
    fn prepcut(&mut self, scrn: u8) {
        if scrn == 0 || scrn == self.vis_scrn {
            return;
        }
        self.vis_scrn = scrn;
        // drawbg: at least two frames before the next cut
        self.cuttimer = 2;
        if self.level.number == 14 && scrn == 5 {
            // the princess's room: YouWin, the happy ending
            log::info!("reached the princess");
            self.playcut(7, 14, crate::cut::After::Won);
            return;
        }
        self.load_kid();
        self.addslicers();
        self.trigger_screen_objects();
        self.crumble();
        self.addguard();
    }

    fn cutchar(&mut self) -> Option<u8> {
        const TOP_PL: u8 = 10;
        const TOP_MI: u8 = (-16i8) as u8;
        const BOT: u8 = BOT_CUT_EDGE;
        let y = self.ch.y;
        let falling = matches!(self.ch.action, 5 | 4 | 3);
        if !falling && (y < TOP_PL || y >= TOP_MI) {
            return Some(2);
        }
        if y >= BOT {
            if self.level.number == 6 && self.ch.scrn == 1 {
                return None;
            }
            return Some(3);
        }
        let p = self.ch.posn;
        if (135..150).contains(&p) || (110..120).contains(&p) || (150..163).contains(&p) || (166..169).contains(&p) {
            return None;
        }
        if self.ch.action == 7 {
            return None;
        }
        const LEFT_CUT: u8 = SCRN_LEFT - 4;
        const RIGHT_CUT: u8 = SCRN_LEFT + 140 - 1 + 4;
        if self.ch.face & 0x80 != 0 {
            let l = self.edges.left;
            if l <= LEFT_CUT {
                return Some(0);
            }
            if l >= SCRN_LEFT + 140 {
                return Some(1);
            }
            return None;
        }
        let (s, by) = (self.ch.scrn, self.ch.block_y as i8 as i32);
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

    /// CUT: move Char to the screen in direction `dir` (0 left, 1 right,
    /// 2 up, 3 down).
    pub fn cut(&mut self, dir: u8) {
        let l = &self.level;
        let k = &mut self.ch;
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
    }

    /// milestone3: on level 3, passing the first gate moves the restart point.
    fn milestone3(&mut self) {
        if self.level.number == 3 && self.ch.scrn == 7 && !self.milestone {
            self.milestone = true;
            log::info!("level 3 milestone");
        }
    }

    /// stealsword: on level 12 the sword is gone once you reach the screen
    /// below where it lay (the shadow has it).
    fn stealsword(&mut self) {
        if self.level.number == 12 && self.ch.scrn == 18 {
            let i = Level::index(crate::guard::SWORDSCRN, 1, 0);
            self.level.set_type(i, crate::level::obj::FLOOR);
        }
    }

    /// JUMPSEQ
    pub fn jumpseq(&mut self, n: u8) {
        self.ch.seq = seq::start(n);
    }

    // CTRLSUBS.S: load and save the character buffers
    pub fn load_kid(&mut self) {
        self.ch = self.kid;
    }
    pub fn save_kid(&mut self) {
        self.kid = self.ch;
    }
    pub fn load_shad(&mut self) {
        self.ch = self.shad;
    }
    pub fn save_shad(&mut self) {
        self.shad = self.ch;
    }
    pub fn load_kid_wop(&mut self) {
        self.ch = self.kid;
        self.op = self.shad;
    }
    pub fn save_kid_wop(&mut self) {
        self.kid = self.ch;
        self.shad = self.op;
    }
    pub fn load_shad_wop(&mut self) {
        self.ch = self.shad;
        self.op = self.kid;
    }
    pub fn save_shad_wop(&mut self) {
        self.shad = self.ch;
        self.kid = self.op;
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

#[cfg(test)]
mod exit_tests {
    use super::*;
    use crate::level::Level;

    /// Stand the kid on block (bx, by) of `scrn`, facing `face`.
    fn place(g: &mut Game, scrn: u8, bx: u8, by: u8, face: u8) {
        g.place_kid(scrn, bx, by, face);
    }

    /// Open the level 1 exit, stand the kid in its doorway, hold Up.
    fn climb(bx: u8, face: u8) -> u8 {
        let mut g = Game::new(1);
        let i = Level::index(9, 3, 1);
        g.level.bluespec[i] = crate::mover::EMAXVAL;
        place(&mut g, 9, bx, 1, face);
        for _ in 0..4 {
            g.tick(Controls::default());
        }
        for _ in 0..80 {
            g.tick(Controls { x: 0, y: -1, btn: false });
            if g.phase == Phase::Cut {
                break;
            }
        }
        // the princess's room comes between levels 1 and 2
        g.cut.as_ref().map_or(g.level.number, |c| c.level)
    }

    #[test]
    fn up_in_the_open_exit_climbs_to_the_next_level() {
        // the regression: a `do 0` block in SEQTABLE.S was assembled, so
        // climbstairs walked back down to `stand` instead of `nextlevel`
        for (bx, face) in [(3, FACE_LEFT), (3, FACE_RIGHT), (4, FACE_LEFT), (4, FACE_RIGHT)] {
            assert_eq!(climb(bx, face), 2, "from block {bx}, face {face:#x}");
        }
    }

    #[test]
    fn the_upside_down_potion_flips_the_picture_until_you_die() {
        let mut g = Game::new(2);
        place(&mut g, 5, 3, 1, FACE_LEFT);
        let mut normal = crate::apple::Screen::default();
        crate::bg::draw(&mut normal, &g);
        g.load_kid();
        g.potion = 4;
        g.jumpseq(crate::seq::seq::DRINKPOTION);
        for _ in 0..40 {
            g.animchar();
        }
        g.save_kid();
        assert!(g.inverted);
        place(&mut g, 5, 3, 1, FACE_LEFT);
        let mut flipped = crate::apple::Screen::default();
        crate::bg::draw(&mut flipped, &g);
        let w = crate::apple::SCREEN_W as usize;
        let row = |s: &crate::apple::Screen, y: usize| s.px[y * w..(y + 1) * w].to_vec();
        assert_eq!(row(&normal, 10), row(&flipped, 181));
        g.kid.life = 0;
        g.tick(Controls::default());
        assert!(!g.inverted);
    }

    #[test]
    fn level_7_drop_in_is_survived_by_grabbing_the_ledge() {
        // STARTKID drops the kid into screen 1 from screen 17; the shaft
        // under him falls to screen 3, the floor to his left is one storey
        // down. Holding the button through the drop catches that ledge.
        let mut g = Game::new(7);
        let btn = Controls { x: 0, y: 0, btn: true };
        let climb = Controls { x: 0, y: -1, btn: true };
        for _ in 0..20 {
            g.tick(btn);
        }
        assert_eq!((g.kid.scrn, g.kid.action), (1, 6), "hanging on the ledge");
        for _ in 0..20 {
            g.tick(climb);
        }
        for _ in 0..30 {
            g.tick(Controls::default());
        }
        assert!(g.kid.life & 0x80 != 0);
        assert_eq!((g.kid.scrn, g.kid.block_y, g.kid.posn), (1, 0, 15), "standing on screen 1");
        assert_eq!(g.kid_strength, g.max_strength);
    }

    #[test]
    fn a_closed_exit_does_not_let_you_climb() {
        let mut g = Game::new(1);
        place(&mut g, 9, 3, 1, FACE_LEFT);
        for _ in 0..60 {
            g.tick(Controls { x: 0, y: -1, btn: false });
        }
        assert_eq!(g.level.number, 1);
    }
}

#[cfg(test)]
mod time_tests {
    use super::*;

    fn run(g: &mut Game, frames: u32) {
        for _ in 0..frames {
            g.tick(Controls::default());
        }
    }

    #[test]
    fn getminleft_counts_725_frame_minutes_then_seconds() {
        let mut g = Game::new(1);
        assert_eq!(g.time_left(), (60, None));
        g.frame_count = MIN_FRAMES - 1;
        assert_eq!(g.time_left(), (60, None));
        g.frame_count = MIN_FRAMES;
        assert_eq!(g.time_left(), (59, None));
        g.frame_count = 58 * MIN_FRAMES;
        assert_eq!(g.time_left(), (2, None));
        g.frame_count = 59 * MIN_FRAMES + 1;
        assert_eq!(g.time_left(), (1, Some(60)));
        g.frame_count = 59 * MIN_FRAMES + 60 * SEC_FRAMES;
        assert_eq!(g.time_left(), (1, Some(0)));
    }

    #[test]
    fn the_clock_stops_while_the_kid_is_dead() {
        let mut g = Game::new(1);
        run(&mut g, 10);
        let f = g.frame_count;
        assert!(f > 0);
        g.kid.life = 0;
        run(&mut g, 10);
        assert_eq!(g.frame_count, f);
    }

    #[test]
    fn time_runs_out_five_frames_after_t_zero() {
        let mut g = Game::new(2);
        g.frame_count = MINUTES * MIN_FRAMES + 3;
        g.next_time_msg = TIMETABLE.len() - 1;
        run(&mut g, 1);
        assert_eq!(g.phase, Phase::Playing);
        run(&mut g, 1);
        assert_eq!(g.phase, Phase::Cut, "the tragic ending first");
        assert_eq!(g.cut.as_ref().map(|c| c.num), Some(6));
        run(&mut g, 400);
        assert_eq!(g.phase, Phase::TimeUp);
    }

    #[test]
    fn the_final_minute_counts_down_on_screen() {
        let mut g = Game::new(2);
        g.message = None;
        g.frame_count = 59 * MIN_FRAMES;
        g.next_time_msg = 15;
        run(&mut g, 2);
        assert_eq!(g.message.as_ref().map(|(m, _)| m.as_str()), Some("60 SECONDS LEFT"));
        run(&mut g, SEC_FRAMES * 10);
        assert_eq!(g.message.as_ref().map(|(m, _)| m.as_str()), Some("50 SECONDS LEFT"));
    }

    #[test]
    fn a_new_level_shows_the_time_after_its_name() {
        let mut g = Game::new(2);
        g.phase = Phase::LevelDone(0);
        run(&mut g, 1);
        assert_eq!(g.level.number, 3);
        assert!(g.message.as_ref().is_some_and(|(m, _)| m.starts_with("LEVEL")));
        run(&mut g, 3 * FPS + 1);
        assert!(g.message.as_ref().is_some_and(|(m, _)| m.ends_with("MINUTES LEFT")), "{:?}", g.message);
    }

    #[test]
    fn skipping_a_level_leaves_15_minutes() {
        let mut g = Game::new(1);
        g.shortentime();
        assert_eq!(g.time_left(), (15, None));
        g.frame_count = 50 * MIN_FRAMES;
        g.next_time_msg = 11;
        g.shortentime();
        assert_eq!(g.time_left(), (10, None));
    }
}
