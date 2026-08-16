#!/usr/bin/env python3
"""Paint text into a WAV's spectrogram (pure stdlib).

Each column of a 5x7 bitmap glyph becomes a short time slice; each lit row
becomes a sine partial at that row's frequency. Run the result through
sgram-tui and the text appears in the time-frequency plane:

    python3 scripts/spell.py SGRAM docs/assets/sgram-spell.wav
    sgram-tui render docs/assets/sgram-spell.wav --png-path hero.png

This is how the README's hero image was made — the image IS audio.
"""

import math
import struct
import sys
import wave

RATE = 48000
COL_SECS = 0.12          # one glyph column per slice
F_LOW, F_STEP = 700.0, 200.0   # row 6 (bottom) at F_LOW, rising per row

# 5x7 font, one string per glyph row (top row first)
FONT = {
    "S": ["#####", "#....", "#....", "#####", "....#", "....#", "#####"],
    "G": ["#####", "#....", "#....", "#.###", "#...#", "#...#", "#####"],
    "R": ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
    "A": [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
    "M": ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"],
    "T": ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
    "U": ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", "#####"],
    "I": ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"],
    "-": [".....", ".....", ".....", "#####", ".....", ".....", "....."],
    " ": [".....", ".....", ".....", ".....", ".....", ".....", "....."],
}


def columns(text):
    """Yield glyph columns (list of 7 bools, top first), 1 blank between."""
    for ch in text.upper():
        glyph = FONT.get(ch, FONT[" "])
        for c in range(5):
            yield [glyph[r][c] == "#" for r in range(7)]
        yield [False] * 7


def synth(text):
    """One continuous tone per horizontal stroke-run, so bars render solid
    (fading every column would chop strokes into dashes)."""
    cols = list(columns(text))
    n_col = int(RATE * COL_SECS)
    total = len(cols) * n_col
    out = [0.0] * total
    for r in range(7):
        f = F_LOW + F_STEP * (6 - r)
        ci = 0
        while ci < len(cols):
            if not cols[ci][r]:
                ci += 1
                continue
            run = ci
            while ci < len(cols) and cols[ci][r]:
                ci += 1
            start, length = run * n_col, (ci - run) * n_col
            for n in range(length):
                t = (start + n) / RATE
                edge = min(n, length - 1 - n) / (0.010 * RATE)
                env = 0.5 - 0.5 * math.cos(math.pi * min(edge, 1.0))
                out[start + n] += 0.22 * env * math.sin(2 * math.pi * f * t)
    peak = max(abs(x) for x in out) or 1.0
    return [x * 0.85 / peak for x in out]


def main():
    text = sys.argv[1] if len(sys.argv) > 1 else "SGRAM"
    path = sys.argv[2] if len(sys.argv) > 2 else "spell.wav"
    samples = synth(text)
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(
            struct.pack("<h", int(max(-1, min(1, x)) * 32767))
            for x in samples))
    print(f"wrote {path}: '{text}', {len(samples)/RATE:.2f}s")


if __name__ == "__main__":
    main()
