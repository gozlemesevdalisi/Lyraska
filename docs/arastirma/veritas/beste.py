"""Deney müziği: programla yazılmış MIDI parçaları, FluidSynth + FluidR3_GM (MIT) ile çalınır.

Her parça ~20 sn, farklı bir zorluk: pop (davul + bas + piyano + yaylılar + melodi), akustik (gitar arpej,
keman, çello), piyano (geniş dinamik, pedal), elektronik (testere dalgası, hızlı zil), orkestra (yaylı, bakır,
timpani, arp), vurmalı (kastanyet, klavsen: MP3'ün bilinen zor örnekleri)."""
import os
import subprocess
import sys

import mido
import numpy as np
import scipy.io.wavfile as wav

SF2 = "/usr/share/sounds/sf2/FluidR3_GM.sf2"
BPM = 112
TPB = 480


def beats(b):
    return int(round(b * TPB))


class Song:
    def __init__(self):
        self.events = []  # (zaman_tik, öncelik, mesaj)

    def program(self, ch, prog):
        self.events.append((0, 0, mido.Message("program_change", channel=ch, program=prog)))

    def cc(self, ch, num, val, at=0.0):
        self.events.append((beats(at), 0, mido.Message("control_change", channel=ch, control=num, value=val)))

    def note(self, ch, pitch, start, dur, vel=90):
        self.events.append((beats(start), 2, mido.Message("note_on", channel=ch, note=pitch, velocity=vel)))
        self.events.append((beats(start + dur), 1, mido.Message("note_off", channel=ch, note=pitch, velocity=0)))

    def bend(self, ch, value, at):
        self.events.append((beats(at), 0, mido.Message("pitchwheel", channel=ch, pitch=int(value))))

    def save(self, path):
        mid = mido.MidiFile(ticks_per_beat=TPB)
        tr = mido.MidiTrack()
        mid.tracks.append(tr)
        tr.append(mido.MetaMessage("set_tempo", tempo=mido.bpm2tempo(BPM)))
        t = 0
        for at, _, msg in sorted(self.events, key=lambda e: (e[0], e[1])):
            tr.append(msg.copy(time=at - t))
            t = at
        mid.save(path)


CHORDS = [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]]  # Am F C G
SCALE = [57, 59, 60, 62, 64, 65, 67, 69, 71, 72]


def drums(s, bars, hats=8, fill=True):
    for bar in range(bars):
        b0 = bar * 4
        for k in (0, 2.5):
            s.note(9, 36, b0 + k, 0.25, 110)  # kick
        s.note(9, 36, b0 + 2, 0.25, 100)
        for k in (1, 3):
            s.note(9, 38, b0 + k, 0.25, 105)  # snare
        for h in range(hats):
            s.note(9, 42, b0 + h * 4 / hats, 0.1, 70 + 20 * (h % 2 == 0))
        if bar % 4 == 0:
            s.note(9, 49, b0, 1.0, 100)  # crash
        if fill and bar % 4 == 3:
            for i, p in enumerate((50, 48, 47, 45)):
                s.note(9, p, b0 + 3 + i * 0.25, 0.25, 100)


def melody(s, ch, bars, rng, octave=12, vel=95, step=0.5):
    t = 0.0
    while t < bars * 4:
        d = rng.choice([step, step, 2 * step, 3 * step])
        s.note(ch, int(rng.choice(SCALE)) + octave, t, d * 0.95, vel)
        t += d


def pop(rng):
    s = Song()
    bars = 10
    for ch, prog in ((0, 33), (1, 0), (2, 48), (3, 73)):
        s.program(ch, prog)
    drums(s, bars)
    for bar in range(bars):
        c = CHORDS[bar % 4]
        for k in range(8):
            s.note(0, c[0] - 24 + (12 if k % 4 == 2 else 0), bar * 4 + k * 0.5, 0.45, 100)
        for k in (0, 1.5, 2, 3.5):
            for p in c:
                s.note(1, p, bar * 4 + k, 0.4, 80)
        for p in c:
            s.note(2, p + 12, bar * 4, 4, 60)
    melody(s, 3, bars, rng)
    return s


def akustik(rng):
    s = Song()
    bars = 10
    for ch, prog in ((0, 24), (1, 40), (2, 42)):
        s.program(ch, prog)
    for bar in range(bars):
        c = CHORDS[bar % 4]
        pat = [c[0] - 12, c[1], c[2], c[1] + 12, c[2], c[1]]
        for k in range(12):
            s.note(0, pat[k % 6], bar * 4 + k / 3, 0.6, 75 + rng.integers(0, 20))
        s.note(2, c[0] - 24, bar * 4, 3.9, 70)
    t = 0.0
    while t < bars * 4:  # keman, titreşimli (pitch bend ile vibrato)
        d = float(rng.choice([1.0, 1.5, 2.0]))
        p = int(rng.choice(SCALE)) + 12
        s.note(1, p, t, d * 0.97, 85)
        for j in range(int(d * 12)):
            s.bend(1, 300 * np.sin(2 * np.pi * 5.5 * j / 12 * 60 / BPM), t + j / 12)
        t += d
    return s


def piyano(rng):
    s = Song()
    bars = 10
    s.program(0, 0)
    for bar in range(bars):
        c = CHORDS[bar % 4]
        s.cc(0, 64, 127, bar * 4)
        s.cc(0, 64, 0, bar * 4 + 3.9)
        vel = int(40 + 80 * (0.5 + 0.5 * np.sin(bar / bars * np.pi)))  # pp → ff → pp
        for k in range(8):
            s.note(0, [c[0] - 12, c[1], c[2], c[1] + 12][k % 4], bar * 4 + k * 0.5, 0.5, vel)
        if bar % 2:
            for p in c:
                s.note(0, p + 24, bar * 4 + 2, 1.5, min(127, vel + 10))
    melody(s, 0, bars, rng, octave=12, vel=90, step=0.5)
    return s


def elektronik(rng):
    s = Song()
    bars = 10
    for ch, prog in ((0, 38), (1, 81), (2, 89)):
        s.program(ch, prog)
    for bar in range(bars):
        b0 = bar * 4
        for k in range(4):
            s.note(9, 36, b0 + k, 0.2, 120)
        for h in range(16):
            s.note(9, 42 if h % 4 else 46, b0 + h / 4, 0.08, 60 + 30 * (h % 2))
        s.note(9, 39, b0 + 1, 0.2, 100)
        s.note(9, 39, b0 + 3, 0.2, 100)
        c = CHORDS[bar % 4]
        for k in range(16):
            s.note(0, c[0] - 24 + (12 if k % 2 else 0), b0 + k / 4, 0.22, 110)
        for p in c:
            s.note(2, p, b0, 4, 70)
    melody(s, 1, bars, rng, octave=12, vel=100, step=0.25)
    return s


def orkestra(rng):
    s = Song()
    bars = 10
    for ch, prog in ((0, 48), (1, 61), (2, 47), (3, 46), (4, 52)):
        s.program(ch, prog)
    for bar in range(bars):
        c = CHORDS[bar % 4]
        for p in c:
            s.note(0, p, bar * 4, 4, 80)
            s.note(0, p - 12, bar * 4, 4, 70)
        if bar % 2 == 1:
            for p in c:
                s.note(1, p, bar * 4 + 2, 1.8, 95)
        s.note(2, c[0] - 24, bar * 4, 0.5, 110)
        s.note(2, c[0] - 24, bar * 4 + 3.5, 0.25, 90)
        for k in range(8):
            s.note(3, c[k % 3] + 12 * (1 + k // 3), bar * 4 + k * 0.125, 0.5, 80)
        s.note(4, c[1] + 12, bar * 4, 4, 60)
    return s


def vurmali(rng):
    s = Song()
    bars = 10
    s.program(0, 6)  # klavsen
    for bar in range(bars):
        c = CHORDS[bar % 4]
        for k in range(16):
            s.note(0, [c[0], c[1], c[2], c[1] + 12][k % 4] + 12, bar * 4 + k / 4, 0.24, 100)
        for k in range(8):  # kastanyet (GM vurmalı 85) ve klave
            if rng.random() < 0.8:
                s.note(9, 85, bar * 4 + k / 2 + (0.25 if k % 3 == 2 else 0), 0.1, 110)
        s.note(9, 75, bar * 4 + 1.5, 0.1, 100)
    return s


PIECES = {"pop": pop, "akustik": akustik, "piyano": piyano, "elektronik": elektronik, "orkestra": orkestra, "vurmali": vurmali}


def render(name, out_dir):
    rng = np.random.default_rng(sum(map(ord, name)))
    mid = os.path.join(out_dir, f"{name}.mid")
    raw = os.path.join(out_dir, f"{name}_ham.wav")
    PIECES[name](rng).save(mid)
    subprocess.run(["fluidsynth", "-ni", "-q", "-g", "0.5", "-r", "44100", "-F", raw, SF2, mid], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    rate, x = wav.read(raw)
    x = x.astype(np.float64)
    x /= np.max(np.abs(x)) + 1e-12
    x *= 10 ** (-1 / 20)
    x = x[: int(20 * rate)]
    wav.write(os.path.join(out_dir, f"{name}.wav"), rate, np.round(x * 32767).astype(np.int16))
    os.remove(raw)


if __name__ == "__main__":
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for name in sys.argv[2:] or PIECES:
        render(name, out)
        print(name, "tamam")
