//! Player control and floor checks: a port of CTRL.S.
//!
//! Each routine keeps its original name and shape. The joystick model is
//! the original's: `jstk_x` < 0 means forward (input is turned to face the
//! character before GENCTRL runs), and each direction has a `clr` flag that
//! latches a fresh press until something uses it.

use crate::game::*;
use crate::level::{is_space, obj};
use crate::seq::{seq as s, FCHECKMARK};

pub(crate) const DEATH_VELOCITY: u8 = 33;
pub(crate) const OOF_VELOCITY: u8 = 22;
const GRABREACH: u8 = (-8i8) as u8;
/// no grabbing a ledge falling this fast: 30, not the original's 32, so a
/// bump into a wall can't catch a ledge two storeys down (SDLPoP
/// fix_grab_falling_speed)
const GRABSPEED: u8 = 30;
const GRABLEAD: u8 = 25;
const STUNTIME: u8 = 12;
const JUMPUPREACH: u8 = 0;
const JUMPUPANGLE: u8 = (-6i8) as u8;
const JUMP_BACK_THRES: u8 = 6;
const STEP_OFF_FWD: u8 = 3;
const STEP_OFF_BACK: u8 = 8;
const GCLIMBTHRES: u8 = 6;
const STAIRTHRES: u8 = 30;
const SWORDTHRES: u8 = 90;
const SWORDTHRES_N: u8 = (-10i8) as u8;
const BLOCKTHRES: u8 = 32;
const GRACEPERIOD: u8 = 9;
const GDPATIENCE: u8 = 15;

const RJ_CHANGE: u8 = 4;
const RJ_LOOKAHEAD: u8 = 1;
const RJ_LEAD_DIST: u8 = 14;
const RJ_MAX_FUJ_BAK: u8 = 8;
const RJ_MAX_FUJ_FWD: u8 = 2;

fn fresh(c: i8) -> bool {
    c < 0
}

impl Game {
    /// ]clr: mark every press as used.
    fn clrall(&mut self) {
        self.joy.clr_b = 0;
        self.joy.clr_f = 0;
        self.joy.clr_u = 0;
        self.joy.clr_d = 0;
    }

    /// PLAYERCTRL + CLRJSTK + UserCtrl (facejstk).
    pub fn playerctrl(&mut self, c: Controls) {
        if self.ch.life & 0x80 != 0 && self.kid_strength == 0 {
            self.ch.life = 0;
        }
        if self.stunned != 0 {
            self.stunned -= 1;
        }
        if self.level.number == 0 {
            self.democtrl();
            return self.genctrl();
        }
        // getselect: raw joystick in screen terms, flags stored as if facing L
        self.joy.jstk_x = c.x;
        self.joy.jstk_y = c.y;
        self.joy.btn = c.btn;
        self.clrjstk();
        let facing_right = !self.face_left();
        if facing_right {
            self.facejstk();
        }
        self.genctrl();
        if facing_right {
            self.facejstk();
        }
    }

    fn facejstk(&mut self) {
        self.joy.jstk_x = -self.joy.jstk_x;
        std::mem::swap(&mut self.joy.clr_f, &mut self.joy.clr_b);
    }

    /// CLRJSTK: 0 -> -1 on a press, 1 -> 0 on release, -1 stays.
    fn clrjstk(&mut self) {
        fn upd(clr: &mut i8, down: bool) {
            if *clr < 0 {
                return;
            }
            if down {
                if *clr == 0 {
                    *clr = -1;
                }
            } else {
                *clr = 0;
            }
        }
        let j = self.joy;
        upd(&mut self.joy.clr_f, j.jstk_x < 0);
        upd(&mut self.joy.clr_b, j.jstk_x == 1);
        upd(&mut self.joy.clr_u, j.jstk_y < 0);
        upd(&mut self.joy.clr_d, j.jstk_y == 1);
        upd(&mut self.joy.clr_btn, j.btn);
    }

    /// GENCTRL
    pub fn genctrl(&mut self) {
        if self.ch.life & 0x80 == 0 {
            if matches!(self.ch.posn, 15 | 166 | 158 | 171) {
                self.jumpseq(s::DROPDEAD);
            }
            return;
        }
        if matches!(self.ch.action, 4 | 5) {
            self.clrall();
            return;
        }
        if self.ch.sword == 2 {
            return self.fightctrl();
        }
        if self.ch.id >= 2 {
            return self.guardctrl();
        }
        let x = self.ch.posn;
        if x == 15 {
            return self.standing();
        }
        if x == 48 {
            return self.turning();
        }
        if (50..53).contains(&x) {
            return self.standing();
        }
        if x < 4 {
            return self.starting();
        }
        if (67..70).contains(&x) {
            return self.stjumpup();
        }
        if x < 15 {
            return self.arunning();
        }
        if (87..100).contains(&x) {
            return self.hanging();
        }
        if x == 109 {
            self.crouching();
        }
        // drinking: a press made meanwhile is used up, so it doesn't move
        // him once he's done (SDLPoP fix_move_after_drink)
        if (191..=205).contains(&x) {
            self.clrall();
        }
    }

    fn standing(&mut self) {
        if fresh(self.joy.clr_btn) && self.joy.btn && self.trypickup() {
            return;
        }
        if self.ch.id != 0 {
            // shadowman only: down and forward to go en garde
            if fresh(self.joy.clr_d) && fresh(self.joy.clr_f) {
                return self.do_engarde();
            }
        } else if self.gotsword && (!self.offguard || self.joy.btn) {
            // if the opponent is within range, go en garde (kid only; off
            // guard, press the button to draw)
            if self.enemy_alert >= 2 {
                let d = self.getopdist();
                if !(SWORDTHRES..SWORDTHRES_N).contains(&d) {
                    self.heroic = true;
                    if d >= (-6i8) as u8 {
                        return self.doturn();
                    }
                    let shadow_landing = self.op.id == 1
                        && (self.op.action == 3 || (107..118).contains(&self.op.posn));
                    if !shadow_landing {
                        return self.do_engarde();
                    }
                }
            }
            self.offguard = false;
        }
        if self.joy.btn {
            if fresh(self.joy.clr_b) {
                return self.doturn();
            }
            if fresh(self.joy.clr_u) {
                return self.standing_up();
            }
            if fresh(self.joy.clr_d) {
                return self.standing_down();
            }
            if self.joy.jstk_x >= 0 {
                return;
            }
            if fresh(self.joy.clr_f) {
                self.dostepfwd();
            }
            return;
        }
        if fresh(self.joy.clr_f) {
            return self.dostartrun();
        }
        if fresh(self.joy.clr_b) {
            return self.doturn();
        }
        if fresh(self.joy.clr_u) {
            return self.standing_up();
        }
        if fresh(self.joy.clr_d) {
            return self.standing_down();
        }
        if self.joy.jstk_x < 0 {
            self.dostartrun();
        }
    }

    fn standing_up(&mut self) {
        // In front of open stairs?
        let mut stairs = self.getunderft() == obj::EXIT;
        if !stairs {
            stairs = self.getbehind() == obj::EXIT;
        }
        if !stairs {
            stairs = self.getinfront() == obj::EXIT;
        }
        if stairs && (self.spec() >> 2) >= STAIRTHRES {
            return self.stairs();
        }
        if self.joy.jstk_x < 0 {
            return self.dostandjump();
        }
        self.dojumpup();
    }

    fn standing_down(&mut self) {
        self.joy.clr_d = 1;
        // facing an edge: step off it
        let id = self.getinfront();
        if is_space(id) && self.getdist() < STEP_OFF_FWD {
            self.ch.x = self.addcharx(5);
            self.rereadblocks();
            return;
        }
        // back to an edge: climb down and hang
        let id = self.getbehind();
        if is_space(id) && self.getdist() >= STEP_OFF_BACK {
            self.blockid = self.getbehind();
            let under = self.getunderft();
            if self.checkledge(under) {
                let ok = if self.face_left() && self.getunderft() == obj::GATE {
                    (self.spec() >> 2) >= GCLIMBTHRES
                } else {
                    true
                };
                if ok {
                    let d = self.getdist().wrapping_sub(9);
                    self.ch.x = self.addcharx(d);
                    self.jumpseq(s::CLIMBDOWN);
                    return;
                }
            }
        }
        self.docrouch();
    }

    fn stairs(&mut self) {
        let bx = self.rd.bx as i8;
        self.ch.x = getblockej(bx).wrapping_add(10);
        self.ch.face = FACE_LEFT;
        self.jumpseq(s::CLIMBSTAIRS);
        log::info!("climbing stairs");
    }

    fn crouching(&mut self) {
        if fresh(self.joy.clr_btn) && self.trypickup() {
            return;
        }
        if self.joy.jstk_y != 1 {
            self.jumpseq(s::STANDUP);
            return;
        }
        if fresh(self.joy.clr_f) {
            self.joy.clr_f = 1;
            self.jumpseq(s::CRAWL);
        }
    }

    fn starting(&mut self) {
        if self.joy.jstk_y < 0 && self.joy.jstk_x < 0 {
            self.dostandjump();
        }
    }

    fn stjumpup(&mut self) {
        if self.joy.jstk_x < 0 || fresh(self.joy.clr_f) {
            self.dostandjump();
        }
    }

    fn turning(&mut self) {
        if self.joy.btn || self.joy.jstk_x >= 0 || self.joy.jstk_y < 0 {
            return;
        }
        self.jumpseq(s::TURNRUN);
    }

    fn arunning(&mut self) {
        if self.joy.jstk_x == 0 {
            // stop running
            if matches!(self.ch.posn, 7 | 11) {
                self.clrall();
                self.joy.clr_f = 1;
                self.jumpseq(s::RUNSTOP);
            }
            return;
        }
        if self.joy.jstk_x > 0 {
            self.clrall();
            self.joy.clr_b = 1;
            self.jumpseq(s::RUNTURN);
            return;
        }
        if self.joy.jstk_y < 0 {
            if fresh(self.joy.clr_u) {
                self.dorunjump();
            }
            return;
        }
        if fresh(self.joy.clr_d) {
            self.joy.clr_d = 1;
            self.jumpseq(s::RDIVEROLL);
        }
    }

    fn hanging(&mut self) {
        if self.stunned == 0 && self.joy.jstk_y < 0 {
            return self.climbup();
        }
        if !self.joy.btn {
            return self.hangdrop();
        }
        if self.ch.action != 6 {
            let id = self.getunderft();
            if id == obj::BLOCK || (self.face_left() && matches!(id, obj::PANELWIF | obj::PANELWOF)) {
                self.jumpseq(s::HANGSTRAIGHT);
                return;
            }
        }
        // if the ledge crumbles away, fall with it
        let id = self.getabove();
        if is_space(id) {
            self.hangdrop();
        }
    }

    fn climbup(&mut self) {
        self.clrall();
        self.joy.clr_btn = 1;
        let id = self.getabove();
        let ok = match id {
            obj::MIRROR | obj::SLICER => self.face_left(),
            obj::GATE => !self.face_left() || (self.spec() >> 2) >= GCLIMBTHRES,
            _ => true,
        };
        self.jumpseq(if ok { s::CLIMBUP } else { s::CLIMBFAIL });
    }

    fn hangdrop(&mut self) {
        self.clrall();
        self.joy.clr_d = 1;
        let behind = self.getbehind();
        if is_space(behind) && is_space(self.getunderft()) {
            self.jumpseq(s::HANGFALL);
            return;
        }
        let id = self.getunderft();
        let sheer = id == obj::BLOCK || (self.face_left() && matches!(id, obj::PANELWOF | obj::PANELWIF));
        if sheer {
            self.ch.x = self.addcharx((-7i8) as u8);
        }
        self.jumpseq(s::HANGDROP);
    }

    fn dostartrun(&mut self) {
        let (d, what) = self.getfwddist();
        if what == 1 && self.rd.id != obj::SLICER && d < 8 {
            if fresh(self.joy.clr_f) {
                self.dostepfwd();
            }
            return;
        }
        self.jumpseq(s::STARTRUN);
    }

    fn doturn(&mut self) {
        self.clrall();
        self.joy.clr_b = 1;
        // if the enemy is behind you, draw as you turn
        if self.gotsword && self.enemy_alert >= 2 && self.getopdist() & 0x80 != 0 && self.getdist() >= 2 {
            self.ch.sword = 2;
            self.offguard = false;
            self.jumpseq(s::TURNDRAW);
            return;
        }
        self.jumpseq(s::TURN);
    }

    /// DoEngarde (CTRL.S)
    pub fn do_engarde(&mut self) {
        self.clrall();
        self.joy.clr_f = 1;
        self.joy.clr_btn = 1;
        self.ch.sword = 2;
        log::debug!("char {} en garde", self.ch.id);
        match self.ch.id {
            0 => {
                self.offguard = false;
                self.add_sound(Sound::DrawSword);
                self.jumpseq(s::ENGARDE);
            }
            1 => self.jumpseq(s::ENGARDE),
            _ => self.jumpseq(s::GUARDENGARDE),
        }
    }

    /// GuardCtrl: a guard standing alert.
    fn guardctrl(&mut self) {
        if self.ch.posn != 166 || !fresh(self.joy.clr_d) {
            return;
        }
        if fresh(self.joy.clr_f) {
            return self.do_engarde();
        }
        self.joy.clr_d = 1;
        self.jumpseq(s::ALERTTURN);
    }

    /// FightCtrl: Char is en garde (CharSword = 2).
    fn fightctrl(&mut self) {
        if self.ch.action >= 2 {
            return; // must be on level ground
        }
        // once the enemy alert is over, put away your sword (unless
        // standing on loose floor); if he is behind you, turn to face him
        let mut drop = false;
        if self.getunderft() != obj::LOOSE && self.enemy_alert < 2 {
            drop = true;
        } else {
            let d = self.getopdist();
            if d < SWORDTHRES {
            } else if d < 128 {
                drop = true;
            } else if d < (-4i8) as u8 {
                self.jumpseq(s::TURNENGARDE);
                return;
            }
        }
        if drop {
            // enemy out of range: kid and shadowman drop their guard
            if self.ch.id == 0 {
                self.heroic = false;
            }
            if self.ch.id < 2 {
                if self.ch.posn == 171 {
                    self.ch.sword = 0;
                    self.jumpseq(s::RESHEATHE);
                }
                return;
            }
        }
        // remain en garde
        let x = self.ch.posn;
        let strike = if x == 161 {
            // successful block: restrike or retreat?
            if !fresh(self.joy.clr_btn) {
                self.jumpseq(s::RETREAT);
                return;
            }
            true
        } else {
            fresh(self.joy.clr_btn)
        };
        if strike {
            if self.ch.id == 0 {
                self.gdtimer = GDPATIENCE;
            }
            self.fight_strike(x);
            if self.joy.clr_btn == 1 {
                return; // struck
            }
        }
        if fresh(self.joy.clr_d) {
            // down to lower your sword
            if matches!(x, 158 | 170 | 171) {
                self.joy.clr_d = 1;
                self.ch.sword = 0;
                match self.ch.id {
                    0 => {
                        self.offguard = true;
                        self.refract = GRACEPERIOD;
                        self.heroic = false;
                        self.jumpseq(s::FASTSHEATHE);
                    }
                    1 => self.jumpseq(s::RESHEATHE),
                    _ => self.jumpseq(s::GOALERTSTAND),
                }
                log::debug!("char {} lowers sword", self.ch.id);
            }
            return;
        }
        // forward to advance, up to block, back to retreat
        if fresh(self.joy.clr_u) {
            self.fight_block();
        } else if fresh(self.joy.clr_f) {
            self.fight_advance();
        } else if fresh(self.joy.clr_b) {
            self.fight_retreat();
        }
    }

    /// DoBlock (CTRL.S)
    fn fight_block(&mut self) {
        let x = self.ch.posn;
        if x == 167 {
            // blocked strike: strike-to-block
            self.joy.clr_u = 1;
            self.jumpseq(s::STRIKEBLOCK);
            return;
        }
        if !matches!(x, 158 | 170 | 171 | 168 | 165) {
            return;
        }
        let block = |g: &mut Game| {
            g.joy.clr_u = 1;
            g.jumpseq(s::READYBLOCK);
        };
        if self.getopdist() < BLOCKTHRES {
            let op = self.op.posn;
            if self.ch.id != 0 {
                // the enemy sees the kid one frame ahead
                if op == 152 {
                    block(self);
                }
                return;
            }
            match op {
                168 => return, // one frame too early: wait
                151 | 152 | 162 => return block(self),
                153 => {
                    // one frame too late: skip a frame
                    block(self);
                    self.animchar();
                    return;
                }
                _ => {}
            }
        }
        // block missed
        if self.ch.id != 0 {
            self.fight_retreat(); // the enemy doesn't waste blocks
        } else {
            block(self);
        }
    }

    /// DoStrike (CTRL.S), from frame `x`.
    fn fight_strike(&mut self, x: u8) {
        let seq = match x {
            157 | 158 | 170 | 171 | 165 => {
                // the kid is fast, others slow
                if self.ch.id == 0 {
                    s::FASTSTRIKE
                } else {
                    s::STRIKE
                }
            }
            150 | 161 => s::BLOCKTOSTRIKE,
            _ => return,
        };
        self.joy.clr_btn = 1;
        self.jumpseq(seq);
    }

    /// DoRetreat (CTRL.S)
    fn fight_retreat(&mut self) {
        if matches!(self.ch.posn, 158 | 170 | 171) {
            self.joy.clr_b = 1;
            self.jumpseq(s::RETREAT);
        }
    }

    /// DoAdvance (CTRL.S)
    fn fight_advance(&mut self) {
        if matches!(self.ch.posn, 158 | 170 | 171) {
            self.joy.clr_f = 1;
            self.jumpseq(if self.ch.id == 0 { s::FASTADVANCE } else { s::ADVANCE });
        }
    }

    fn dostandjump(&mut self) {
        self.joy.clr_u = 1;
        self.joy.clr_f = 1;
        self.jumpseq(s::STANDJUMP);
    }

    fn docrouch(&mut self) {
        self.jumpseq(s::STOOP);
        self.clrall();
        self.joy.clr_d = 1;
    }

    fn dojumpup(&mut self) {
        self.clrall();
        self.joy.clr_u = 1;
        self.blockid = self.getabove();
        let a = self.getaboveinf();
        if self.checkledge(a) {
            return self.dojumphang();
        }
        self.blockid = self.getabovebeh();
        let a = self.getabove();
        if self.checkledge(a) {
            // jump back to grab the block directly overhead
            if self.getdist() < JUMP_BACK_THRES {
                return self.dojumphigh();
            }
            if is_space(self.getbehind()) {
                // back to the ledge: jumpbackhang
                self.getabove();
                let d = self.getdist().wrapping_sub(10);
                self.ch.x = self.addcharx(d);
                self.jumpseq(s::JUMPBACKHANG);
                return;
            }
            let d = self.getdist().wrapping_sub(14);
            self.ch.x = self.addcharx(d);
            self.rereadblocks();
            return self.dojumphang();
        }
        self.dojumphigh();
    }

    fn dojumphang(&mut self) {
        self.getaboveinf();
        let atemp = self.getdist();
        let long = if atemp < 4 {
            let (d, what) = self.getfwddist();
            d < 4 && what == 1
        } else {
            true
        };
        if long {
            self.ch.x = self.addcharx(atemp.wrapping_sub(4));
            self.jumpseq(s::JUMPHANG_LONG);
        } else {
            self.ch.x = self.addcharx(atemp);
            self.jumpseq(s::JUMPHANG_MED);
        }
    }

    /// DoRunjump: calibrate so the foot pushes off at the edge.
    fn dorunjump(&mut self) {
        if self.ch.posn < 7 {
            return;
        }
        let ztemp = self.addcharx(RJ_CHANGE);
        let mut bx = getblockxp(ztemp).0 as i32;
        let mut count = 0u8;
        let mut edge = false;
        loop {
            bx += if self.face_left() { -1 } else { 1 };
            let (sc, by) = (self.ch.scrn, self.ch.block_y as i8 as i32);
            let id = self.rdblock(sc, bx, by);
            if id == obj::SPIKES || is_space(id) {
                edge = true;
                break;
            }
            count += 1;
            if count >= RJ_LOOKAHEAD + 1 {
                break;
            }
        }
        if edge {
            let a = self
                .getdist1(ztemp)
                .wrapping_add(7 * count)
                .wrapping_add(7 * count)
                .wrapping_sub(RJ_LEAD_DIST);
            let fudge = if a >= RJ_MAX_FUJ_BAK.wrapping_neg() || a < RJ_MAX_FUJ_FWD {
                a
            } else if a < 0x80 {
                return; // still too far away: wait till next frame
            } else {
                (-3i8) as u8 // jumped too late; make it look good anyway
            };
            self.ch.x = self.addcharx(fudge.wrapping_add(RJ_CHANGE));
        }
        self.clrall();
        self.joy.clr_u = 1;
        self.jumpseq(s::RUNJUMP);
    }

    fn dostepfwd(&mut self) {
        self.joy.clr_f = 1;
        self.joy.clr_btn = 1;
        let (d, what) = self.getfwddist();
        let step = if d != 0 {
            d
        } else if what == 1 || self.ch.repeat == 0 {
            // barrier: step through; second try at an edge: step off
            11
        } else {
            // first time at an edge: test with the foot
            self.ch.repeat = 0;
            self.jumpseq(s::TESTFOOT);
            return;
        };
        self.ch.repeat = step;
        // stepfwd1..13 are sequences 29..41; fullstep (42) is step 14
        self.jumpseq(s::STEPFWD1 - 1 + step);
    }

    fn dojumphigh(&mut self) {
        self.clrall();
        self.joy.clr_u = 1;
        let (d, what) = self.getfwddist();
        if d < 4 && what == 1 {
            self.ch.x = self.addcharx(d.wrapping_sub(3));
        }
        // three pixels from an edge he jumped up and fell (after climbing
        // two floors and turning round); a step back keeps him on it, as
        // SDLPoP's fix_jump_distance_at_edge does
        if d == 3 && what == 0 {
            self.ch.x = self.addcharx((-1i8) as u8);
        }
        let ztemp = self.facedx(JUMPUPREACH);
        let a = self.getbasex().wrapping_add(JUMPUPANGLE).wrapping_add(ztemp);
        let bx = getblockx(a).0 as i32;
        let (sc, by) = (self.ch.scrn, self.ch.block_y as i8 as i32 - 1);
        let id = self.rdblock(sc, bx, by);
        if id == obj::BLOCK || !is_space(id) {
            self.jumpseq(s::JUMPUP);
        } else {
            self.jumpseq(s::HIGHJUMP);
        }
    }

    // ------------------------------------------------------------------
    // CHECKFLOOR and friends

    pub fn checkfloor(&mut self) {
        match self.ch.action {
            6 | 2 => {}
            5 => {
                // standing up from a crouch (110-119) checks the floor too,
                // as SDLPoP's fix_stand_on_thin_air does
                if matches!(self.ch.posn, 109 | 185 | 110..=119) {
                    self.onground();
                }
            }
            4 => self.falling(),
            3 => {
                if (102..106).contains(&self.ch.posn) {
                    self.fallon();
                }
            }
            _ => self.onground(),
        }
    }

    fn floor_y(&self, by_plus1: i32) -> u8 {
        FLOOR_Y[by_plus1.clamp(0, 4) as usize]
    }

    /// If he's passed through the floor plane, change CharBlockY; if the
    /// floor is solid, stop him.
    fn falling(&mut self) {
        let by = self.ch.block_y as i8 as i32;
        if self.ch.y < self.floor_y(by + 1) {
            return self.fallon();
        }
        let mut id = self.getunderft();
        if id == obj::BLOCK {
            id = self.insideblock();
        }
        if is_space(id) {
            self.ch.block_y = self.ch.block_y.wrapping_add(1);
            return;
        }
        self.hitflr();
    }

    fn hitflr(&mut self) {
        let by = self.ch.block_y as i8 as i32;
        self.ch.y = self.floor_y(by + 1);
        // landing on spikes that haven't sprung yet (state 0) was safe in
        // the original; they impale, as with SDLPoP's
        // fix_safe_landing_on_spikes (jammed spikes stay safe)
        if self.getunderft() == obj::SPIKES {
            if self.getspikes() != 0 || self.spec() == 0 {
                return self.doimpale();
            }
        } else {
            // landed too close to an edge?
            let id = self.getinfront();
            if is_space(id) && self.getdist() < 4 {
                self.ch.x = self.addcharx((-3i8) as u8);
            }
            self.addslicers();
            if self.ch.life & 0x80 != 0 {
                let mut spikes = false;
                if self.getdist() >= 12 && self.getbehind() == obj::SPIKES {
                    spikes = true;
                }
                if !spikes && self.getunderft() == obj::SPIKES {
                    spikes = true;
                }
                if spikes && (self.getspikes() != 0 || self.spec() == 0) {
                    return self.doimpale();
                }
            }
        }
        self.screaming = false;
        let soft = |g: &mut Game| {
            if g.ch.id == 0 {
                g.add_sound(Sound::SoftLand);
            }
            // guards always land en garde
            if g.ch.id >= 2 || g.ch.sword == 2 {
                g.ch.sword = 2;
                s::LANDENGARDE
            } else {
                s::SOFTLAND
            }
        };
        let hard = |g: &mut Game, dec: bool| {
            if dec {
                g.decstr(100);
            }
            g.add_sound(Sound::FellToDeath);
            s::HARDLAND
        };
        let seq = if self.ch.life & 0x80 == 0 {
            // dead before he hits the ground
            hard(self, true)
        } else if self.ch.yvel < OOF_VELOCITY {
            soft(self)
        } else if self.ch.yvel < DEATH_VELOCITY {
            match self.ch.id {
                1 => soft(self), // the shadowman lands easy
                id if id >= 2 => hard(self, true), // guards can't survive two storeys
                _ => {
                    if self.decstr(1) {
                        self.add_sound(Sound::MediumLand);
                        s::MEDLAND
                    } else {
                        hard(self, false)
                    }
                }
            }
        } else {
            hard(self, true)
        };
        log::debug!("land yvel={} seq={}", self.ch.yvel, seq);
        self.jumpseq(seq);
        self.animchar();
        self.ch.yvel = 0;
    }

    /// Hasn't hit the floor yet: can he grab the edge above?
    fn fallon(&mut self) {
        if !(self.joy.btn && self.ch.life & 0x80 != 0) {
            return;
        }
        if self.ch.yvel >= GRABSPEED {
            return;
        }
        let by = self.ch.block_y as i8 as i32;
        if self.ch.y.wrapping_add(GRABLEAD) < self.floor_y(by + 1) {
            return;
        }
        self.savekidx = self.ch.x;
        self.ch.x = self.addcharx(GRABREACH);
        self.rereadblocks();
        self.blockid = self.getabove();
        let a = self.getaboveinf();
        if !self.checkledge(a) {
            self.ch.x = self.savekidx;
            self.rereadblocks();
            return;
        }
        let d = self.getdist();
        self.ch.x = self.addcharx(d);
        let by = self.ch.block_y as i8 as i32;
        self.ch.y = self.floor_y(by + 1);
        self.ch.yvel = 0;
        self.jumpseq(s::FALLHANG);
        self.animchar();
        self.stunned = STUNTIME;
        self.screaming = false;
        self.add_sound(Sound::Grab);
        log::debug!("grabbed ledge");
    }

    /// Is there floor underfoot? If not, start to fall.
    fn onground(&mut self) {
        if self.frame.check & FCHECKMARK == 0 {
            return;
        }
        let mut id = self.getunderft();
        if id == obj::BLOCK {
            id = self.insideblock();
        }
        if !is_space(id) {
            return;
        }
        // Level 12: after the merge, a phantom bridge appears underfoot
        if self.level.number == 12 && self.mergetimer < 0 && self.ch.block_y == 0 {
            let s = self.ch.scrn;
            if s == 2 || (s == 13 && self.rd.bx >= 6) {
                self.set_type(obj::FLOOR);
                return;
            }
        }
        self.startfall();
    }

    fn startfall(&mut self) {
        self.ch.sword = 0;
        self.ch.block_y = self.ch.block_y.wrapping_add(1);
        self.addslicers();
        let p = self.ch.posn;
        self.rjumpflag = p;
        log::debug!("startfall from posn {p}");
        let seq = match p {
            9 => s::STEPFALL,
            13 => s::STEPFALL2,
            26 => s::JUMPFALL,
            44 => s::RJUMPFALL,
            81..=85 => {
                self.ch.x = self.addcharx(5);
                self.rereadblocks();
                s::STEPFALL2
            }
            150..=179 => {
                if self.ch.id >= 2 {
                    // did the guard step off forwards or backwards?
                    if self.ch.xvel & 0x80 != 0 {
                        s::EFIGHTFALL
                    } else {
                        self.droppedout = false;
                        s::EFIGHTFALLFWD
                    }
                } else {
                    self.droppedout = true; // for the guard's benefit
                    s::FIGHTFALL
                }
            }
            _ => s::STEPFALL,
        };
        self.jumpseq(seq);
        self.animchar();
        self.rereadblocks();
        let id = self.getunderft();
        if self.cmpwall(id) {
            self.insideblock();
            return;
        }
        let id = self.getinfront();
        if !self.cmpwall(id) {
            return;
        }
        // CDpatch
        if self.rjumpflag == 44 && self.getdist() < 6 {
            self.jumpseq(s::PATCHFALL);
            self.animchar();
            self.rereadblocks();
            return;
        }
        self.ch.x = self.addcharx(0xff);
        self.rereadblocks();
    }

    /// Char is "inside" a block: bump him outside, hopefully the way he came.
    fn insideblock(&mut self) -> u8 {
        let d = self.getdist();
        let fwd = d < 8 && self.getinfront() != obj::BLOCK;
        let delta = if fwd {
            self.getdist().wrapping_add(4)
        } else if self.getbehind() == obj::BLOCK {
            (self.getdist().wrapping_add(14) ^ 0xff).wrapping_add(8)
        } else {
            (self.getdist() ^ 0xff).wrapping_add(8)
        };
        self.ch.x = self.addcharx(delta);
        self.rereadblocks();
        self.getunderft()
    }

    /// CHECKPRESS: stepping on a pressure plate or loose floor?
    pub fn checkpress(&mut self) {
        let p = self.ch.posn;
        let hanging = (87..100).contains(&p) || (135..141).contains(&p);
        let id = if hanging {
            self.getabove()
        } else {
            if !matches!(self.ch.action, 7 | 5 | 0 | 1) {
                return;
            }
            if p == 79 {
                // jumping up to touch the ceiling
                if self.getabove() == obj::LOOSE {
                    self.breakloose(1);
                }
                return;
            }
            if self.frame.check & FCHECKMARK == 0 {
                return;
            }
            // his block from where he is now: after a closed gate has
            // pushed him back the old one is past it, and he pressed the
            // plate behind the gate (SDLPoP fix_press_through_closed_gates)
            self.getbaseblock();
            self.getunderft()
        };
        match id {
            obj::UPRESSPLATE | obj::PRESSPLATE => {
                if self.ch.life & 0x80 != 0 {
                    self.pushpp();
                } else {
                    self.jampp();
                }
            }
            obj::LOOSE => {
                self.alertguard = true;
                self.breakloose(1);
            }
            _ => {}
        }
    }

    /// CHECKIMPALE: running or jumping onto spikes.
    pub fn checkimpale(&mut self) {
        let (sc, bx, by) = (self.ch.scrn, self.ch.block_x as i8 as i32, self.ch.block_y as i8 as i32);
        if self.rdblock(sc, bx, by) != obj::SPIKES {
            return;
        }
        let p = self.ch.posn;
        if p < 7 {
            return;
        }
        if p < 15 {
            if self.getspikes() >= 2 {
                self.doimpale();
            }
            return;
        }
        if (p == 43 || p == 26) && self.getspikes() != 0 {
            self.doimpale();
        }
    }

    /// DOIMPALE (rd = the spikes)
    pub fn doimpale(&mut self) {
        self.jamspikes();
        let by = self.ch.block_y as i8 as i32;
        self.ch.y = self.floor_y(by + 1);
        self.ch.x = getblockej(self.rd.bx as i8).wrapping_add(10);
        self.ch.x = self.addcharx(8);
        self.ch.yvel = 0;
        self.add_sound(Sound::Spiked);
        self.decstr(100);
        self.jumpseq(s::IMPALE);
        self.animchar();
        log::info!("impaled");
    }

    /// TryPickup: true if something got picked up.
    fn trypickup(&mut self) -> bool {
        let id = self.getunderft();
        if id == obj::FLASK || id == obj::SWORD {
            if is_space(self.getbehind()) {
                return false;
            }
            self.ch.x = self.addcharx((-14i8) as u8);
            self.rereadblocks();
        }
        let id = self.getinfront();
        if id == obj::FLASK || id == obj::SWORD {
            self.pickitup(id);
            return true;
        }
        false
    }

    fn pickitup(&mut self, id: u8) {
        if self.ch.posn != 109 {
            // crouch first, then pick up
            let (d, what) = self.getfwddist();
            if what != 2 {
                self.ch.x = self.addcharx(d);
            }
            if !self.face_left() {
                self.ch.x = self.addcharx((-2i8) as u8);
            }
            self.docrouch();
            return;
        }
        if id == obj::SWORD {
            self.removeobj(0xff);
            self.jumpseq(s::PICKUPSWORD);
        } else {
            let n = self.spec() >> 5;
            self.removeobj(n);
            self.jumpseq(s::DRINKPOTION);
        }
    }

    fn removeobj(&mut self, potion: u8) {
        self.potion = potion;
        self.joy.clr_btn = 1;
        self.set_type(obj::FLOOR);
        self.set_spec(0);
    }
}
