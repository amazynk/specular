#!/usr/bin/env python3
"""MIDI oracle for the fmt500 corpus: renders a song with TiMidity++ through a SoundFont and
with Specular's synthesiser (the ignored `render_midi_for_the_oracle` test), then compares
their loudness envelopes (50 ms RMS, correlation) and lengths. A synthesiser is not
sample-exact with another, so the envelope is what is compared.

usage: fmt500-midi-oracle.py SONG.mid FONT.sf2 OUT_DIR SPECULAR_RAW
  SPECULAR_RAW: the 44.1 kHz stereo s16le our test wrote (FMT500_OUT).
"""
import subprocess
import sys

import numpy as np


def envelope(x, rate=44100, win=0.05):
    n = int(rate * win)
    m = len(x) // n
    x = x[: m * n].reshape(m, n)
    return np.sqrt((x.astype(np.float64) ** 2).mean(axis=1))


def main():
    song, font, out, ours_raw = sys.argv[1:5]
    cfg = f"{out}/timidity.cfg"
    with open(cfg, "w") as f:
        f.write(f"soundfont {font}\n")
    wav = f"{out}/timidity.raw"
    subprocess.run(
        ["timidity", "-c", cfg, "-Or1sl", "-s", "44100", "-o", wav, song],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    t = np.fromfile(wav, dtype="<i2").reshape(-1, 2).mean(axis=1)
    o = np.fromfile(ours_raw, dtype="<i2").reshape(-1, 2).mean(axis=1)
    et, eo = envelope(t), envelope(o)
    # Lengths: where the sound falls below -60 dB of its peak for good.
    def end(e):
        thr = e.max() * 1e-3
        idx = np.nonzero(e > thr)[0]
        return (idx[-1] + 1) * 0.05 if len(idx) else 0.0

    n = min(len(et), len(eo))
    corr = np.corrcoef(et[:n], eo[:n])[0, 1]
    # Note onsets: the envelope's rises.
    print(f"timidity end {end(et):.2f} s, ours {end(eo):.2f} s, envelope correlation {corr:.3f}")


if __name__ == "__main__":
    main()
