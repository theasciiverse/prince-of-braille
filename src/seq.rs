//! Sequence and frame tables, assembled from SEQTABLE.S / FRAMEDEF.S.

#[path = "gen/seqtable.rs"]
mod seqtable;
#[path = "gen/framedef.rs"]
#[allow(dead_code)] // altset 2 and the sword table come in with guards and swords
mod framedef;

pub use seqtable::{op, seq};

/// Start of sequence `n` (1-114) as an index into the table (JUMPSEQ).
pub fn start(n: u8) -> u16 {
    let i = 2 * (n as usize - 1);
    let addr = seqtable::SEQ[i] as u16 | (seqtable::SEQ[i + 1] as u16) << 8;
    addr - seqtable::ORG
}

/// GETSEQ: the byte at `pos`.
pub fn byte(pos: u16) -> u8 {
    seqtable::SEQ.get(pos as usize).copied().unwrap_or(op::GOTO)
}

/// A `dw` operand (absolute address) as a table index.
pub fn addr(lo: u8, hi: u8) -> u16 {
    (lo as u16 | (hi as u16) << 8).wrapping_sub(seqtable::ORG)
}

/// Frame definition (FRAMEDEF.S): Fimage, Fsword, Fdx, Fdy, Fcheck.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Frame {
    pub image: u8,
    pub sword: u8,
    pub dx: u8,
    pub dy: u8,
    pub check: u8,
}

pub const FCHECKMARK: u8 = 0b0100_0000;
pub const FTHINMARK: u8 = 0b0010_0000;
pub const FFOOTMARK: u8 = 0b0001_1111;

fn entry(t: &[u8], i: usize) -> Frame {
    match t.get(i * 5..i * 5 + 5) {
        Some(b) => Frame { image: b[0], sword: b[1], dx: b[2], dy: b[3], check: b[4] },
        None => Frame::default(),
    }
}

/// GETFRAME + usealtsets for the kid (CharID 0 uses the main set).
pub fn kid_frame(posn: u8) -> Frame {
    if posn == 0 {
        return Frame::default();
    }
    entry(&framedef::FDEF, posn as usize - 1)
}

/// Guard frames: 150-189 come from altset 1, falling 102-106 from 172-176.
#[allow(dead_code)]
pub fn guard_frame(posn: u8) -> Frame {
    match posn {
        102..=106 => entry(&framedef::ALTSET1, (posn - 102 + 172 - 150) as usize),
        150..=189 => entry(&framedef::ALTSET1, (posn - 150) as usize),
        _ => kid_frame(posn),
    }
}

/// Sword table entry (image, dx, dy) for a 1-based sword frame.
#[allow(dead_code)]
pub fn sword(n: u8) -> (u8, u8, u8) {
    let t = &framedef::SWORDTAB;
    let i = (n as usize).saturating_sub(1) * 3;
    match t.get(i..i + 3) {
        Some(b) => (b[0], b[1], b[2]),
        None => (0, 0, 0),
    }
}

/// decodeim: FCharTable and FCharImage from Fimage/Fsword.
pub fn decode_image(f: &Frame) -> (u8, u8) {
    let table = (f.image & 0x80) >> 5 | (f.sword & 0xc0) >> 6;
    (table, f.image & 0x7f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stand_loops_on_frame_15() {
        let s = start(seq::STAND);
        assert_eq!((byte(s), byte(s + 1)), (op::ACT, 0));
        assert_eq!(byte(s + 2), 15);
        assert_eq!(byte(s + 3), op::GOTO);
        assert_eq!(addr(byte(s + 4), byte(s + 5)), s);
    }

    #[test]
    fn frame_15_is_the_standing_kid() {
        let f = kid_frame(15);
        assert_eq!(f, Frame { image: 0x0f, sword: 9, dx: 0, dy: 0, check: 0x43 });
        assert_eq!(decode_image(&f), (0, 0x0f));
        // jumphang frames use chtable2
        assert_eq!(decode_image(&kid_frame(67)).0, 1);
    }
}
