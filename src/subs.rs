//! Character subroutines: CTRLSUBS.S, plus GRAVITY/ADDFALL (SUBS.S) and the
//! sequence interpreter ANIMCHAR (COLL.S).

use crate::apple;
use crate::game::*;
use crate::level::{is_space, obj};
use crate::seq::{self, op, FFOOTMARK, FTHINMARK};

pub const THINNER: u8 = 3;

/// CMPBARR: barrier code (0 clear, 1 panel/gate, 3 mirror/slicer, 4 block).
pub fn cmpbarr(id: u8) -> u8 {
    match id {
        obj::PANELWIF | obj::PANELWOF | obj::GATE => 1,
        obj::MIRROR | obj::SLICER => 3,
        obj::BLOCK => 4,
        _ => 0,
    }
}

/// Distance from either edge of a block to a barrier (COLL.S BarL/BarR).
pub const BAR_L: [u8; 5] = [0, 12, 2, 0, 0];
pub const BAR_R: [u8; 5] = [0, 0, 9, 11, 0];

impl Game {
    pub fn face_left(&self) -> bool {
        self.ch.face & 0x80 != 0
    }

    /// ADDCHARX: CharX + A in the direction the char faces.
    pub fn addcharx(&self, a: u8) -> u8 {
        if self.face_left() {
            self.ch.x.wrapping_sub(a)
        } else {
            self.ch.x.wrapping_add(a)
        }
    }

    /// FACEDX
    pub fn facedx(&self, a: u8) -> u8 {
        if self.face_left() {
            a
        } else {
            a.wrapping_neg()
        }
    }

    pub fn rdblock(&mut self, scrn: u8, bx: i32, by: i32) -> u8 {
        self.rd = self.level.rdblock(scrn, bx, by);
        self.rd.id
    }

    /// RDBLOCK1: reread the last block (after changing rd.bx etc.)
    pub fn rdblock1(&mut self) -> u8 {
        let (s, x, y) = (self.rd.scrn, self.rd.bx, self.rd.by);
        self.rdblock(s, x, y)
    }

    /// (BlueSpec),y for the last rdblock
    pub fn spec(&self) -> u8 {
        self.level.spec(&self.rd)
    }

    pub fn set_spec(&mut self, v: u8) {
        if let Some(i) = self.rd.idx {
            self.level.bluespec[i] = v;
        }
    }

    pub fn set_type(&mut self, id: u8) {
        if let Some(i) = self.rd.idx {
            self.level.set_type(i, id);
        }
    }

    fn bx(&self) -> i32 {
        self.ch.block_x as i8 as i32
    }
    fn by(&self) -> i32 {
        self.ch.block_y as i8 as i32
    }
    fn fwd(&self) -> i32 {
        if self.face_left() {
            -1
        } else {
            1
        }
    }

    pub fn getunderft(&mut self) -> u8 {
        let (s, x, y) = (self.ch.scrn, self.bx(), self.by());
        self.rdblock(s, x, y)
    }
    pub fn getinfront(&mut self) -> u8 {
        self.infrontx = self.bx() + self.fwd();
        let (s, x, y) = (self.ch.scrn, self.infrontx, self.by());
        self.rdblock(s, x, y)
    }
    pub fn get2infront(&mut self) -> u8 {
        let (s, x, y) = (self.ch.scrn, self.bx() + 2 * self.fwd(), self.by());
        self.rdblock(s, x, y)
    }
    pub fn getbehind(&mut self) -> u8 {
        self.behindx = self.bx() - self.fwd();
        let (s, x, y) = (self.ch.scrn, self.behindx, self.by());
        self.rdblock(s, x, y)
    }
    pub fn getabove(&mut self) -> u8 {
        self.abovey = self.by() - 1;
        let (s, x, y) = (self.ch.scrn, self.bx(), self.abovey);
        self.rdblock(s, x, y)
    }
    pub fn getaboveinf(&mut self) -> u8 {
        self.infrontx = self.bx() + self.fwd();
        self.abovey = self.by() - 1;
        let (s, x, y) = (self.ch.scrn, self.infrontx, self.abovey);
        self.rdblock(s, x, y)
    }
    pub fn getabovebeh(&mut self) -> u8 {
        self.behindx = self.bx() - self.fwd();
        self.abovey = self.by() - 1;
        let (s, x, y) = (self.ch.scrn, self.behindx, self.abovey);
        self.rdblock(s, x, y)
    }

    /// GETBASEX: the char's base (foot) x-coord.
    pub fn getbasex(&self) -> u8 {
        let a = (self.frame.check & FFOOTMARK).wrapping_neg().wrapping_add(self.frame.dx);
        self.addcharx(a)
    }

    /// GETDIST: pixels (0-13) to add to CharX to reach the end of this block.
    pub fn getdist(&self) -> u8 {
        self.getdist1(self.getbasex())
    }

    pub fn getdist1(&self, a: u8) -> u8 {
        let (_, off) = getblockxp(a);
        if self.face_left() {
            off
        } else {
            13 - off
        }
    }

    /// GETFRAMEINFO
    pub fn getframeinfo(&mut self) {
        self.frame = seq::frame(self.ch.id, self.ch.posn);
    }

    /// GETBASEBLOCK
    pub fn getbaseblock(&mut self) {
        self.ch.block_x = getblockxp(self.getbasex()).0 as u8;
    }

    /// REREADBLOCKS
    pub fn rereadblocks(&mut self) {
        self.getframeinfo();
        self.getbaseblock();
    }

    /// SETUPCHAR: frame data to 280 x 192 screen coords.
    pub fn setupchar(&mut self) {
        self.getframeinfo();
        let (table, image) = seq::decode_image(&self.frame);
        let a = self.addcharx(self.frame.dx).wrapping_sub(SCRN_LEFT);
        let mut x = a as i32 * 2;
        if x >= 256 && (x & 0xff) >= 0xf0 {
            x -= 512;
        }
        // odd x-coord when the Fcheck hibit matches the facing
        if (self.frame.check ^ self.ch.face) & 0x80 == 0 {
            x += 1;
        }
        self.fchar = FChar {
            x,
            y: self.frame.dy.wrapping_add(self.ch.y),
            face: self.ch.face,
            image,
            table,
        };
    }

    /// GETEDGES: image bounds in 140-res, plus the collision edges.
    pub fn getedges(&mut self) {
        let (w, h) = apple::chtable(self.fchar.table, self.level.number)
            .and_then(|t| t.get(self.fchar.image))
            .map_or((0, 0), |i| (i.w as u8, i.h as u8));
        let imwidth = (w * 7 + 1) >> 1;
        let x140 = ((self.fchar.x >> 1) as u8).wrapping_add(SCRN_LEFT);
        let left = if self.face_left() { x140 } else { x140.wrapping_sub(imwidth) };
        let right = left.wrapping_add(imwidth);
        let thin = if self.frame.check & FTHINMARK != 0 { THINNER } else { 0 };
        self.edges = Edges {
            left,
            right,
            cd_left: left.wrapping_add(thin),
            cd_right: right.wrapping_sub(thin),
            imheight: h,
        };
    }

    /// CHECKLEDGE: can the char grab the ledge (A = the floorpiece) with
    /// `blockid` the space above it? Uses the spec of the last rdblock.
    pub fn checkledge(&self, a: u8) -> bool {
        let tempstate = self.spec();
        let b = self.blockid;
        if b == obj::BLOCK {
            return false;
        }
        if b == obj::PANELWOF && !self.face_left() {
            return false;
        }
        if !is_space(b) {
            return false;
        }
        if a == obj::LOOSE && a & tempstate != 0 {
            return false;
        }
        if a == obj::PANELWIF && self.face_left() {
            return false;
        }
        !is_space(a)
    }

    /// CMPWALL: solid block, or a panel if facing left.
    pub fn cmpwall(&self, id: u8) -> bool {
        id == obj::BLOCK || (self.face_left() && (id == obj::PANELWIF || id == obj::PANELWOF))
    }

    /// ANIMCHAR: run the sequence table up to the next frame number.
    pub fn animchar(&mut self) {
        for _ in 0..64 {
            let b = self.getseq();
            match b {
                op::CHX => {
                    let d = self.getseq();
                    self.ch.x = self.addcharx(d);
                }
                op::CHY => {
                    let d = self.getseq();
                    self.ch.y = self.ch.y.wrapping_add(d);
                }
                op::ABOUTFACE => self.ch.face ^= 0xff,
                op::GOTO => {
                    let lo = self.getseq();
                    let hi = self.getseq();
                    self.ch.seq = seq::addr(lo, hi);
                }
                op::UP => self.ch.block_y = self.ch.block_y.wrapping_sub(1),
                op::DOWN => self.ch.block_y = self.ch.block_y.wrapping_add(1),
                op::ACT => self.ch.action = self.getseq(),
                op::SETFALL => {
                    self.ch.xvel = self.getseq();
                    self.ch.yvel = self.getseq();
                }
                op::IFWTLESS => {
                    let lo = self.getseq();
                    let hi = self.getseq();
                    if self.weightless != 0 {
                        self.ch.seq = seq::addr(lo, hi);
                    }
                }
                op::DIE => {}
                op::JARU => self.jarabove = 1,
                op::JARD => self.jarabove = -1,
                op::TAP => {
                    let n = self.getseq();
                    match n {
                        0 => self.alertguard = true,
                        1 => {
                            self.add_sound(Sound::Footstep);
                            self.alertguard = true;
                        }
                        2 => {
                            self.add_sound(Sound::SmackWall);
                            self.alertguard = true;
                        }
                        _ => {}
                    }
                }
                op::NEXTLEVEL => {
                    self.add_sound(Sound::Upstairs);
                    self.next_level = true;
                }
                op::EFFECT => {
                    let n = self.getseq();
                    if n == 1 {
                        self.potion_effect();
                    }
                }
                frame => {
                    self.ch.posn = frame;
                    return;
                }
            }
        }
        log::warn!("animchar: no frame found, seq {:#x}", self.ch.seq);
    }

    fn getseq(&mut self) -> u8 {
        let b = seq::byte(self.ch.seq);
        self.ch.seq = self.ch.seq.wrapping_add(1);
        b
    }

    /// GRAVITY
    pub fn gravity(&mut self) {
        if self.ch.action != 4 {
            return;
        }
        let (acc, term) = if self.weightless != 0 { (1, 4) } else { (3, 33) };
        let v = self.ch.yvel.wrapping_add(acc);
        self.ch.yvel = if v >= term { term } else { v };
    }

    /// ADDFALL
    pub fn addfall(&mut self) {
        self.ch.y = self.ch.y.wrapping_add(self.ch.yvel);
        if self.ch.action != 4 {
            return;
        }
        self.ch.x = self.addcharx(self.ch.xvel);
        self.rereadblocks();
    }

    /// POTIONEFFECT (MISC.S), for the item REMOVEOBJ recorded.
    fn potion_effect(&mut self) {
        if self.ch.id != 0 {
            return;
        }
        let kind = self.potion;
        log::info!("potion {kind:#x}");
        match kind {
            0xff => {
                self.gotsword = true;
                self.lightcolor = 0xff;
                self.lightning = 3;
                self.show_message("YOU HAVE THE SWORD".into(), 2 * FPS);
            }
            1 => {
                if self.kid_strength != self.max_strength {
                    self.lightcolor = 0x99;
                    self.lightning = 2;
                    self.chg_kid_str = 1;
                }
            }
            2 => {
                self.lightcolor = 0x99;
                self.lightning = 5;
                self.boostmeter();
            }
            3 => self.weightless = 200,
            4 => self.inverted = !self.inverted,
            5 => {
                self.add_sound(Sound::Splat);
                self.chg_kid_str = 0xff;
            }
            _ => {}
        }
        self.add_sound(Sound::Drink);
    }
}
