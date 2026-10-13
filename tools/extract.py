#!/usr/bin/env python3
"""Extract game data from Jordan Mechner's released Apple II source.

Reads the reference archive (Prince-of-Persia-Apple-II-master.zip, or an
unpacked copy of it) and writes:

  assets/apple2/IMG.*      raw image tables (sprites and background pieces)
  assets/apple2/LEVEL*     raw level blueprints (2304 bytes each)
  src/gen/seqtable.rs      SEQTABLE.S assembled to bytes, plus SEQDATA.S names
  src/gen/framedef.rs      FRAMEDEF.S frame, altset and sword tables

The two .S files are assembled by a tiny Merlin subset (labels, local
:labels, equates, db/dfb/dw/hex/ds with +/- expressions, do/else/fin
conditional assembly), which is all those two files use. Everything is copied or assembled byte for byte; no
values are retyped by hand.

Usage: tools/extract.py [path-to-zip-or-dir]
"""

import io
import os
import re
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
DEFAULT_REF = os.path.join(HERE, "..", "..", "..", "references", "Prince of Persia", "Prince-of-Persia-Apple-II-master.zip")
PREFIX = "Prince-of-Persia-Apple-II-master/01 POP Source/"


class Source:
    """Read files from either the zip or an unpacked directory."""

    def __init__(self, path):
        self.zip = zipfile.ZipFile(path) if path.endswith(".zip") else None
        self.dir = None if self.zip else path

    def read(self, rel):
        if self.zip:
            return self.zip.read(PREFIX + rel)
        base = self.dir
        if os.path.isdir(os.path.join(base, "01 POP Source")):
            base = os.path.join(base, "01 POP Source")
        with open(os.path.join(base, rel), "rb") as f:
            return f.read()

    def listdir(self, rel):
        if self.zip:
            pre = PREFIX + rel + "/"
            return sorted(n[len(pre):] for n in self.zip.namelist() if n.startswith(pre) and n != pre)
        base = self.dir
        if os.path.isdir(os.path.join(base, "01 POP Source")):
            base = os.path.join(base, "01 POP Source")
        return sorted(os.listdir(os.path.join(base, rel)))


# --------------------------------------------------------------------------
# Minimal Merlin assembler

class Asm:
    def __init__(self, text, org=0):
        self.lines = text.decode("latin-1").replace("\r", "\n").split("\n")
        self.org = org
        self.syms = {}

    @staticmethod
    def split(line):
        if not line.strip() or line[0] in "*;":
            return None
        # strip comment (no strings in these files)
        semi = line.find(";")
        if semi >= 0:
            line = line[:semi]
        label = None
        if line[0] not in " \t":
            parts = line.split(None, 1)
            label = parts[0]
            rest = parts[1] if len(parts) > 1 else ""
        else:
            rest = line
        rest = rest.strip()
        if not rest:
            return label, None, ""
        parts = rest.split(None, 1)
        op = parts[0]
        operand = parts[1].strip() if len(parts) > 1 else ""
        return label, op, operand

    def scoped(self, name, glob):
        return glob + name if name.startswith(":") else name

    def value(self, expr, glob, pc, strict):
        expr = expr.strip().replace("#", "")
        tokens = re.findall(r"[+-]|[^+-]+", expr)
        total, sign, expect_term = 0, 1, True
        for t in tokens:
            t = t.strip()
            if t in "+-":
                if expect_term:
                    sign = -sign if t == "-" else sign
                else:
                    sign = -1 if t == "-" else 1
                    expect_term = True
                continue
            if t.startswith("$"):
                v = int(t[1:], 16)
            elif t.startswith("%"):
                v = int(t[1:], 2)
            elif t == "*":
                v = pc
            elif t.isdigit():
                v = int(t)
            else:
                name = self.scoped(t, glob)
                if name in self.syms:
                    v = self.syms[name]
                elif strict:
                    raise KeyError(f"undefined symbol {t!r} (scope {glob!r})")
                else:
                    v = 0
            total += sign * v
            sign, expect_term = 1, False
        return total

    def run(self, strict):
        out = bytearray()
        pc = self.org
        glob = ""
        dum = None  # pc inside a dum section
        cond = []  # do/else/fin stack: is each level being assembled?
        for raw in self.lines:
            s = self.split(raw)
            if s is None:
                continue
            label, op, operand = s
            opl = (op or "").lower()
            # conditional assembly: `do EXPR` ... [`else`] ... `fin`
            if opl == "do":
                cond.append(all(cond) and self.value(operand, glob, pc, False) != 0)
                continue
            if opl == "else":
                outer = all(cond[:-1])
                cond[-1] = outer and not cond[-1]
                continue
            if opl == "fin":
                cond.pop()
                continue
            if not all(cond):
                continue
            if label and not label.startswith(":"):
                if opl != "=":
                    glob = label
            here = dum if dum is not None else pc
            if label and opl != "=":
                self.syms[self.scoped(label, glob)] = here
            if op is None:
                continue
            if opl == "=":
                self.syms[label] = self.value(operand, glob, here, False)
                continue
            if opl == "dum":
                dum = self.value(operand, glob, here, strict)
                continue
            if opl == "dend":
                dum = None
                continue
            if opl == "org":
                if out:
                    raise ValueError("org after code")
                pc = self.org = self.value(operand, glob, pc, strict)
                continue
            if opl in ("db", "dfb", "dw", "hex", "ds"):
                if opl == "ds":
                    n = self.value(operand, glob, here, strict)
                    if dum is not None:
                        dum += n
                    else:
                        out += bytes(max(n, 0))
                        pc += max(n, 0)
                    continue
                if dum is not None:
                    raise ValueError("data inside dum")
                if opl == "hex":
                    data = bytes.fromhex(operand.replace(",", "").replace(" ", ""))
                else:
                    data = bytearray()
                    for item in operand.split(","):
                        v = self.value(item, glob, pc, strict)
                        if opl == "dw":
                            data += bytes([v & 0xFF, (v >> 8) & 0xFF])
                        else:
                            data.append(v & 0xFF)
                out += data
                pc += len(data)
                continue
            # tr, lst, lstdo, usr, put, ...: listing/linker directives, no bytes
        return bytes(out)

    def assemble(self):
        self.run(False)  # pass 1: collect labels
        return self.run(True)


# --------------------------------------------------------------------------

def rust_bytes(name, data, per_line=16):
    rows = []
    for i in range(0, len(data), per_line):
        rows.append("    " + ", ".join(f"0x{b:02x}" for b in data[i:i + per_line]) + ",")
    return f"pub static {name}: [u8; {len(data)}] = [\n" + "\n".join(rows) + "\n];\n"


HEADER = """// Generated by tools/extract.py from Jordan Mechner's Apple II source
// ({src}). Do not edit by hand; rerun the extractor.
"""


def gen_seqtable(src):
    seqdata = src.read("Source/SEQDATA.S")
    seqtable = src.read("Source/SEQTABLE.S")
    # SEQTABLE.S does not `put` SEQDATA, but uses only its own equates
    asm = Asm(seqtable)
    code = asm.assemble()
    org = asm.org
    # rebase every dw (absolute address) to an index into SEQ
    n_entries = 114
    out = bytearray(code)
    # the jump table: entries 1..114 are dw at the start
    starts = []
    for i in range(n_entries):
        addr = code[2 * i] | code[2 * i + 1] << 8
        starts.append(addr - org)
    names = []
    sd = Asm(seqdata)
    sd.run(False)
    for line in seqdata.decode("latin-1").replace("\r", "\n").split("\n"):
        m = re.match(r"^([A-Za-z][A-Za-z0-9]*)\s*=\s*(\d+)\s*$", line.split(";")[0].rstrip())
        if m and 1 <= int(m.group(2)) <= n_entries:
            names.append((m.group(1), int(m.group(2))))
    instr = [(k, v) for k, v in asm.syms.items() if isinstance(v, int) and -15 <= v <= -1 and not k.startswith(":")]
    s = HEADER.format(src="SEQTABLE.S, SEQDATA.S")
    s += f"\n/// Load address of the assembled table on the Apple II.\npub const ORG: u16 = 0x{org:04x};\n\n"
    s += "/// The assembled sequence table exactly as it sat in memory at ORG.\n"
    s += "/// Bytes 0..228 are the 114-entry jump table (`dw` absolute addresses).\n"
    s += rust_bytes("SEQ", out)
    s += "\n/// Sequence instructions (negative bytes in the table).\npub mod op {\n"
    for k, v in sorted(instr, key=lambda kv: -kv[1]):
        s += f"    pub const {k.upper()}: u8 = 0x{v & 0xFF:02x};\n"
    s += "}\n\n/// Sequence numbers from SEQDATA.S (argument to JUMPSEQ).\n#[allow(dead_code)]\npub mod seq {\n"
    for k, v in names:
        s += f"    pub const {re.sub(r'(?<!^)(?=[A-Z])', '_', k).upper()}: u8 = {v};\n"
    s += "}\n"
    return s


def gen_framedef(src):
    text = src.read("Source/FRAMEDEF.S")
    asm = Asm(text)
    code = asm.assemble()
    org = asm.org
    fdef = asm.syms["Fdef"] - org
    alt1 = asm.syms["ALTSET1"] - org
    alt2 = asm.syms["ALTSET2"] - org
    sw = asm.syms["SWORDTAB"] - org
    s = HEADER.format(src="FRAMEDEF.S")
    s += "\n/// Frame defs: Fimage, Fsword, Fdx, Fdy, Fcheck. Index = frame# - 1.\n"
    s += rust_bytes("FDEF", code[fdef:alt1])
    s += "\n/// Enemy frames 150-189 (chtable4), same layout.\n"
    s += rust_bytes("ALTSET1", code[alt1:alt2])
    s += "\n/// Princess and vizier frames, same layout.\n"
    s += rust_bytes("ALTSET2", code[alt2:sw])
    s += "\n/// Sword table: Image, DX, DY (images from chtable3).\n"
    s += rust_bytes("SWORDTAB", code[sw:sw + 192])
    return s


def main():
    ref = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_REF
    src = Source(ref)
    out_assets = os.path.join(ROOT, "assets", "apple2")
    os.makedirs(out_assets, exist_ok=True)
    count = 0
    for sub in ("Images", "Levels"):
        for name in src.listdir(sub):
            data = src.read(f"{sub}/{name}")
            with open(os.path.join(out_assets, name), "wb") as f:
                f.write(data)
            count += 1
    gen = os.path.join(ROOT, "src", "gen")
    os.makedirs(gen, exist_ok=True)
    with open(os.path.join(gen, "seqtable.rs"), "w") as f:
        f.write(gen_seqtable(src))
    with open(os.path.join(gen, "framedef.rs"), "w") as f:
        f.write(gen_framedef(src))
    print(f"copied {count} files to assets/apple2, wrote src/gen/seqtable.rs and framedef.rs")


if __name__ == "__main__":
    main()
