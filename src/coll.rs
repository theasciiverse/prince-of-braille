//! Collision detection with vertical barriers (COLL.S).
//!
//! The original compares, block by block, which side of every barrier the
//! character's leading edges were on last frame and this frame. A nybble
//! flipping from 0 to 1 means an edge was crossed: that's a collision.

use crate::game::*;
use crate::level::{is_space, obj};
use crate::seq::seq as s;
use crate::mover::SLICER_EXT;
use crate::subs::{cmpbarr, BAR_L, BAR_R};

const GATEMARGIN: u8 = 6;
const OOF_VELOCITY: u8 = 22;

#[derive(Clone, Copy)]
enum Buf {
    This,
    Above,
    Below,
}

impl Game {
    /// gatebarr?: true (carry set) if the gate is open enough to pass.
    pub fn gate_open(&self) -> bool {
        (self.spec() >> 2).wrapping_add(GATEMARGIN) >= self.edges.imheight
    }

    pub fn getleftbar(&mut self, scrn: u8, bx: i32, by: i32) -> u8 {
        let id = self.rdblock(scrn, bx, by);
        match cmpbarr(id) {
            0 => 255,
            c => self.blockedge.wrapping_add(BAR_L[c as usize]),
        }
    }

    pub fn getrightbar(&mut self, scrn: u8, bx: i32, by: i32) -> u8 {
        let id = self.rdblock(scrn, bx, by);
        match cmpbarr(id) {
            0 => 0,
            c => self.blockedge.wrapping_add(13).wrapping_sub(BAR_R[c as usize]),
        }
    }

    fn getcdata(&mut self, blocky: i32, buf: Buf, beg: i32, end: i32) {
        self.blockedge = getblockej(beg as i8).wrapping_add(ANGLE);
        let mut i = beg;
        while i < end {
            let scrn = self.ch.scrn;
            let lb = self.getleftbar(scrn, i, blocky);
            let mut cd = if lb < self.edges.cd_right { 0x0f } else { 0 };
            let rb = self.getrightbar(scrn, i, blocky);
            if rb > self.edges.cd_left {
                cd |= 0xf0;
            }
            let x = self.rd.bx as usize;
            let sn = self.rd.scrn;
            match buf {
                Buf::This => {
                    self.cd_this[x] = cd;
                    self.sn_this[x] = sn;
                }
                Buf::Above => {
                    self.cd_above[x] = cd;
                    self.sn_above[x] = sn;
                }
                Buf::Below => {
                    self.cd_below[x] = cd;
                    self.sn_below[x] = sn;
                }
            }
            self.blockedge = self.blockedge.wrapping_add(14);
            i += 1;
        }
    }

    fn init_cd_bufs(&mut self, this: u8) {
        let last = self.block_y_last;
        let src = if this == last || this.wrapping_add(3) == last || this.wrapping_sub(3) == last {
            Buf::This
        } else if this.wrapping_add(1) == last || this.wrapping_sub(2) == last {
            Buf::Above
        } else {
            Buf::Below
        };
        let (sn, cd) = match src {
            Buf::This => (self.sn_this, self.cd_this),
            Buf::Above => (self.sn_above, self.cd_above),
            Buf::Below => (self.sn_below, self.cd_below),
        };
        self.sn_last = sn;
        self.cd_last = cd;
        self.sn_this = [0xff; 10];
        self.sn_above = [0xff; 10];
        self.sn_below = [0xff; 10];
    }

    /// CHECKBARR: returns (collideL, collideR) block numbers.
    pub fn checkbarr(&mut self) -> (Option<usize>, Option<usize>) {
        if self.ch.action == 7 {
            return (None, None);
        }
        let this = self.ch.block_y;
        self.init_cd_bufs(this);
        self.block_y_last = this;

        let end = (getblockxp(self.edges.cd_right).0 as i32 + 2).min(11);
        let beg = getblockxp(self.edges.cd_left).0 as i32 - 1;
        let by = this as i8 as i32;
        self.getcdata(by, Buf::This, beg, end);
        self.getcdata(by + 1, Buf::Below, beg, end);
        self.getcdata(by - 1, Buf::Above, beg, end);

        let (mut cl, mut cr) = (None, None);
        for x in (0..10).rev() {
            if self.sn_this[x] == 0xff || self.sn_this[x] != self.sn_last[x] {
                continue;
            }
            if self.cd_last[x] & 0x0f == 0 && self.cd_this[x] & 0x0f != 0 {
                cl = Some(x);
            }
            if self.cd_last[x] & 0xf0 == 0 && self.cd_this[x] & 0xf0 != 0 {
                cr = Some(x);
            }
        }
        (cl, cr)
    }

    /// COLLISIONS: act on what CHECKBARR found.
    pub fn collisions(&mut self, cl: Option<usize>, cr: Option<usize>) {
        if matches!(self.ch.action, 2 | 6) || (135..149).contains(&self.ch.posn) {
            return;
        }
        // left edge of a barrier: only matters walking right
        if let Some(x) = cl {
            if (self.ch.sword == 2 || !self.face_left()) && self.checkcoll1(x) {
                let (s, bx, by) = (self.rd.scrn, self.rd.bx, self.rd.by);
                let d = self.getleftbar(s, bx, by).wrapping_sub(self.edges.cd_right);
                self.collide(d, true);
            }
        }
        // the original looked at a right edge only when there was no left
        // one, so with a barrier on each side he could pass through the
        // second; both are checked, as SDLPoP's fix_two_coll_bug does
        if let Some(x) = cr {
            if (self.ch.sword == 2 || self.face_left()) && self.checkcoll1(x) {
                let (s, bx, by) = (self.rd.scrn, self.rd.bx, self.rd.by);
                let d = self.getrightbar(s, bx, by).wrapping_sub(self.edges.cd_left);
                self.collide(d, false);
            }
        }
    }

    fn checkcoll1(&mut self, x: usize) -> bool {
        let mut by = self.ch.block_y as i8 as i32;
        if by < 0 {
            by += 3;
        } else if by >= 3 {
            by -= 3;
        }
        let scrn = self.sn_this[x];
        self.rdblock(scrn, x as i32, by);
        self.checkcoll()
    }

    /// CHECKCOLL: is the block in `rd` really in the way?
    pub fn checkcoll(&mut self) -> bool {
        match self.rd.id {
            obj::FLASK => return false,
            obj::GATE => {
                if self.gate_open() {
                    return false;
                }
            }
            obj::SLICER => {
                if self.spec() != 2 {
                    return false;
                }
            }
            obj::MIRROR => {
                // passing through from the right takes a running jump
                if self.ch.id == 0 && (39..44).contains(&self.ch.posn) && self.face_left() {
                    // SMASHMIRROR; the reflection comes to life
                    self.set_spec(86);
                    self.createshad = true;
                    return false;
                }
            }
            _ => {}
        }
        let mut edge = getblockej(self.rd.bx as i8);
        edge = self.adjust_scrn(edge);
        self.blockedge = edge.wrapping_add(ANGLE);
        true
    }

    /// AdjustScrn: x-coord on rd.scrn to x-coord on the visible screen.
    fn adjust_scrn(&self, a: u8) -> u8 {
        let t = self.rd.scrn;
        let v = self.vis_scrn;
        if t == v {
            return a;
        }
        let l = &self.level;
        let below = l.down(v);
        if t == l.left(v) || t == l.left(below) {
            a.wrapping_sub(140)
        } else if t == l.right(v) || t == l.right(below) {
            a.wrapping_add(140)
        } else {
            a
        }
    }

    /// COLLIDE: `left` is a collision with a barrier's left edge.
    fn collide(&mut self, d: u8, left: bool) {
        if self.ch.life & 0x80 == 0 || self.ch.posn == 177 {
            return;
        }
        self.ch.x = self.ch.x.wrapping_add(d);
        let mut id = self.rdblock1();
        if left {
            if id == obj::BLOCK {
                self.rd.bx -= 1;
                id = self.rdblock1();
            }
        } else if matches!(id, obj::PANELWOF | obj::PANELWIF | obj::BLOCK) {
            self.rd.bx += 1;
            if self.rd.scrn == 0 && self.rd.bx == 10 {
                self.rd.scrn = self.ch.scrn;
                self.rd.bx = 0;
            }
            id = self.rdblock1();
        }
        if is_space(id) {
            self.airbump();
        } else {
            self.groundbump(left);
        }
    }

    fn airbump(&mut self) {
        self.ch.x = self.addcharx((-4i8) as u8);
        if self.ch.action == 4 {
            self.ch.xvel = 0;
        } else {
            self.jumpseq(s::BUMPFALL);
            self.animchar();
        }
        self.alertguard = true;
        self.add_sound(Sound::SmackWall);
    }

    fn groundbump(&mut self, left: bool) {
        let fy = FLOOR_Y[(self.ch.block_y as i8 + 1).clamp(0, 4) as usize];
        if self.ch.sword != 2 && fy.wrapping_sub(self.ch.y) >= 15 {
            self.airbump();
            return;
        }
        self.ch.y = fy;
        if self.ch.yvel >= OOF_VELOCITY {
            self.ch.x = self.addcharx((-5i8) as u8);
            return;
        }
        self.ch.yvel = 0;
        if self.ch.life == 0 {
            return;
        }
        if self.ch.sword == 2 {
            // collide en garde
            let collface = if left { FACE_LEFT } else { FACE_RIGHT };
            if collface == self.ch.face {
                self.jumpseq(s::BUMPENGBACK);
                self.animchar();
                self.ch.x = self.addcharx(1);
            } else {
                self.jumpseq(s::BUMPENGFWD);
                self.animchar();
                self.add_sound(Sound::SmackWall);
            }
            return;
        }
        let p = self.ch.posn;
        let hard = p == 24 || p == 25 || (40..43).contains(&p) || (102..107).contains(&p);
        if hard {
            self.jumpseq(s::HARDBUMP);
            self.animchar();
        } else {
            self.jumpseq(s::BUMP);
            self.animchar();
        }
        self.alertguard = true;
        self.add_sound(Sound::SmackWall);
    }

    /// CHECKSLICE: does this frame's CD buffer show the kid overlapping a
    /// closed slicer?
    pub fn checkslice(&mut self) {
        let by = self.ch.block_y as i8 as i32;
        for x in (0..10).rev() {
            if self.cd_this[x] != 0xff || self.sn_this[x] == 0xff {
                continue;
            }
            let sn = self.sn_this[x];
            if self.rdblock(sn, x as i32, by) == obj::SLICER && self.spec() & 0x7f == SLICER_EXT {
                return self.slice(x as i8);
            }
        }
    }

    /// CHECKSLICE2: the same for an enemy, who has no CD data.
    pub fn checkslice2(&mut self) {
        self.getunderft();
        if self.slice_here() {
            return;
        }
        self.rd.bx += 1;
        self.rdblock1();
        self.slice_here();
    }

    fn slice_here(&mut self) -> bool {
        if self.rd.id != obj::SLICER || self.spec() & 0x7f != SLICER_EXT {
            return false;
        }
        let (sc, bx, by) = (self.rd.scrn, self.rd.bx, self.rd.by);
        self.blockedge = getblockej(bx as i8).wrapping_add(ANGLE);
        if self.getleftbar(sc, bx, by) >= self.edges.cd_right {
            return false;
        }
        if self.getrightbar(sc, bx, by) <= self.edges.cd_left {
            return false;
        }
        self.rdblock(sc, bx, by);
        self.slice(bx as i8);
        true
    }

    /// Slice! (rd = the slicer, `bx` its block)
    fn slice(&mut self, bx: i8) {
        // bloodied, unless it's the skeleton (SDLPoP fix_skeleton_chomper_blood)
        if self.ch.id != 4 {
            let v = self.spec() | 0x80;
            self.set_spec(v);
        }
        if self.ch.posn == 178 {
            return; // already cut in half
        }
        self.ch.x = getblockej(bx).wrapping_add(7);
        self.ch.x = self.addcharx(8);
        let by = self.ch.block_y as i8 as i32;
        self.ch.y = FLOOR_Y[(by + 1).clamp(0, 4) as usize];
        self.decstr(100);
        self.add_sound(Sound::Splat);
        self.jumpseq(s::HALVE);
        self.animchar();
        log::info!("char {} sliced", self.ch.id);
    }

    /// CHECKGATE: a closing gate knocks a character standing under it
    /// to one side.
    pub fn checkgate(&mut self) {
        let p = self.ch.posn;
        if !(self.ch.action == 7 || p == 15 || (108..111).contains(&p)) {
            return;
        }
        if self.getunderft() != obj::GATE {
            self.rd.bx -= 1;
            if self.rdblock1() != obj::GATE {
                return;
            }
        }
        let x = self.rd.bx;
        if !(0..10).contains(&x) || self.cd_this[x as usize] & self.cd_last[x as usize] != 0xff {
            return;
        }
        if self.gate_open() {
            return;
        }
        self.add_sound(Sound::SmackWall);
        let coll_x = self.rd.bx;
        self.getunderft();
        let d: u8 = if self.rd.bx > coll_x { 5 } else { (-5i8) as u8 };
        self.ch.x = self.ch.x.wrapping_add(d);
        log::debug!("knocked aside by a closing gate");
    }

    /// GETFWDDIST: size of a careful step forward (0-14) and what's there
    /// (0 = edge, 1 = barrier, 2 = clear). Leaves `rd` on that block.
    pub fn getfwddist(&mut self) -> (u8, u8) {
        self.getbaseblock();
        self.setupchar();
        self.getedges();

        let id = self.getunderft();
        if cmpbarr(id) != 0 {
            let d = self.dbarr(id);
            if d & 0x80 == 0 {
                return self.tobarr(d);
            }
        }
        let id = self.getinfront();
        if !(id == obj::PANELWOF && !self.face_left()) {
            if cmpbarr(id) != 0 {
                let d = self.dbarr(id);
                if d & 0x80 == 0 {
                    return self.tobarr(d);
                }
            }
            let id = self.getinfront();
            match id {
                obj::LOOSE => return (self.getdist(), 0),
                obj::PRESSPLATE | obj::SWORD | obj::FLASK => {
                    let d = self.getdist();
                    return if d == 0 { (11, 2) } else { (d, 0) };
                }
                _ if is_space(id) => return (self.getdist(), 0),
                _ => return (11, 2),
            }
        }
        (self.getdist(), 0)
    }

    fn tobarr(&self, d: u8) -> (u8, u8) {
        if d >= 14 {
            (11, 2)
        } else {
            (d, 1)
        }
    }

    /// DBarr: distance to the barrier in `rd` (negative if behind).
    fn dbarr(&mut self, id: u8) -> u8 {
        if id == obj::GATE && self.gate_open() {
            return 0xff;
        }
        let edge = getblockej(self.rd.bx as i8).wrapping_add(ANGLE);
        let c = cmpbarr(id) as usize;
        if c == 0 {
            return 0xff;
        }
        if !self.face_left() {
            edge.wrapping_add(BAR_L[c]).wrapping_sub(self.edges.cd_right)
        } else {
            let r = edge.wrapping_add(13).wrapping_sub(BAR_R[c]);
            self.edges.cd_left.wrapping_sub(r)
        }
    }
}
