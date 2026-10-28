#!/usr/bin/env python3
"""Turn the C64 port's music into assets/c64/MUSIC.BIN.

The Apple II music module is not in Mechner's released source, so the
music comes from the C64 port (EasyFlash cartridge, pop.crt). Its music
driver sits in bank 29: init at $804B (A = song), play at $80CF, called
from a CIA timer interrupt whose latch the songs set themselves ($63/$64).
This runs that driver in py65 and records, call by call, what changed in
the SID registers and the timer.

MUSIC.BIN, little-endian:
  u8 song count, then per song:
    u8 song number, u32 record count, records of
    (u16 call, u8 register, u8 value)
  register 0x00-0x18 is the SID's, 0x19/0x1a the timer latch lo/hi.

Usage: tools/c64music.py path/to/pop.crt   (needs: pip install py65)
"""
import os
import struct
import sys

from py65.devices.mpu6502 import MPU

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "assets", "c64", "MUSIC.BIN")
BANK = 29
INIT, PLAY = 0x804B, 0x80CF
# the songs the game plays (src/sid.rs maps Apple song cues onto these);
# 29, 30 and 32 repeat 13, 7 and 16, and 1, 4, 5 belong to the title
KEEP = {7, 9, 10, 11, 13, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 27, 28}
SENTINEL = 0x0300


def bank_image(crt, bank):
    mem = bytearray(0x10000)
    o = int.from_bytes(crt[0x10:0x14], "big")
    while o < len(crt):
        ln = int.from_bytes(crt[o + 4:o + 8], "big")
        b = int.from_bytes(crt[o + 0xA:o + 0xC], "big")
        la = int.from_bytes(crt[o + 0xC:o + 0xE], "big")
        if b == bank:
            data = crt[o + 16:o + ln]
            mem[la:la + len(data)] = data
        o += ln
    return mem


def call(mpu, addr, a=0, limit=500000):
    ret = SENTINEL - 1
    mpu.stPush(ret >> 8)
    mpu.stPush(ret & 0xFF)
    mpu.pc, mpu.a = addr, a
    for _ in range(limit):
        if mpu.pc == SENTINEL:
            return True
        mpu.step()
    return False


def run(mem, song, max_calls=8000):
    mpu = MPU()
    mpu.memory[:] = mem
    mpu.memory[SENTINEL] = 0x00
    # init waits four screen frames for the SID to settle: skip the wait
    for a in range(0x806C, 0x807B):
        mpu.memory[a] = 0xEA
    mpu.sp = 0xFF
    if not call(mpu, INIT, song):
        raise RuntimeError(f"song {song}: init did not return")
    state = [None] * 27
    records = []
    for n in range(max_calls):
        if not call(mpu, PLAY):
            raise RuntimeError(f"song {song}: play did not return")
        regs = list(mpu.memory[0xD400:0xD419]) + [mpu.memory[0x63], mpu.memory[0x64]]
        for r, v in enumerate(regs):
            if state[r] != v:
                records.append((n, r, v))
                state[r] = v
        if mpu.memory[0x65] == 0:
            return records, n + 1
    raise RuntimeError(f"song {song}: no end in {max_calls} calls")


def main():
    crt = open(sys.argv[1], "rb").read()
    if not crt.startswith(b"C64 CARTRIDGE"):
        sys.exit("not a .crt file")
    mem = bank_image(crt, BANK)
    out = bytearray()
    songs = [s for s in range(33) if mem[0x802A + s] and s in KEEP]
    out.append(len(songs))
    for s in songs:
        records, calls = run(mem, s)
        out += struct.pack("<BI", s, len(records))
        for n, r, v in records:
            out += struct.pack("<HBB", n, r, v)
        print(f"song {s:2}: {calls:5} calls, {len(records):5} changes")
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    open(OUT, "wb").write(out)
    print(f"{OUT}: {len(out)} bytes")


if __name__ == "__main__":
    main()
