//! Guards, the shadowman and sword fighting: a port of AUTO.S (enemy
//! control, sword contact, guards entering and leaving the screen), the
//! fight routines of MISC.S (FIRSTGUARD, STABCHAR, UNHOLY, CHECKALERT,
//! BONESRISE, VANISHCHAR, DECSTR) and TOPCTRL.S (DoShad, chgmeters).
//!
//! As in the original, every routine works on the Char buffer (`ch`) with
//! its opponent in `op`: DoShad loads the guard into `ch` and the kid into
//! `op`, runs the same control and physics code the kid uses, and saves
//! the guard back. The AI "presses keys" the way the player does (DoFwd,
//! DoStrike, ...) and GENCTRL acts on them.

use crate::game::*;
use crate::level::{gd, is_space, obj};
use crate::seq::{self, seq as s};
use crate::subs::cmpbarr;

/// ShadFace value meaning "no character".
pub const NO_CHAR: u8 = 86;

// Strike/block ranges and thresholds (AUTO.S)
const STRIKERANGE1: u8 = 12;
const STRIKERANGE2: u8 = 29;
const BLOCKRANGE1: u8 = 0;
const BLOCKRANGE2: u8 = 29;
const SWORDTHRES: u8 = 90;
const STRIKETHRES1: u8 = STRIKERANGE1;
const STRIKETHRES2: u8 = STRIKERANGE2;
const BLOCKTHRES1: u8 = 10;
const BLOCKTHRES2: u8 = BLOCKRANGE2;
const TOOCLOSE: u8 = STRIKETHRES1;
const TOOFAR: u8 = STRIKETHRES2 + 6;
const OFFGUARDTHRES: u8 = 8;
const JUMPTHRES: u8 = 50;
const RUNTHRES: u8 = 40;
const BLOCKTIME: u8 = 4;
const ESTWIDTH: u8 = 13;

/// Fighter parameters, indexed by guardprog (AUTO.S): probabilities x 255
/// of striking from ready, restriking after a block, blocking, blocking
/// while impaired, advancing; the refractory period after being hit; the
/// uniform colour swap and extra strength.
const STRIKEPROB: [u8; 12] = [75, 100, 75, 75, 75, 50, 100, 220, 0, 60, 40, 60];
const RESTRIKEPROB: [u8; 12] = [0, 0, 0, 5, 5, 175, 20, 10, 0, 255, 255, 150];
const BLOCKPROB: [u8; 12] = [0, 150, 150, 200, 200, 255, 200, 250, 0, 255, 255, 255];
const IMPBLOCKPROB: [u8; 12] = [0, 75, 75, 100, 100, 145, 100, 250, 0, 145, 255, 175];
const ADVPROB: [u8; 12] = [255, 200, 200, 200, 255, 255, 200, 0, 0, 255, 100, 100];
const REFRACTIMER: [u8; 12] = [20, 20, 20, 20, 10, 10, 10, 10, 0, 10, 0, 0];
const SPECIALCOLOR: [u8; 12] = [0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 1];
const EXTRASTRENGTH: [u8; 12] = [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
const NUMPROGS: u8 = 12;

/// Basic guard strength and uniform colour by level (0 blue, 1 red).
const BASICSTRENGTH: [u8; 15] = [4, 3, 3, 3, 3, 4, 5, 4, 4, 5, 5, 5, 4, 6, 6];
const BASICCOLOR: [u8; 15] = [1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0];
const SHADSTRENGTH: u8 = 4;

// Special objects
const FLASKSCRN: u8 = 24;
const FLASKX: i32 = 3;
pub const SWORDSCRN: u8 = 15;
const SWORDX: i32 = 1;
const MIRSCRN: u8 = 4;

// BONESRISE
const SKELSCRN: u8 = 1;
const SKELX: i32 = 5;
const SKELY: u8 = 1;
const SKELTRIG: u8 = 2;
const SKELPROG: u8 = 2;

/// Shadowman start positions: Posn, X, Y, Face, BlockX, BlockY, Action,
/// then the sequence.
const SHADPOS6A: ([u8; 7], u8) = ([0x0f, 0x51, 0x76, 0x00, 0x00, 0x01, 0x00], s::STAND);
const SHADPOS5: ([u8; 7], u8) = ([0x0f, 0x37, 0x37, 0x00, 0xff, 0x00, 0x00], s::STAND);
const SHADPOS12: ([u8; 7], u8) = ([0x0f, 0x51, 0xf0, 0x00, 0x00, 0x00, 0x00], s::STEPFALL);

/// Prerecorded moves for the level 5 thief: (frame, command).
const SHADPROG5: [(u8, i8); 8] = [(0, CTR), (1, FWD), (14, CTR), (18, PRESS), (29, RELEASE), (45, BACK), (49, FWD), (255, END_PROG)];
const END_PROG: i8 = -2;
const END_DEMO: i8 = -1;
const CTR: i8 = 0;
const FWD: i8 = 1;
const BACK: i8 = 2;
const UP: i8 = 3;
const DOWN: i8 = 4;
const UPFWD: i8 = 5;
const PRESS: i8 = 6;
const RELEASE: i8 = 7;

/// DemoProg1 (SUBS.S): the attract demo's kid, up to the fight with the
/// first guard.
const DEMOPROG1: [(u8, i8); 25] = [
    (0, CTR),
    (1, FWD),
    (13, CTR),
    (30, FWD), // start running...
    (37, UPFWD), // jump 1st pit
    (47, CTR),
    (48, FWD), // & keep running
    (65, CTR), // stop
    (73, BACK), // look back...
    (75, CTR),
    (99, BACK),
    (100, CTR),
    (115, UPFWD), // jump 2nd pit
    (128, PRESS), // & grab ledge
    (136, UP),
    (157, RELEASE),
    (158, CTR),
    (159, FWD),
    (173, DOWN),
    (177, CTR),
    (178, FWD),
    (188, CTR),
    (193, FWD),
    (205, CTR),
    (233, END_DEMO),
];

impl Game {
    /// GETOPDIST: pixels Char must move forward to reach the opponent
    /// (signed byte, clamped to +-127; 127 if on another screen).
    pub fn getopdist(&self) -> u8 {
        let (c, o) = (self.ch, self.op);
        if c.scrn != o.scrn {
            return 127;
        }
        let mut a = if o.x >= c.x {
            let d = o.x - c.x;
            if d & 0x80 != 0 {
                127
            } else {
                d
            }
        } else {
            let d = c.x - o.x;
            (if d & 0x80 != 0 { 127 } else { d }).wrapping_neg()
        };
        if c.face & 0x80 != 0 {
            a = a.wrapping_neg();
        }
        // facing each other: allow for the width of the figures
        if (c.face ^ o.face) & 0x80 != 0 && a < 127 - ESTWIDTH {
            a += ESTWIDTH;
        }
        a
    }

    fn rndp(&mut self) -> (u8, usize) {
        let x = self.guardprog.min(NUMPROGS - 1) as usize;
        (self.rnd(), x)
    }

    // ------------------------------------------------------------------
    // The AI's "joystick" (AUTO.S DoRelease ... DoRelBtn)

    fn dorelease(&mut self) {
        self.joy = Joy::default();
    }
    fn cmd_fwd(&mut self) {
        self.joy.clr_f = -1;
        self.joy.jstk_x = -1;
    }
    fn cmd_back(&mut self) {
        self.joy.clr_b = -1;
        self.joy.jstk_x = 1;
    }
    fn cmd_up(&mut self) {
        self.joy.clr_u = -1;
        self.joy.jstk_y = -1;
    }
    fn cmd_down(&mut self) {
        self.joy.clr_d = -1;
        self.joy.jstk_y = 1;
    }
    fn cmd_dropguard(&mut self) {
        self.joy.clr_d = -1;
        self.cmd_back();
    }
    fn cmd_engarde(&mut self) {
        self.joy.clr_d = -1;
        self.cmd_fwd();
    }
    fn cmd_strike(&mut self) {
        self.joy.clr_btn = -1;
        self.joy.btn = true;
    }
    fn cmd_relbtn(&mut self) {
        self.joy.btn = false;
    }

    /// AUTOCTRL: automatic enemy control.
    fn autoctrl(&mut self) {
        self.dorelease();
        if self.justblocked != 0 {
            self.justblocked -= 1;
        }
        if self.gdtimer != 0 {
            self.gdtimer -= 1;
        }
        if self.refract != 0 {
            self.refract -= 1;
        }
        match self.ch.id {
            24 => self.mouseprog(),
            4 => {
                // SkelProg
                self.ch.sword = 2;
                self.guardprogram();
            }
            id if id >= 2 => self.guardprogram(),
            // KidProg: the kid fighting in the demo
            0 => self.guardprogram(),
            _ => self.shadowprog(),
        }
    }

    fn mouseprog(&mut self) {
        if self.ch.face == NO_CHAR {
            return;
        }
        if self.ch.action != 0 {
            if self.ch.x < 166 {
                self.jumpseq(s::MLEAVE);
                self.animchar();
            }
            return;
        }
        if self.ch.x >= 200 {
            self.vanishchar();
        }
    }

    fn shadowprog(&mut self) {
        match self.level.number {
            4 => {
                // runs off to the left out of the mirror, then is gone
                if self.ch.scrn == MIRSCRN {
                    if self.ch.x < 80 {
                        self.vanishchar();
                    } else {
                        self.cmd_fwd();
                    }
                }
            }
            5 => self.shad_level5(),
            6 => {
                // when the kid jumps the chasm, step onto the plate
                if self.ch.scrn == 1 && self.kid.posn == 43 && self.kid.x < 0x80 {
                    self.cmd_strike();
                    self.cmd_fwd();
                }
            }
            12 => self.finalshad(),
            _ => {}
        }
    }

    /// Level 5: when the gate opens, steal the potion.
    fn shad_level5(&mut self) {
        if self.ch.scrn != FLASKSCRN {
            return;
        }
        if self.playcount == 0 {
            self.rdblock(FLASKSCRN, 1, 0);
            if self.spec() < 20 {
                return;
            }
            self.prerecptr = 0;
        }
        self.autoplayback(&SHADPROG5);
        if self.ch.x < 15 {
            self.vanishchar();
        }
    }

    /// DemoCtrl (CTRL.S): the kid in the demo plays back DemoProg1, and
    /// once en garde fights as a guard would (programs 10 and 11).
    pub(crate) fn democtrl(&mut self) {
        if self.ch.sword != 0 {
            self.guardprog = 10;
            self.autoctrl();
            self.guardprog = 11;
        } else {
            self.autoplayback(&DEMOPROG1);
        }
    }

    /// AUTOPLAYBACK: replay a prerecorded list of moves.
    fn autoplayback(&mut self, prog: &[(u8, i8)]) {
        if self.playcount >= 254 {
            return;
        }
        self.playcount += 1;
        let i = self.prerecptr.min(prog.len() - 1);
        let cmd = if self.playcount >= prog[i].0 {
            self.prerecptr = i + 1;
            prog[i].1
        } else {
            // not there yet: repeat the last command
            prog[i.saturating_sub(1)].1
        };
        match cmd {
            CTR => self.dorelease(),
            FWD => self.cmd_fwd(),
            BACK => self.cmd_back(),
            UP => self.cmd_up(),
            DOWN => self.cmd_down(),
            UPFWD => {
                self.cmd_up();
                self.cmd_fwd();
            }
            PRESS => self.cmd_strike(),
            RELEASE => self.cmd_relbtn(),
            END_DEMO => self.demo_over = true,
            _ => {}
        }
    }

    /// Level 12: the final battle, and the kid and shadow made one.
    fn finalshad(&mut self) {
        if self.ch.scrn == SWORDSCRN && self.shadowaction == 0 {
            if self.op.x >= 150 {
                // hold the shadow at the top until the kid arrives
                self.csps(SHADPOS12);
                return;
            }
            self.shadowaction = 1;
        }
        if self.ch.sword >= 2 {
            // normal fighting
            if self.offguard && self.refract == 0 {
                self.cmd_down();
            } else {
                self.engarde();
            }
            return;
        }
        if self.op.sword >= 2 || !self.offguard {
            // hostile
            if self.enemy_alert >= 2 && self.getopdist() < SWORDTHRES {
                if self.ch.posn == 15 {
                    self.cmd_engarde();
                }
                return;
            }
            if self.getopdist() & 0x80 != 0 {
                self.cmd_back();
            }
            return;
        }
        // face to face, swords down
        if self.getopdist() & 0x80 != 0 {
            return self.merge();
        }
        if self.enemy_alert != 2 {
            return;
        }
        let p = self.op.posn;
        // if the kid starts moving towards you (startrun, stepfwd), do the same
        if (3..15).contains(&p) || (127..133).contains(&p) {
            self.cmd_fwd();
        }
    }

    /// Kid and shadow reunite.
    fn merge(&mut self) {
        log::info!("kid and shadow merge");
        self.lightcolor = 0xff;
        self.lightning = 10;
        self.boostmeter();
        self.cuesong(crate::music::Song::Prince);
        self.mergetimer = 42;
        self.ch.id = 0;
        self.save_kid();
        self.vanishchar();
    }

    /// GuardProg
    fn guardprogram(&mut self) {
        if self.ch.sword >= 2 {
            self.engarde();
        } else {
            self.alert();
        }
    }

    /// Alert (not en garde): turn to face the kid, go en garde on sight.
    fn alert(&mut self) {
        if self.kid.life & 0x80 == 0 {
            return; // kid's dead: relax
        }
        let a = self.getopdist();
        let on_top = self.op.block_y == self.ch.block_y && a >= (-8i8) as u8;
        if !on_top {
            if self.alertguard {
                self.alertguard = false;
                if a >= 128 {
                    if a < (-4i8) as u8 {
                        return self.cmd_down(); // turn around
                    }
                    return; // overlapping: stand still
                }
            } else if a >= 128 {
                return; // kid is behind you
            }
        }
        if self.enemy_alert == 0 {
            return;
        }
        // the vizier only: wait for his music
        if self.level.number == 13 && self.jaffar_wait > 0 {
            return;
        }
        log::debug!("guard goes en garde, dist {}", a as i8);
        self.cmd_engarde();
    }

    /// EnGarde: fighting stance decisions.
    fn engarde(&mut self) {
        let p = self.ch.posn;
        if p == 166 || p < 150 {
            return; // wait till you're ready
        }
        if self.enemy_alert < 2 {
            if self.enemy_alert == 1 {
                return; // in sight but a gap or barrier between: stay put
            }
            if self.droppedout {
                return self.followkid();
            }
            if self.ch.id != 4 {
                self.cmd_dropguard();
            }
            return;
        }
        // clear floor to the kid; if he is recovering from a bump, let him
        let a = self.getopdist();
        if (12..128).contains(&a) && (102..118).contains(&self.op.posn) && self.op.action == 5 {
            return;
        }
        let a = self.getopdist();
        if a >= TOOFAR {
            return self.outofrange();
        }
        let close = if self.ch.sword >= 2 { a < TOOCLOSE } else { a < OFFGUARDTHRES };
        if close {
            if self.ch.face == self.op.face {
                self.cmd_back();
            } else {
                self.cmd_fwd();
            }
            return;
        }
        self.inrange();
    }

    fn outofrange(&mut self) {
        if self.refract != 0 {
            return;
        }
        if self.ch.face != self.op.face {
            let p = self.op.posn;
            if (7..15).contains(&p) {
                // kid running at you: cut him down
                if self.getopdist() < RUNTHRES {
                    self.cmd_strike();
                }
                return;
            }
            if (34..44).contains(&p) {
                // kid jumping at you
                if self.getopdist() < JUMPTHRES {
                    self.cmd_strike();
                }
                return;
            }
        }
        // don't advance unless there is solid floor
        if is_space(self.getinfront()) || is_space(self.get2infront()) {
            self.cmd_back();
        } else {
            self.cmd_fwd();
        }
    }

    /// FollowKid: the kid has dropped out of the fight. Advance to the
    /// edge, and follow him down only onto safe floor one storey below.
    fn followkid(&mut self) {
        if matches!(self.op.action, 2 | 6) {
            return; // kid is hanging on a ledge
        }
        let id = self.getinfront();
        if cmpbarr(id) == 0 {
            if !is_space(id) {
                return self.cmd_fwd();
            }
            self.getinfront();
            self.rd.by += 1;
            let z = self.rdblock1();
            let safe = z != obj::SPIKES && z != obj::LOOSE && cmpbarr(z) == 0 && !is_space(z);
            if safe && self.ch.block_y.wrapping_add(1) == self.op.block_y {
                return self.cmd_fwd();
            }
        }
        self.droppedout = false;
        self.cmd_back();
    }

    fn inrange(&mut self) {
        if self.op.sword == 2 {
            return self.genfight();
        }
        // opponent unarmed or off guard: maul him
        if self.refract != 0 {
            return;
        }
        if self.getopdist() >= STRIKETHRES2 {
            self.cmd_fwd();
        } else {
            self.cmd_strike();
        }
    }

    /// GenFight: en garde, face to face and too close to advance safely.
    fn genfight(&mut self) {
        let a = self.getopdist();
        if !(BLOCKTHRES1..BLOCKTHRES2).contains(&a) {
            return self.maybeadvance();
        }
        self.maybeblock();
        if self.refract != 0 {
            return;
        }
        let a = self.getopdist();
        if !(STRIKETHRES1..STRIKETHRES2).contains(&a) {
            return self.maybeadvance();
        }
        self.maybestrike();
    }

    fn maybeadvance(&mut self) {
        if self.guardprog != 0 && self.gdtimer != 0 {
            return;
        }
        let (r, x) = self.rndp();
        if r < ADVPROB[x] {
            self.cmd_fwd();
        }
    }

    fn maybeblock(&mut self) {
        if !matches!(self.op.posn, 152 | 153 | 162) {
            return;
        }
        let (r, x) = self.rndp();
        let prob = if self.justblocked != 0 { IMPBLOCKPROB[x] } else { BLOCKPROB[x] };
        if r < prob {
            self.cmd_up();
        }
    }

    fn maybestrike(&mut self) {
        if matches!(self.op.posn, 169 | 151) {
            return; // opponent starting to strike
        }
        let (r, x) = self.rndp();
        let prob = if matches!(self.ch.posn, 161 | 150) { RESTRIKEPROB[x] } else { STRIKEPROB[x] };
        if r < prob {
            self.cmd_strike();
        }
    }

    // ------------------------------------------------------------------
    // Sword contact

    /// CHECKSTRIKE: both characters test for sword contact.
    pub fn checkstrike(&mut self) {
        let p = self.kid.posn;
        if p == 0 || (219..229).contains(&p) {
            return;
        }
        self.load_shad_wop();
        self.teststrike();
        self.save_shad_wop();
        self.load_kid_wop();
        self.teststrike();
        self.save_kid_wop();
    }

    fn teststrike(&mut self) {
        if self.ch.sword != 2 || self.ch.block_y != self.op.block_y {
            return;
        }
        let p = self.ch.posn;
        if p != 153 && p != 154 {
            return; // not on a strike frame
        }
        let a = self.getopdist();
        if (BLOCKRANGE1..BLOCKRANGE2).contains(&a) && matches!(self.op.posn, 161 | 150) {
            // the opponent blocks
            self.op.posn = 161;
            if self.ch.id != 0 {
                self.justblocked = BLOCKTIME;
            }
            log::debug!("char {} strike blocked", self.ch.id);
            self.jumpseq(s::BLOCKEDSTRIKE);
            self.animchar();
            return;
        }
        if p != 154 {
            return;
        }
        let a = self.getopdist();
        let near = if self.op.sword >= 2 { STRIKERANGE1 } else { OFFGUARDTHRES };
        if a < near || a >= STRIKERANGE2 {
            return;
        }
        self.op.action = 99; // stabbed
    }

    /// CHECKSTAB: act on the strikes CHECKSTRIKE found.
    pub fn checkstab(&mut self) {
        if self.shad.action == 99 {
            if self.kid.action == 99 {
                // both lunge at once: the player wins a tie
                self.kid.action = 1;
            }
            self.load_shad();
            self.stabchar();
            self.save_shad();
            let (_, x) = self.rndp();
            self.refract = REFRACTIMER[x];
        }
        if self.kid.action == 99 {
            self.load_kid();
            self.stabchar();
            self.save_kid();
        }
    }

    /// STABCHAR
    fn stabchar(&mut self) {
        if self.ch.life & 0x80 == 0 {
            return; // already dead
        }
        log::info!("char {} stabbed, sword {}", self.ch.id, self.ch.sword);
        let seq = if self.ch.sword != 2 {
            // stabbed when defenceless
            self.decstr(100);
            None
        } else if self.ch.id == 4 || self.decstr(1) {
            // the skeleton has no life points to lose
            Some(s::STABBED)
        } else {
            None
        };
        let seq = match seq {
            Some(q) => q,
            None => {
                // killed: at an edge, knocked off it
                if self.getbehind() == obj::SPACE && self.getdist() >= 4 {
                    let d = self.getdist().wrapping_sub(14);
                    self.ch.x = self.addcharx(d);
                    self.ch.block_y = self.ch.block_y.wrapping_add(1);
                    self.jumpseq(s::FIGHTFALL);
                    self.add_sound(Sound::Splat);
                    self.animchar();
                    return;
                }
                s::STABKILL
            }
        };
        self.jumpseq(seq);
        let by = self.ch.block_y as i8 as i32;
        self.ch.y = FLOOR_Y[(by + 1).clamp(0, 4) as usize];
        self.ch.yvel = 0;
        self.add_sound(Sound::Splat);
        self.animchar();
    }

    /// addsfx: the clash of a blocked strike.
    pub fn addsfx(&mut self) {
        if self.kid.posn == 167 || self.shad.posn == 167 {
            self.add_sound(Sound::SwordClash);
        }
    }

    // ------------------------------------------------------------------
    // Strength

    /// DECSTR: lose strength (Char's, or the opponent's if Char is not the
    /// kid); false if that kills.
    pub fn decstr(&mut self, n: u8) -> bool {
        let (st, chg) = if self.ch.id == 0 {
            (self.kid_strength, &mut self.chg_kid_str)
        } else {
            (self.opp_strength, &mut self.chg_opp_str)
        };
        if n >= st {
            *chg = st.wrapping_neg();
            log::info!("char {} strength -{st}: killed", self.ch.id);
            false
        } else {
            *chg = n.wrapping_neg();
            log::info!("char {} strength -{n}", self.ch.id);
            true
        }
    }

    /// RECHARGEMETER
    pub fn rechargemeter(&mut self) {
        self.chg_kid_str = self.max_strength.wrapping_sub(self.kid_strength);
    }

    /// BOOSTMETER: one more strength, then recharge.
    pub fn boostmeter(&mut self) {
        if self.max_strength < 10 {
            self.max_strength += 1;
        }
        self.rechargemeter();
    }

    /// chgmeters (TOPCTRL.S): apply this frame's strength changes.
    pub fn chgmeters(&mut self) {
        if self.level.number == 12 && self.op.id | self.ch.id == 1 {
            // kid vs. shadow: what hurts one hurts the other
            if self.chg_kid_str & 0x80 != 0 {
                self.chg_opp_str = self.chg_kid_str;
            } else if self.chg_opp_str & 0x80 != 0 {
                self.chg_kid_str = self.chg_opp_str;
            }
        }
        let k = self.kid_strength.wrapping_add(self.chg_kid_str);
        if k <= self.max_strength {
            self.kid_strength = k;
        }
        let o = self.opp_strength.wrapping_add(self.chg_opp_str);
        if o <= self.max_opp_str {
            self.opp_strength = o;
        }
    }

    /// getgdstrength. Note that ADDGUARD calls this before it sets
    /// guardprog for the new guard, so the extra strength comes from the
    /// previous guard's program; that is kept as the original has it.
    fn getgdstrength(&mut self) {
        let lvl = self.level.number as usize;
        let x = self.guardprog.min(NUMPROGS - 1) as usize;
        self.max_opp_str = BASICSTRENGTH[lvl] + EXTRASTRENGTH[x];
        self.opp_strength = self.max_opp_str;
    }

    // ------------------------------------------------------------------
    // DoShad and the routines it shares with DoKid

    /// DoShad (TOPCTRL.S): move the shadowman or other opponent.
    pub fn doshad(&mut self) {
        if self.shad.face == NO_CHAR {
            return;
        }
        let kid_joy = self.joy;
        self.load_shad_wop();
        self.rereadblocks();
        self.unholy();
        self.shadctrl();
        if self.ch.scrn == self.vis_scrn {
            self.animchar();
            let x = self.ch.x;
            if (SCRN_LEFT - 14..SCRN_RIGHT + 14).contains(&x) {
                self.gravity();
                self.addfall();
                self.setupchar();
                self.rereadblocks();
                self.getedges();
                self.enemycoll();
                self.checkfloor();
                self.checkpress();
                self.checkspikes();
                self.checkimpale();
                self.checkslice2();
            }
        }
        self.save_shad();
        self.joy = kid_joy;
    }

    /// SHADCTRL (CTRL.S)
    fn shadctrl(&mut self) {
        if self.ch.id == 24 {
            return self.autoctrl();
        }
        let alive = self.ch.life & 0x80 != 0;
        if alive && self.opp_strength == 0 {
            self.ch.life = 0;
            log::info!("guard {} dies", self.ch.id);
            self.deadenemy();
        }
        if (!alive || self.ch.life == 0) && self.ch.id == 1 {
            return self.vanishchar();
        }
        self.autoctrl();
        self.genctrl();
    }

    /// VANISHCHAR
    pub fn vanishchar(&mut self) {
        self.ch.face = NO_CHAR;
        self.ch.action = 0;
        self.ch.life = 0;
        self.chg_opp_str = self.opp_strength.wrapping_neg();
    }

    /// DEADENEMY: music for a victory; on level 13 the vizier's death
    /// opens the exit.
    fn deadenemy(&mut self) {
        if self.level.number != 13 && self.ch.id != 1 {
            self.cuesong(crate::music::Song::Victory);
        }
        if self.level.number == 13 {
            self.cuesong(crate::music::Song::JaffarDead);
            self.lightcolor = 0xff;
            self.lightning = 10;
            self.exitopen = 1;
            // the clock stops: show the time left
            self.time_request = 4;
            self.rdblock(24, 0, 0);
            self.pushpp();
            log::info!("vizier dead: exit opens");
        }
    }

    /// UNHOLY: on level 12, if the shadow dies the kid dies (and the other
    /// way round).
    pub fn unholy(&mut self) {
        if self.level.number != 12 || self.op.id | self.ch.id != 1 {
            return;
        }
        if self.ch.life & 0x80 == 0 || self.op.life & 0x80 != 0 {
            return;
        }
        self.lightcolor = 0xff;
        self.lightning = 5;
        self.add_sound(Sound::Splat);
        self.decstr(100);
    }

    /// FIRSTGUARD: the kid can't run or jump past an en garde guard.
    pub fn firstguard(&mut self) {
        if self.enemy_alert < 2 || self.ch.sword != 0 || self.op.sword == 0 || self.op.action >= 2 {
            return;
        }
        if self.ch.face == self.op.face || self.getopdist() < (-15i8) as u8 {
            return;
        }
        log::debug!("bumped off the guard");
        let by = self.ch.block_y as i8 as i32;
        self.ch.y = FLOOR_Y[(by + 1).clamp(0, 4) as usize];
        self.jumpseq(s::BUMP);
        self.animchar();
    }

    /// ENEMYCOLL (COLL.S): an en garde enemy backing into a wall or gate.
    fn enemycoll(&mut self) {
        if self.ch.action != 1 || self.ch.life & 0x80 == 0 || self.ch.sword < 2 {
            return;
        }
        let mut id = self.getunderft();
        let mut hit = id == obj::BLOCK || id == obj::PANELWIF || (id == obj::GATE && !self.gate_open());
        if !hit && self.ch.face & 0x80 == 0 {
            // facing right: check the block behind too
            self.rd.bx -= 1;
            id = self.rdblock1();
            hit = id == obj::PANELWIF || (id == obj::GATE && !self.gate_open());
        }
        if !hit {
            return;
        }
        self.setupchar();
        self.getedges();
        let (sc, bx, by) = (self.rd.scrn, self.rd.bx, self.rd.by);
        let tempobjid = self.rdblock(sc, bx, by);
        if !self.checkcoll() {
            return;
        }
        let d = self.dbarr2(tempobjid);
        if d & 0x80 == 0 {
            return;
        }
        self.ch.x = self.addcharx(d.wrapping_neg());
        self.jumpseq(s::BUMPENGBACK);
        self.animchar();
        self.rereadblocks();
    }

    /// DBarr2: distance to the barrier behind (reversed from DBarr).
    fn dbarr2(&self, id: u8) -> u8 {
        let c = cmpbarr(id) as usize;
        if c == 0 {
            return 0xff;
        }
        if self.ch.face & 0x80 == 0 {
            let left = self.blockedge.wrapping_add(crate::subs::BAR_L[c]);
            left.wrapping_sub(self.edges.cd_right)
        } else {
            let right = self.blockedge.wrapping_add(13).wrapping_sub(crate::subs::BAR_R[c]);
            self.edges.cd_left.wrapping_sub(right)
        }
    }

    // ------------------------------------------------------------------
    // Guards on and off screen

    /// INITIALGUARDS: starting x-coords for the whole level.
    pub fn initialguards(&mut self) {
        for scrn in 1..=24u8 {
            let b = self.level.gd(gd::BLOCK, scrn);
            if b >= 30 {
                continue;
            }
            let x = getblockej((b % 10) as i8).wrapping_add(ANGLE + 7);
            self.level.set_gd(gd::X, scrn, x);
            self.level.set_gd(gd::SEQH, scrn, 0);
        }
    }

    /// CHECKALERT: can the kid and the opponent see each other?
    /// 2 = same stretch of floor, 1 = in sight across a gap or obstacle.
    pub fn checkalert(&mut self) {
        let (k, sh) = (self.kid, self.shad);
        if sh.id == 24 {
            return;
        }
        let ea = self.alert_level(&k, &sh);
        if ea != self.enemy_alert {
            log::debug!("enemy alert {} -> {ea}", self.enemy_alert);
        }
        self.enemy_alert = ea;
    }

    fn alert_level(&mut self, k: &Char, sh: &Char) -> u8 {
        if sh.id == 1 && self.level.number != 12 {
            return 0; // fight the shadow only on level 12
        }
        if k.posn == 0 || (219..229).contains(&k.posn) || sh.face == NO_CHAR {
            return 0;
        }
        if (k.life & sh.life) & 0x80 == 0 || k.scrn != sh.scrn || k.block_y != sh.block_y {
            return 0;
        }
        let mut ea = 2;
        let mut xcount = getblockej(k.block_x as i8).wrapping_add(7);
        let mut xend = getblockej(sh.block_x as i8).wrapping_add(7);
        if xend < xcount {
            std::mem::swap(&mut xcount, &mut xend);
        }
        let by = k.block_y as i8 as i32;
        let rd = |g: &mut Game, x: u8| g.rdblock(k.scrn, getblockxp(x).0 as i32, by);
        if rd(self, xcount) == obj::SLICER {
            xcount = xcount.wrapping_add(14);
        }
        if rd(self, xend) == obj::GATE {
            xend = xend.wrapping_sub(14);
        }
        while xcount <= xend {
            let id = rd(self, xcount);
            let view = match id {
                obj::BLOCK | obj::PANELWIF | obj::PANELWOF => return 0,
                obj::LOOSE | obj::SLICER => true,
                obj::GATE => self.spec() < 28 * 4,
                _ => is_space(id),
            };
            if view {
                ea = 1;
            }
            xcount = xcount.wrapping_add(14);
            if xcount == 0 {
                break;
            }
        }
        ea
    }

    /// cutguards: the kid has cut to a new screen; take an en garde guard
    /// along if he is close behind, otherwise leave him where he is.
    pub fn cutguards(&mut self, dir: u8) {
        let sh = self.shad;
        let transfer = if sh.life & 0x80 == 0 || sh.sword != 2 {
            false
        } else {
            let ks = self.kid.scrn;
            let guard_there = self.level.gd(gd::BLOCK, ks) < 30 && self.level.gd(gd::SEQH, ks) == 0;
            if guard_there {
                false
            } else {
                match dir {
                    0 => sh.x < (256 - 140 - 25) as u8,
                    1 => sh.x >= 140 + 25,
                    2 => sh.block_y & 0x80 != 0,
                    _ => sh.block_y >= 3,
                }
            }
        };
        if transfer {
            log::info!("guard follows the kid to screen {}", self.kid.scrn);
            self.level.set_gd(gd::BLOCK, self.kid.scrn, 0xff);
            self.level.set_gd(gd::BLOCK, sh.scrn, 0xff);
            self.load_shad();
            self.cut(dir);
            self.save_shad();
        } else {
            self.updateguard();
        }
    }

    /// updateguard: leave the guard behind; remember where he is (and, if
    /// dead, his sequence) so ADDGUARD can put him back.
    fn updateguard(&mut self) {
        let sh = self.shad;
        if sh.face == NO_CHAR || sh.id == 1 || sh.id == 24 {
            return;
        }
        let s = sh.scrn;
        self.level.set_gd(gd::BLOCK, s, sh.block_y.wrapping_mul(10));
        self.level.set_gd(gd::X, s, sh.x);
        self.level.set_gd(gd::FACE, s, sh.face);
        self.level.set_gd(gd::PROG, s, self.guardprog);
        if sh.life & 0x80 != 0 {
            self.level.set_gd(gd::SEQH, s, 0);
        } else {
            let a = sh.seq.wrapping_add(seq::ORG);
            self.level.set_gd(gd::SEQL, s, a as u8);
            self.level.set_gd(gd::SEQH, s, (a >> 8) as u8);
        }
        self.shad.face = NO_CHAR;
        self.opp_strength = 0;
    }

    /// CUTGUARD: an enemy who has fallen off the bottom is gone.
    pub fn cutguard(&mut self) {
        if self.shad.face == NO_CHAR || self.shad.y < BOT_CUT_EDGE {
            return;
        }
        match self.shad.id {
            1 => {
                if self.shad.action == 4 {
                    self.load_shad();
                    self.vanishchar();
                    self.save_shad();
                }
            }
            4 => {
                self.shad.scrn = self.level.down(self.shad.scrn);
                if self.shad.scrn != 3 {
                    return self.removegd();
                }
                // the skeleton lands on screen 3
                self.add_sound(Sound::Splat);
                self.shad.x = 0x85;
                self.shad.block_y = 1;
                self.shad.face = 0;
                self.shad.life = 0xff;
                self.updateguard();
            }
            _ => self.removegd(),
        }
    }

    fn removegd(&mut self) {
        log::info!("guard fell off screen {}", self.vis_scrn);
        self.deadenemy();
        self.level.set_gd(gd::BLOCK, self.vis_scrn, 0xff);
        self.shad.face = NO_CHAR;
        self.opp_strength = 0;
        self.chg_opp_str = 0xff;
    }

    /// chgshadposn + csps: put the shadowman at a fixed position.
    fn csps(&mut self, (pos, seq): ([u8; 7], u8)) {
        self.ch.posn = pos[0];
        self.ch.x = pos[1];
        self.ch.y = pos[2];
        self.ch.face = pos[3];
        self.ch.block_x = pos[4];
        self.ch.block_y = pos[5];
        self.ch.action = pos[6];
        self.jumpseq(seq);
        self.ch.id = 1;
        self.playcount = 0;
        self.guardprog = 3;
        self.max_opp_str = SHADSTRENGTH;
        self.opp_strength = SHADSTRENGTH;
        self.save_shad();
    }

    /// ADDGUARD: on a cut to a new screen, bring its guard to life (or
    /// death), plus the hard-wired shadowman appearances. Char holds the
    /// kid on entry, as after PrepCut's LoadKid.
    pub fn addguard(&mut self) {
        self.offguard = false;
        let v = self.vis_scrn;
        match self.level.number {
            12 => {
                if self.exitopen != 0 {
                    return; // set when the shadow drops
                }
                if self.mergetimer != 0 || v != SWORDSCRN {
                    return;
                }
                self.ch.scrn = v;
                if self.rdblock(v, SWORDX, 0) == obj::SWORD {
                    return; // sword is still there
                }
                self.shadowaction = 0;
                self.exitopen = 1;
                log::info!("the shadow drops in");
                self.csps(SHADPOS12);
            }
            6 => {
                self.ch.scrn = v;
                if v != 1 {
                    return self.addnormalgd();
                }
                if self.exitopen != 77 {
                    self.cuesong(crate::music::Song::Danger);
                    self.exitopen = 77;
                }
                self.csps(SHADPOS6A);
            }
            5 => {
                self.ch.scrn = v;
                if v != FLASKSCRN {
                    return self.addnormalgd();
                }
                if self.rdblock(v, FLASKX, 0) != obj::FLASK {
                    return; // potion is gone
                }
                self.csps(SHADPOS5);
            }
            _ => self.addnormalgd(),
        }
    }

    fn addnormalgd(&mut self) {
        let v = self.vis_scrn;
        let b = self.level.gd(gd::BLOCK, v);
        if b >= 30 {
            return;
        }
        self.ch.scrn = v;
        self.ch.block_y = b / 10;
        self.ch.y = FLOOR_Y[(b / 10 + 1) as usize];
        self.ch.x = self.level.gd(gd::X, v);
        self.ch.block_x = getblockxp(self.ch.x).0 as u8;
        self.ch.face = self.level.gd(gd::FACE, v);
        self.ch.id = if self.level.number == 3 { 4 } else { 2 };
        let seqh = self.level.gd(gd::SEQH, v);
        if seqh == 0 {
            // fresh start
            if self.ch.id == 4 {
                self.ch.sword = 2;
                self.jumpseq(s::LANDENGARDE);
            } else {
                self.ch.sword = 0;
                self.jumpseq(s::ALERTSTAND);
            }
        } else {
            self.ch.seq = seq::addr(self.level.gd(gd::SEQL, v), seqh);
        }
        self.animchar();
        if matches!(self.ch.posn, 185 | 177 | 178) {
            // dead guard
            self.ch.life = 1;
            self.opp_strength = 0;
        } else {
            self.ch.life = 0xff;
            self.alertguard = false;
            self.refract = 0;
            self.justblocked = 0;
            self.getgdstrength();
        }
        self.ch.xvel = 0;
        self.ch.yvel = 0;
        self.ch.action = 1;
        let mut prog = self.level.gd(gd::PROG, v);
        if prog >= NUMPROGS {
            prog = 3;
        }
        self.guardprog = prog;
        let lvl = self.level.number as usize;
        self.guard_color = BASICCOLOR[lvl] ^ SPECIALCOLOR[prog as usize];
        log::info!(
            "guard on screen {v}: prog {prog} strength {} colour {} alive {}",
            self.opp_strength,
            self.guard_color,
            self.ch.life & 0x80 != 0
        );
        // meeting Jaffar on level 13
        if self.level.number == 13 && self.ch.life & 0x80 != 0 && !self.met_jaffar {
            self.met_jaffar = true;
            self.cuesong(crate::music::Song::MeetJaffar);
            // as SDLPoP's meet_Jaffar: 28 frames, 2.33 seconds
            self.jaffar_wait = 28;
        }
        self.save_shad();
    }

    /// BONESRISE: level 3, the skeleton comes to life once the exit is open.
    pub fn bonesrise(&mut self) {
        if self.level.number != 3 || self.shad.face != NO_CHAR || self.vis_scrn != SKELSCRN || self.exitopen == 0 {
            return;
        }
        let kbx = self.kid.block_x;
        if kbx != SKELTRIG && kbx != SKELTRIG + 1 {
            return;
        }
        let v = self.vis_scrn;
        let id = self.rdblock(v, SKELX, SKELY as i32);
        self.set_type(obj::FLOOR);
        if id != obj::BONES {
            return;
        }
        log::info!("the skeleton rises");
        self.ch.scrn = v;
        self.ch.block_y = SKELY;
        self.ch.y = FLOOR_Y[SKELY as usize + 1];
        self.ch.block_x = SKELX as u8;
        self.ch.x = getblockej(SKELX as i8).wrapping_add(ANGLE + 7);
        self.ch.face = FACE_LEFT;
        self.jumpseq(s::ARISE);
        self.animchar();
        self.guardprog = SKELPROG;
        self.ch.life = 0xff;
        self.opp_strength = 3;
        self.alertguard = false;
        self.refract = 0;
        self.ch.xvel = 0;
        self.ch.yvel = 0;
        self.ch.sword = 2;
        self.ch.id = 4;
        self.save_shad();
    }

    /// MOUSERESCUE: level 8, the mouse comes to open the gate.
    pub fn mouserescue(&mut self) {
        log::info!("the mouse comes");
        self.load_kid();
        self.ch.id = 24;
        self.ch.x = 200;
        self.ch.block_y = 0;
        self.ch.y = FLOOR_Y[1];
        self.ch.face = FACE_LEFT;
        self.ch.life = 0xff;
        self.opp_strength = 1;
        self.jumpseq(s::MSCURRY);
        self.animchar();
        self.save_shad();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Game, c: Controls, n: u32) {
        for _ in 0..n {
            g.tick(c);
        }
    }

    const NONE: Controls = Controls { x: 0, y: 0, btn: false };
    const LEFT: Controls = Controls { x: -1, y: 0, btn: false };
    const BTN: Controls = Controls { x: 0, y: 0, btn: true };

    /// Level 2 starts on screen 5; the first guard waits on screen 4.
    fn meet_level2_guard() -> Game {
        let mut g = Game::new(2);
        run(&mut g, NONE, 12);
        run(&mut g, LEFT, 36);
        run(&mut g, NONE, 10);
        g
    }

    /// Level 0, the attract demo: the kid runs, jumps, climbs and fights
    /// the first guard by himself, wins, and the demo ends.
    #[test]
    fn the_demo_plays_itself_to_the_end() {
        let mut g = Game::new(0);
        let mut t = 0;
        while !g.demo_over && t < 2000 {
            g.tick(NONE);
            t += 1;
        }
        assert!(g.demo_over, "demo still running after {t} frames");
        assert!(g.kid.life & 0x80 != 0, "the kid lost");
        assert_eq!(g.shad.life & 0x80, 0, "the guard is still alive");
        assert!(g.song_start.is_none());
    }

    /// Level 13: the vizier hears his music before he goes en garde.
    #[test]
    fn vizier_waits_for_his_music() {
        let mut g = Game::new(13);
        let scrn = (1..=24u8).find(|&s| g.level.gd(gd::BLOCK, s) < 30).expect("a guard on level 13");
        let b = g.level.gd(gd::BLOCK, scrn);
        let (gx, gy) = (b % 10, b / 10);
        let (kx, face) = if gx >= 5 { (gx - 4, FACE_RIGHT) } else { (gx + 4, FACE_LEFT) };
        g.place_kid(scrn, kx, gy, face);
        // entering the screen places him and cues the music
        let mut met = g.met_jaffar.then_some(0);
        for t in 1..120u32 {
            g.tick(NONE);
            if met.is_none() && g.met_jaffar {
                met = Some(t);
            }
            if g.shad.sword >= 2 {
                let m = met.expect("music cued before en garde");
                assert!(t - m >= 28, "en garde {} frames after the music", t - m);
                return;
            }
        }
        panic!("the vizier never went en garde (met {met:?})");
    }

    #[test]
    fn opdist_counts_forward_and_allows_for_width() {
        let mut g = Game::new(2);
        g.ch = Char { scrn: 4, x: 100, face: FACE_LEFT, ..Char::default() };
        g.op = Char { scrn: 4, x: 80, face: FACE_RIGHT, ..Char::default() };
        // 20 pixels ahead, facing each other: + estwidth
        assert_eq!(g.getopdist(), 20 + ESTWIDTH);
        // same facing: no allowance
        g.op.face = FACE_LEFT;
        assert_eq!(g.getopdist(), 20);
        // behind: negative
        g.ch.face = FACE_RIGHT;
        assert_eq!(g.getopdist() as i8, -20);
        // another screen: far away
        g.op.scrn = 5;
        assert_eq!(g.getopdist(), 127);
    }

    #[test]
    fn guard_is_placed_from_the_level_data() {
        let g = meet_level2_guard();
        assert_eq!(g.kid.scrn, 4);
        let o = g.shad;
        assert_ne!(o.face, NO_CHAR);
        assert_eq!((o.id, o.scrn, o.block_y), (2, 4, 1));
        // level 2: basic strength 3, guard program 1, blue uniform
        assert_eq!((g.max_opp_str, g.guardprog, g.guard_color), (3, 1, 0));
        // he has seen the kid and drawn
        assert_eq!(o.sword, 2);
        assert_eq!(g.kid.sword, 2, "kid goes en garde when the guard is near");
    }

    #[test]
    fn advancing_and_striking_kills_the_guard() {
        let mut g = meet_level2_guard();
        let mut killed = false;
        for _ in 0..25 {
            run(&mut g, LEFT, 1);
            run(&mut g, NONE, 3);
            run(&mut g, BTN, 1);
            run(&mut g, NONE, 5);
            if g.shad.life & 0x80 == 0 {
                killed = true;
                break;
            }
        }
        assert!(killed, "guard still alive with strength {}", g.opp_strength);
        assert_eq!(g.opp_strength, 0);
        assert!(g.kid.life & 0x80 != 0, "kid survived");
        run(&mut g, NONE, 10);
        assert_eq!(g.shad.posn, 185, "guard lies dead");
    }

    #[test]
    fn a_kid_who_does_nothing_is_cut_down() {
        let mut g = meet_level2_guard();
        run(&mut g, NONE, 200);
        assert_eq!(g.kid.life & 0x80, 0);
        assert_eq!(g.kid_strength, 0);
        assert!(matches!(g.phase, Phase::Dead(_)));
        // with the kid dead the guard relaxes
        assert_eq!(g.shad.sword, 0);
    }

    #[test]
    fn a_dead_guard_stays_dead_when_you_come_back() {
        let mut g = meet_level2_guard();
        g.load_shad();
        g.ch.life = 0;
        g.jumpseq(s::STABKILL);
        for _ in 0..8 {
            g.animchar();
        }
        g.save_shad();
        assert_eq!(g.shad.posn, 185);
        g.updateguard();
        assert_eq!(g.shad.face, NO_CHAR);
        g.vis_scrn = 4;
        g.load_kid();
        g.addguard();
        assert_eq!(g.shad.posn, 185);
        assert_eq!(g.shad.life, 1);
        assert_eq!(g.opp_strength, 0);
    }

    #[test]
    fn strength_changes_wait_for_chgmeters() {
        let mut g = Game::new(2);
        g.load_kid();
        assert!(g.decstr(1));
        assert_eq!(g.kid_strength, 3, "not applied yet");
        g.chgmeters();
        assert_eq!(g.kid_strength, 2);
        g.chg_kid_str = 0;
        assert!(!g.decstr(100));
        g.chgmeters();
        assert_eq!(g.kid_strength, 0);
        // the opponent's meter when Char is a guard
        g.ch.id = 2;
        g.max_opp_str = 4;
        g.opp_strength = 4;
        g.chg_opp_str = 0;
        assert!(g.decstr(1));
        g.chgmeters();
        assert_eq!(g.opp_strength, 3);
    }

    #[test]
    fn level_3_skeleton_never_loses_strength() {
        let mut g = Game::new(3);
        g.ch = Char { id: 4, sword: 2, life: 0xff, scrn: 1, block_y: 1, face: FACE_LEFT, ..Char::default() };
        g.opp_strength = 3;
        g.stabchar();
        assert_eq!(g.chg_opp_str, 0);
        assert!(g.ch.life & 0x80 != 0);
    }
}

#[cfg(test)]
mod mirror_tests {
    use super::*;
    use crate::level::Level;

    /// Level 4 with the exit open (so the mirror is in place), the kid
    /// running left along the top of screen 4 towards it.
    fn mirror_run(run: u32) -> Game {
        let mut g = Game::new(4);
        let i = Level::index(4, 4, 0);
        g.level.set_type(i, obj::MIRROR);
        g.exitopen = 1;
        g.place_kid(4, 9, 0, FACE_LEFT);
        let left = Controls { x: -1, y: 0, btn: false };
        let jump = Controls { x: -1, y: -1, btn: false };
        for _ in 0..run {
            g.tick(left);
        }
        for _ in 0..30 {
            g.tick(jump);
        }
        g
    }

    #[test]
    fn running_jump_through_the_mirror_frees_the_shadow() {
        let freed = (4..14).map(mirror_run).find(|g| g.shad.id == 1 && g.shad.face != NO_CHAR || g.level.spec(&g.level.rdblock(4, 4, 0)) == 86);
        let g = freed.expect("no running jump reached the mirror");
        // the mirror is smashed, the shadow takes the kid's strength
        assert_eq!(g.level.spec(&g.level.rdblock(4, 4, 0)), 86);
        assert_eq!(g.kid_strength, 1);
        assert_eq!(g.max_opp_str, g.max_strength);
    }

    #[test]
    fn standing_before_the_mirror_shows_a_reflection() {
        let mut g = Game::new(4);
        let i = Level::index(4, 4, 0);
        g.level.set_type(i, obj::MIRROR);
        g.place_kid(4, 4, 0, FACE_LEFT);
        g.tick(Controls::default());
        let (r, left) = g.reflect.expect("no reflection");
        assert_eq!(r.face, FACE_RIGHT);
        assert_eq!(left, (4 * 4 + 1) * 7);
    }
}

#[cfg(test)]
mod level12_tests {
    use super::*;
    use crate::level::Level;

    #[test]
    fn the_shadow_drops_in_on_level_12_once_the_sword_is_gone() {
        let mut g = Game::new(12);
        // the sword still there: no shadow
        g.place_kid(15, 0, 0, FACE_RIGHT);
        assert_eq!(g.shad.face, NO_CHAR);
        let i = Level::index(SWORDSCRN, 1, 0);
        g.level.set_type(i, obj::FLOOR);
        // the kid comes in at the right; the shadow waits above
        g.place_kid(18, 8, 0, FACE_LEFT);
        g.place_kid(15, 7, 0, FACE_LEFT);
        assert_eq!(g.shad.id, 1);
        assert_eq!(g.exitopen, 1);
        assert_eq!(g.max_opp_str, SHADSTRENGTH);
        for _ in 0..10 {
            g.tick(Controls::default());
        }
        assert_eq!(g.shadowaction, 0, "held until the kid moves left");
        let left = Controls { x: -1, y: 0, btn: false };
        for _ in 0..3 {
            g.tick(left);
        }
        for _ in 0..12 {
            g.tick(Controls::default());
        }
        // he has dropped from above the screen onto the kid's floor
        assert_eq!(g.shadowaction, 1);
        assert_eq!((g.shad.scrn, g.shad.block_y, g.shad.y), (15, 0, FLOOR_Y[1]));
        assert!(g.shad.action != 4, "landed");
    }

    #[test]
    fn on_level_12_wounding_the_shadow_wounds_the_kid() {
        let mut g = Game::new(12);
        g.ch = Char { id: 1, ..Char::default() };
        g.op = Char { id: 0, ..Char::default() };
        g.max_opp_str = 4;
        g.opp_strength = 4;
        g.chg_opp_str = 0xff;
        g.chgmeters();
        assert_eq!((g.kid_strength, g.opp_strength), (2, 3));
    }
}
