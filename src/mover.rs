//! Moving gadgets: a port of MOVER.S.
//!
//! TROBs ("transitional objects") are blocks whose BLUESPEC state animates:
//! gates, pressure plates, loose floors, spikes, the exit door, torches and
//! bubbling flasks. MOBs are loose floor pieces that have broken off and
//! are falling.

use crate::game::*;
use crate::level::{is_space, obj, Level};
use crate::seq::seq as s;

const GATEVEL: [u8; 9] = [0, 0, 0, 20, 40, 60, 80, 100, 120];
const MAXGATEVEL: i8 = 8;
const PPTIMER: u8 = 5;
const SPIKETIMER: u8 = 15 + 128;
pub const GMAXVAL: u8 = 47 * 4;
const GATETIMER: u8 = GMAXVAL + 50;
const LOOSETIMER: u8 = FFALLING;
pub const FFALLING: u8 = 10;
const WIGGLETIME: u8 = 4;
const FFACCEL: u8 = 3;
const FFTERMVEL: u8 = 29;
const CRUMBLETIME: u8 = 2;
const DISAPPEARTIME: u8 = 2;
const CRUSHDIST: u8 = 30;
const GATEINC: [u8; 3] = [0xff, 4, 4];
const EXITINC: u8 = 4;
pub const EMAXVAL: u8 = 43 * 4;
pub const SPIKE_EXT: u8 = 5;
const SPIKE_RET: u8 = 9;
pub const TORCH_LAST: u8 = 17;
pub const BUBB_LAST: u8 = 8;
const MAXTR: usize = 30;
const MAXMOB: usize = 14;
const REQMASK: u8 = 0b0010_0000;
pub const SLICER_EXT: u8 = 2;
pub const SLICER_RET: u8 = 6;
const SLICETIMER: u8 = 15;
const SLICERSYNC: u8 = 3;

/// SETINITIALS: initial gadget states for a freshly loaded level.
pub fn set_initials(l: &mut Level) {
    for i in 0..720 {
        match l.bluetype[i] & 0x1f {
            obj::GATE => {
                l.bluespec[i] = match l.bluespec[i] {
                    1 => GMAXVAL,
                    2 => 0,
                    v => v,
                }
            }
            obj::LOOSE => l.bluespec[i] = 0,
            obj::FLASK => l.bluespec[i] <<= 5,
            _ => {}
        }
    }
}

impl Game {
    fn addtrob(&mut self, loc: u8, scrn: u8, direc: i8) {
        if let Some(t) = self.trobs.iter_mut().find(|t| t.loc == loc && t.scrn == scrn) {
            t.direc = direc;
            return;
        }
        if self.trobs.len() < MAXTR {
            self.trobs.push(Trob { loc, scrn, direc });
        }
    }

    fn rd_loc(&self) -> Option<(u8, u8)> {
        self.rd.idx.map(|i| ((i % 30) as u8, self.rd.scrn))
    }

    /// ADDTORCHES: start torches, flasks and swords on the visible screen.
    pub fn trigger_screen_objects(&mut self) {
        let v = self.vis_scrn;
        if v == 0 {
            return;
        }
        for loc in 0..30u8 {
            let i = Level::index(v, (loc % 10) as i32, (loc / 10) as i32);
            match self.level.bluetype[i] & 0x1f {
                obj::TORCH => {
                    self.level.bluespec[i] = self.rnd() & 0x0f;
                    self.addtrob(loc, v, 1);
                }
                obj::FLASK => {
                    let r = self.rnd() & 7;
                    self.level.bluespec[i] |= r;
                    self.addtrob(loc, v, 1);
                }
                obj::SWORD => {
                    self.level.bluespec[i] = self.rnd() & 0x1f;
                    self.addtrob(loc, v, 1);
                }
                _ => {}
            }
        }
    }

    /// ADDSLICERS (SUBS.S): start every slicer on Char's row, each a few
    /// frames out of step with the last.
    pub fn addslicers(&mut self) {
        let (scrn, by) = (self.ch.scrn, self.ch.block_y);
        if scrn == 0 || scrn > 24 || by >= 3 {
            return;
        }
        let mut tempstate = SLICETIMER;
        for bx in 0..10u8 {
            let i = Level::index(scrn, bx as i32, by as i32);
            if self.level.bluetype[i] & 0x1f != obj::SLICER {
                continue;
            }
            let v = self.level.bluespec[i];
            let n = v & 0x7f;
            if n != 0 && n < SLICER_RET {
                continue; // in mid-slice: leave it alone
            }
            self.trigslicer(i, (v & 0x80) | tempstate);
            // getnextstate
            tempstate = tempstate.wrapping_sub(SLICERSYNC);
            if tempstate < SLICER_RET {
                tempstate += SLICETIMER + 1 - SLICER_RET;
            }
        }
    }

    /// CRUMBLE (SUBS.S): on level 13, screens 16 and 23 shake loose the
    /// floor of the screen above as you enter.
    pub fn crumble(&mut self) {
        let v = self.vis_scrn;
        if self.level.number != 13 || (v != 23 && v != 16) {
            return;
        }
        let above = self.level.up(v);
        for bx in (2..=7).rev() {
            if self.rdblock(above, bx, 2) == obj::LOOSE {
                let st = (self.rnd() & 0x0f).wrapping_neg();
                self.breakloose(st);
            }
        }
    }

    /// TRIGSLICER: set slicer `i` going from `state`.
    fn trigslicer(&mut self, i: usize, state: u8) {
        let v = self.level.bluespec[i];
        if v != 0 && v < SLICER_RET {
            return; // in mid-slice: don't interfere
        }
        self.level.bluespec[i] = state;
        let loc = (i % 30) as u8;
        let v = self.vis_scrn;
        self.addtrob(loc, v, 1);
    }

    /// GETSPIKES: 0 safe, 1 sprung, 2 springing (rd = the spikes).
    pub fn getspikes(&self) -> u8 {
        let v = self.spec();
        if v & 0x80 != 0 {
            return if v == 0xff { 0 } else { 1 };
        }
        if v != 0 && v < SPIKE_EXT {
            2
        } else {
            0
        }
    }

    pub fn trigspikes(&mut self) {
        let v = self.spec();
        if v == 0 {
            if let Some((loc, scrn)) = self.rd_loc() {
                self.addtrob(loc, scrn, 1);
            }
            self.add_sound(Sound::GateDown);
        } else if v & 0x80 != 0 && v != 0xff {
            self.set_spec(SPIKETIMER);
        }
    }

    pub fn jamspikes(&mut self) {
        self.set_spec(0xff);
        if let Some((loc, scrn)) = self.rd_loc() {
            self.addtrob(loc, scrn, -1);
        }
    }

    /// CHECKSPIKES: spikes spring out when the kid passes over them.
    pub fn checkspikes(&mut self) {
        let right = getblockxp(self.edges.right).0;
        if right < 0 {
            return;
        }
        let left = getblockxp(self.edges.left).0;
        for bx in left..=right {
            let (scrn, mut by) = (self.ch.scrn, self.ch.block_y as i8 as i32);
            loop {
                let id = self.rdblock(scrn, bx as i32, by);
                if id == obj::SPIKES {
                    self.trigspikes();
                    break;
                }
                if !is_space(id) || self.rd.scrn == 0 || self.rd.scrn != self.ch.scrn {
                    break;
                }
                by += 1;
            }
        }
    }

    /// BREAKLOOSE1 (rd = the loose floor)
    pub fn breakloose(&mut self, state: u8) {
        let Some(i) = self.rd.idx else { return };
        if self.level.bluetype[i] & REQMASK != 0 {
            return;
        }
        let v = self.level.bluespec[i];
        if v & 0x80 == 0 && v != 0 {
            return;
        }
        self.level.bluespec[i] = state;
        if let Some((loc, scrn)) = self.rd_loc() {
            self.addtrob(loc, scrn, 0);
        }
    }

    fn gettimer(&self, x: u8) -> u8 {
        self.level.linkmap[x as usize] & 0x1f
    }

    fn chgtimer(&mut self, x: u8, t: u8) {
        let m = &mut self.level.linkmap[x as usize];
        *m = (*m & 0xe0) | (t & 0x1f);
    }

    /// PUSHPP (rd = the plate)
    pub fn pushpp(&mut self) {
        let pptype = self.rd.id;
        let link = self.spec();
        self.pushpp1(pptype, link);
    }

    fn pushpp1(&mut self, pptype: u8, link: u8) {
        let t = self.gettimer(link);
        if t == 31 {
            return; // permanently down
        }
        self.chgtimer(link, PPTIMER);
        if t < 2 {
            if let Some((loc, scrn)) = self.rd_loc() {
                self.addtrob(loc, scrn, 1);
            }
            self.alertguard = true;
            self.add_sound(Sound::PlateDown);
        }
        self.trigger(pptype, link);
    }

    /// JAMPP: dead weight on a plate.
    pub fn jampp(&mut self) {
        let link = self.spec();
        if self.rd.id == obj::PRESSPLATE {
            self.set_type(obj::DPRESSPLATE);
            self.pushpp1(obj::PRESSPLATE, link);
        } else {
            self.set_type(obj::FLOOR);
            self.set_spec(0);
            self.pushpp1(obj::RUBBLE, link);
        }
    }

    /// trigger: walk the plate's LINKLOC list.
    fn trigger(&mut self, pptype: u8, mut link: u8) {
        loop {
            let ll = self.level.linkloc[link as usize];
            if ll == 0xff {
                return;
            }
            let lm = self.level.linkmap[link as usize];
            let loc = ll & 0x1f;
            let scrn = (lm >> 5) << 2 | (ll >> 5) & 3;
            if (1..=24).contains(&scrn) && loc < 30 {
                let i = Level::index(scrn, (loc % 10) as i32, (loc / 10) as i32);
                match self.level.bluetype[i] & 0x1f {
                    obj::GATE => {
                        if let Some(d) = self.triggate(i, pptype) {
                            self.addtrob(loc, scrn, d);
                        }
                    }
                    obj::EXIT | obj::EXIT2 => {
                        if self.level.bluespec[i] == 0 {
                            self.addtrob(loc, scrn, 1);
                        }
                    }
                    _ => {}
                }
            }
            if ll & 0x80 != 0 {
                return;
            }
            link = link.wrapping_add(1);
        }
    }

    fn triggate(&mut self, i: usize, pptype: u8) -> Option<i8> {
        let state = self.level.bluespec[i];
        match pptype {
            obj::UPRESSPLATE => {
                if state == 0xff {
                    None
                } else if state >= GMAXVAL {
                    self.level.bluespec[i] = GATETIMER;
                    None
                } else {
                    Some(1)
                }
            }
            obj::RUBBLE => {
                if state >= GMAXVAL {
                    self.level.bluespec[i] = 0xff;
                    None
                } else {
                    Some(2)
                }
            }
            _ => {
                if state == 0 {
                    None
                } else {
                    Some(3)
                }
            }
        }
    }

    /// SHAKEM1: wiggle every loose floor on row `by` of `scrn`.
    pub fn shakem(&mut self, scrn: u8, by: i32) {
        if self.level.number == 13 {
            return;
        }
        for bx in (0..10).rev() {
            if self.rdblock(scrn, bx, by) == obj::LOOSE {
                let v = self.spec();
                if v == 0 {
                    self.set_spec(0x80);
                    if let Some((loc, s)) = self.rd_loc() {
                        self.addtrob(loc, s, 1);
                    }
                }
            }
        }
    }

    /// shakeloose (TOPCTRL.S): landing jars the floorboards.
    pub fn shakeloose(&mut self) {
        let j = std::mem::take(&mut self.jarabove);
        let by = self.ch.block_y as i8 as i32;
        match j {
            j if j < 0 => self.shakem(self.vis_scrn, by),
            j if j > 0 => self.shakem(self.vis_scrn, by - 1),
            _ => {}
        }
    }

    /// ANIMTRANS
    pub fn animtrans(&mut self) {
        let mut i = self.trobs.len();
        while i > 0 {
            i -= 1;
            let t = self.trobs[i];
            let d = self.animobj(t);
            self.trobs[i].direc = d;
        }
        self.trobs.retain(|t| t.direc != -1);
    }

    fn animobj(&mut self, t: Trob) -> i8 {
        if t.scrn == 0 || t.scrn > 24 || t.loc >= 30 {
            return -1;
        }
        let i = Level::index(t.scrn, (t.loc % 10) as i32, (t.loc / 10) as i32);
        let mut state = self.level.bluespec[i];
        let mut d = t.direc;
        match self.level.bluetype[i] & 0x1f {
            obj::TORCH => {
                if d >= 0 {
                    if t.scrn != self.vis_scrn {
                        d = -1;
                    } else {
                        state = self.flameframe(state);
                    }
                }
            }
            obj::UPRESSPLATE | obj::PRESSPLATE => {
                if d >= 0 {
                    let tm = self.gettimer(state).wrapping_sub(1);
                    self.chgtimer(state, tm);
                    if tm < 2 {
                        self.add_sound(Sound::PlateUp);
                        d = -1;
                    }
                }
            }
            obj::SPIKES => {
                if d >= 0 {
                    if state & 0x80 != 0 {
                        state -= 1;
                        if state & 0x7f == 0 {
                            state = SPIKE_EXT + 1;
                        }
                    } else {
                        let was = state;
                        state += 1;
                        if was == SPIKE_EXT {
                            state = SPIKETIMER;
                        } else if was == SPIKE_RET {
                            state = 0;
                            d = -1;
                        }
                    }
                }
            }
            obj::LOOSE => {
                if d >= 0 {
                    state = state.wrapping_add(1);
                    if state & 0x80 != 0 {
                        if self.level.number != 13 && state >= WIGGLETIME + 0x80 {
                            state = 0;
                            d = -1;
                        }
                    } else if state >= LOOSETIMER {
                        // floor detaches: becomes space, a MOB takes over
                        self.level.set_type(i, obj::SPACE);
                        state = if self.level.palace() { 1 } else { 0 };
                        d = -1;
                        let level = t.loc / 10;
                        if self.mobs.len() < MAXMOB {
                            self.mobs.push(Mob {
                                x: (t.loc % 10) * 4,
                                y: BLOCK_BOT[level as usize + 1],
                                scrn: t.scrn,
                                vel: 0,
                                level,
                            });
                        }
                    }
                }
            }
            obj::SPACE => d = -1,
            obj::SLICER => self.animslicer(t, &mut state, &mut d),
            obj::GATE => self.animgate(&mut state, &mut d),
            obj::EXIT => self.animexit(&mut state, &mut d),
            obj::FLASK => {
                if d >= 0 {
                    if t.scrn != self.vis_scrn {
                        d = -1;
                    } else {
                        let mut f = (state & 0x1f) + 1;
                        if f > BUBB_LAST {
                            f = 1;
                        }
                        state = (state & 0xe0) | f;
                    }
                }
            }
            obj::SWORD => {
                if t.scrn != self.vis_scrn {
                    d = -1;
                } else {
                    state = state.wrapping_sub(1);
                    if state == 0 {
                        state = (self.rnd() & 0x3f) + 40;
                    }
                }
            }
            _ => d = -1,
        }
        self.level.bluespec[i] = state;
        d
    }

    /// animslicer: the jaws close at SLICER_EXT; a slicer keeps going
    /// while the kid is alive on its row of the visible screen (or while
    /// it is bloodied), otherwise it stops once retracted.
    fn animslicer(&mut self, t: Trob, state: &mut u8, d: &mut i8) {
        if *d < 0 {
            return;
        }
        let hib = *state & 0x80;
        let mut n = (*state & 0x7f) + 1;
        if n > SLICETIMER {
            n = 1;
        }
        *state = n | hib;
        if n == SLICER_EXT {
            self.add_sound(Sound::JawsClash);
        }
        let keep = t.scrn == self.vis_scrn
            && t.loc / 10 == self.kid.block_y
            && (self.kid.life & 0x80 != 0 || hib != 0);
        if !keep && n >= SLICER_RET {
            *d = -1;
        }
    }

    fn flameframe(&mut self, state: u8) -> u8 {
        let r = self.rnd();
        if r != state && r <= TORCH_LAST {
            return r;
        }
        let n = state + 1;
        if n > TORCH_LAST {
            0
        } else {
            n
        }
    }

    fn animgate(&mut self, state: &mut u8, d: &mut i8) {
        if *d < 0 {
            return;
        }
        if *d >= 3 {
            if *d < MAXGATEVEL {
                *d += 1;
            }
            let v = GATEVEL[*d as usize];
            if *state <= v {
                *state = 0;
                *d = -1;
                self.add_sound(Sound::GateSlam);
            } else {
                *state -= v;
            }
            return;
        }
        if *state == 0xff {
            *d = -1;
            self.add_sound(Sound::GateDown);
            return;
        }
        *state = state.wrapping_add(GATEINC[*d as usize]);
        if *d == 0 {
            if *state == 0 {
                *d = -1;
                self.add_sound(Sound::GateDown);
            }
            return;
        }
        if *state >= GMAXVAL {
            if *d == 2 {
                *state = 0xff;
                *d = -1;
            } else {
                *state = GATETIMER;
                *d = 0;
            }
        } else {
            self.add_sound(Sound::RaisingGate);
        }
    }

    fn animexit(&mut self, state: &mut u8, d: &mut i8) {
        if *d < 0 {
            return;
        }
        if *d >= 3 {
            if *d < MAXGATEVEL {
                *d += 1;
            }
            let v = GATEVEL[*d as usize];
            if *state <= v {
                *state = 0;
                *d = -1;
                self.add_sound(Sound::GateSlam);
            } else {
                *state -= v;
            }
            return;
        }
        *state = state.wrapping_add(EXITINC);
        if *state >= EMAXVAL {
            *d = -1;
            self.add_sound(Sound::GateDown);
            self.exitopen = 1;
            log::info!("exit open");
            self.mirappear();
        }
    }

    /// MIRAPPEAR (SUBS.S): on level 4 the mirror appears when the exit opens.
    fn mirappear(&mut self) {
        if self.level.number == 4 {
            let i = Level::index(4, 4, 0);
            self.level.set_type(i, obj::MIRROR);
        }
    }

    /// ANIMMOBS: falling floors.
    pub fn animmobs(&mut self) {
        let mut i = self.mobs.len();
        while i > 0 {
            i -= 1;
            let mut m = self.mobs[i];
            if m.vel & 0x80 == 0 {
                self.mobfloor(&mut m);
            }
            if m.vel & 0x80 != 0 {
                m.vel = m.vel.wrapping_add(1);
            }
            self.checkcrush(&m);
            self.mobs[i] = m;
        }
        self.mobs.retain(|m| m.vel != 0xff);
    }

    fn mobfloor(&mut self, m: &mut Mob) {
        if m.vel < FFTERMVEL {
            m.vel += FFACCEL;
        }
        m.y = m.y.wrapping_add(m.vel);
        if m.scrn == 0 {
            if m.y >= 192 + 17 {
                m.vel = DISAPPEARTIME.wrapping_neg();
            }
            return;
        }
        if m.y >= (-30i8) as u8 || m.y < BLOCK_AY[m.level as usize + 1] {
            return;
        }
        let id = self.rdblock(m.scrn, (m.x / 4) as i32, m.level as i32);
        match id {
            obj::SPACE => self.passthru(m),
            obj::LOOSE => {
                // knock out the loose floor and keep falling
                self.level.set_type(self.rd.idx.unwrap(), obj::SPACE);
                let st = if self.level.palace() { 1 } else { 0 };
                self.set_spec(st);
                m.vel >>= 1;
                let mut n = *m;
                n.y = n.y.wrapping_add(6);
                self.passthru(&mut n);
                if self.mobs.len() < MAXMOB {
                    self.mobs.push(n);
                }
            }
            _ => {
                self.add_sound(Sound::LooseCrash);
                self.shakem(m.scrn, m.level as i32);
                m.y = BLOCK_AY[m.level as usize + 1];
                m.vel = CRUMBLETIME.wrapping_neg();
                self.makerubble(m);
            }
        }
    }

    fn passthru(&self, m: &mut Mob) {
        m.level += 1;
        if m.level >= 3 {
            m.y = m.y.wrapping_sub(192);
            m.level = 0;
            m.scrn = self.level.down(m.scrn);
        }
    }

    fn makerubble(&mut self, m: &Mob) {
        let id = self.rdblock(m.scrn, (m.x / 4) as i32, m.level as i32);
        match id {
            obj::PRESSPLATE => {
                self.pushpp();
                self.set_type(obj::RUBBLE);
            }
            obj::UPRESSPLATE => {
                self.set_type(obj::RUBBLE);
                let link = self.spec();
                self.pushpp1(obj::RUBBLE, link);
            }
            obj::FLOOR | obj::SPIKES | obj::FLASK | obj::TORCH => self.set_type(obj::RUBBLE),
            _ => {}
        }
    }

    /// checkcrush: did the falling floor land on the kid?
    fn checkcrush(&mut self, m: &Mob) {
        self.load_kid();
        let k = self.ch;
        if m.scrn != k.scrn || (m.x / 4) as i8 != k.block_x as i8 {
            return;
        }
        if m.y >= k.y || k.y.wrapping_sub(CRUSHDIST) >= m.y {
            return;
        }
        if self.level.number != 13 && (5..15).contains(&k.posn) {
            return; // running: escape
        }
        if !matches!(k.action, 0 | 1 | 7) {
            return;
        }
        let by = k.block_y as i8 as i32;
        self.ch.y = FLOOR_Y[(by + 1).clamp(0, 4) as usize];
        if !self.decstr(1) {
            self.jumpseq(s::HARDLAND);
        } else if k.posn != 109 {
            self.jumpseq(s::CRUSH);
        }
        self.save_kid();
        log::info!("crushed by falling floor");
    }
}
