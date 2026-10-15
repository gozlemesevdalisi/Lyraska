"""R2 eğitim ve sınama müziği: rastgele programla yazılmış parçalar, FluidSynth + FluidR3_GM (MIT) ile çalınır.

`beste.py`'nin genişletilmiş hâli: her parçada ton, tempo, akor dizisi, çalgılar (GM programları), ritim
kalıpları, davul, pedal/yankı ve ses yüksekliği tohumdan rastgele seçilir. R1'in altı parçası burada ÜRETİLMEZ;
onlar yalnızca sınamada kullanılır. Kullanım: python r2_beste.py <klasör> <tohum_başı> <adet> [süre_sn]."""
import os
import subprocess
import sys

import mido
import numpy as np
import scipy.io.wavfile as wav

from beste import SF2, Song

# GM çalgı aileleri (program numaraları); her parçada ailelerden rastgele seçilir.
MELODIC = list(range(0, 8)) + list(range(8, 16)) + list(range(16, 24)) + list(range(24, 32)) + [40, 41, 42, 43, 45, 46, 48, 49, 50, 51, 52, 56, 57, 58, 60, 61, 64, 65, 66, 67, 68, 71, 72, 73, 74, 75, 79, 80, 81, 82, 85, 88, 89, 90, 95, 104, 105, 106, 108, 109]
BASS = [32, 33, 34, 35, 36, 38, 39, 43, 58]
PAD = [48, 49, 50, 52, 54, 88, 89, 90, 91, 92, 94, 95]
MODES = {"major": [0, 2, 4, 5, 7, 9, 11], "minor": [0, 2, 3, 5, 7, 8, 10], "dorian": [0, 2, 3, 5, 7, 9, 10], "pent": [0, 2, 4, 7, 9]}
PROGRESSIONS = [[0, 5, 3, 4], [0, 3, 4, 4], [5, 3, 0, 4], [0, 4, 5, 3], [0, 6, 3, 4], [0, 0, 3, 4], [0, 2, 3, 0]]


def chord(root, mode, degree):
    sc = MODES[mode]
    n = len(sc)
    return [root + sc[(degree + i) % n] + 12 * ((degree + i) // n) for i in (0, 2, 4)]


def compose(rng, seconds):
    s = Song()
    bpm = int(rng.integers(66, 170))
    s.bpm = bpm
    bars = int(np.ceil(seconds * bpm / 60 / 4)) + 1
    root = int(rng.integers(45, 57))
    mode = str(rng.choice(list(MODES)))
    prog = PROGRESSIONS[int(rng.integers(len(PROGRESSIONS)))]
    n_parts = int(rng.integers(2, 6))
    parts = ["lead"] + list(rng.choice(["chords", "bass", "pad", "arp", "counter"], n_parts - 1, replace=False))
    use_drums = rng.random() < 0.6
    scale_abs = [root + 12 + x for x in MODES[mode]] + [root + 24 + x for x in MODES[mode]]
    ch = 0
    for part in parts:
        if part == "lead":
            s.program(ch, int(rng.choice(MELODIC)))
            s.cc(ch, 10, int(rng.integers(30, 98)))
            t, step = 0.0, float(rng.choice([0.25, 0.5, 0.5, 1.0]))
            vel0 = int(rng.integers(70, 110))
            while t < bars * 4:
                d = step * int(rng.choice([1, 1, 2, 3, 4]))
                p = int(rng.choice(scale_abs)) + int(rng.choice([0, 0, 12]))
                s.note(ch, p, t, d * float(rng.uniform(0.6, 1.0)), int(np.clip(vel0 + rng.integers(-20, 20), 30, 127)))
                if rng.random() < 0.15:  # vibrato
                    for j in range(int(d * 12)):
                        s.bend(ch, 250 * np.sin(2 * np.pi * 5.5 * j / 12 * 60 / bpm), t + j / 12)
                    s.bend(ch, 0, t + d)
                t += d
        elif part == "chords":
            s.program(ch, int(rng.choice(MELODIC[:32] + [24, 25, 26, 27])))
            s.cc(ch, 10, int(rng.integers(30, 98)))
            rhythm = [list(r) for r in ([0, 1, 2, 3], [0, 1.5, 2, 3.5], [0, 2], [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5], [0.5, 1.5, 2.5, 3.5])]
            rh = rhythm[int(rng.integers(len(rhythm)))]
            vel = int(rng.integers(55, 100))
            pedal = rng.random() < 0.4
            for bar in range(bars):
                c = chord(root, mode, prog[bar % 4])
                if pedal:
                    s.cc(ch, 64, 127, bar * 4)
                    s.cc(ch, 64, 0, bar * 4 + 3.9)
                for k in rh:
                    for p in c:
                        s.note(ch, p, bar * 4 + k, float(rng.uniform(0.3, 1.0)) if len(rh) > 2 else 1.8, vel)
        elif part == "bass":
            s.program(ch, int(rng.choice(BASS)))
            pat = int(rng.integers(3))
            for bar in range(bars):
                c = chord(root, mode, prog[bar % 4])
                if pat == 0:
                    for k in range(8):
                        s.note(ch, c[0] - 24 + (12 if k % 4 == 2 else 0), bar * 4 + k * 0.5, 0.45, 100)
                elif pat == 1:
                    s.note(ch, c[0] - 24, bar * 4, 2.0, 100)
                    s.note(ch, c[2] - 24, bar * 4 + 2, 1.5, 90)
                else:
                    for k in range(16):
                        s.note(ch, c[0] - 24 + (12 if k % 2 else 0), bar * 4 + k / 4, 0.22, 105)
        elif part == "pad":
            s.program(ch, int(rng.choice(PAD)))
            for bar in range(bars):
                for p in chord(root, mode, prog[bar % 4]):
                    s.note(ch, p + int(rng.choice([0, 12])), bar * 4, 4, int(rng.integers(45, 75)))
        elif part == "arp":
            s.program(ch, int(rng.choice([46, 6, 7, 11, 24, 25, 104, 108, 80, 81, 11])))
            s.cc(ch, 10, int(rng.integers(20, 108)))
            div = int(rng.choice([8, 12, 16]))
            for bar in range(bars):
                c = chord(root, mode, prog[bar % 4])
                pat = [c[0], c[1], c[2], c[1] + 12, c[2] + 12, c[0] + 12]
                for k in range(div):
                    s.note(ch, pat[k % len(pat)], bar * 4 + 4 * k / div, 4 / div * 0.9, int(rng.integers(60, 100)))
        elif part == "counter":
            s.program(ch, int(rng.choice(MELODIC)))
            s.cc(ch, 10, int(rng.integers(20, 108)))
            t = 0.0
            while t < bars * 4:
                d = float(rng.choice([1.0, 1.5, 2.0, 3.0]))
                s.note(ch, int(rng.choice(scale_abs)) - 12, t, d * 0.95, int(rng.integers(55, 95)))
                t += d
        ch += 1
        if ch == 9:
            ch = 10
    if use_drums:
        style = int(rng.integers(3))
        hats = int(rng.choice([4, 8, 16]))
        kit = int(rng.choice([0, 0, 8, 16, 24, 25, 32, 40]))
        s.program(9, kit)
        for bar in range(bars):
            b0 = bar * 4
            if style == 0:
                for k in (0, 2.5):
                    s.note(9, 36, b0 + k, 0.25, 110)
                s.note(9, 36, b0 + 2, 0.25, 100)
                for k in (1, 3):
                    s.note(9, 38, b0 + k, 0.25, 105)
            elif style == 1:
                for k in range(4):
                    s.note(9, 36, b0 + k, 0.2, 120)
                s.note(9, 39, b0 + 1, 0.2, 100)
                s.note(9, 39, b0 + 3, 0.2, 100)
            else:
                for k in (0, 1.5, 3):
                    s.note(9, 36, b0 + k, 0.25, 100)
                for k in (1, 2.5):
                    s.note(9, 40, b0 + k, 0.25, 95)
                s.note(9, 85 if rng.random() < 0.5 else 75, b0 + 3.5, 0.1, 100)
            for h in range(hats):
                s.note(9, 42 if h % 4 or rng.random() < 0.7 else 46, b0 + h * 4 / hats, 0.1, 60 + 25 * (h % 2 == 0))
            if bar % 4 == 0 and rng.random() < 0.6:
                s.note(9, 49, b0, 1.0, 100)
    return s


def render(name, out_dir, seed, seconds):
    rng = np.random.default_rng(seed)
    song = compose(rng, seconds)
    mid = os.path.join(out_dir, f"{name}.mid")
    raw = os.path.join(out_dir, f"{name}_ham.wav")
    import beste
    old = beste.BPM
    beste.BPM = song.bpm  # Song.save tempoyu modül değişkeninden okur
    song.save(mid)
    beste.BPM = old
    reverb = float(rng.uniform(0.0, 0.6))
    chorus = rng.random() < 0.3
    subprocess.run(
        ["fluidsynth", "-ni", "-q", "-g", "0.5", "-r", "44100", "-F", raw,
         "-o", f"synth.reverb.room-size={reverb:.2f}", "-o", f"synth.chorus.active={'1' if chorus else '0'}", SF2, mid],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    rate, x = wav.read(raw)
    x = x.astype(np.float64)
    x /= np.max(np.abs(x)) + 1e-12
    x *= 10 ** (-float(rng.uniform(0.5, 6.0)) / 20)
    x = x[: int(seconds * rate)]
    wav.write(os.path.join(out_dir, f"{name}.wav"), rate, np.round(x * 32767).astype(np.int16))
    os.remove(raw)
    os.remove(mid)


if __name__ == "__main__":
    out, seed0, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    seconds = float(sys.argv[4]) if len(sys.argv) > 4 else 12.0
    os.makedirs(out, exist_ok=True)
    for i in range(count):
        name = f"r2_{seed0 + i:04d}"
        render(name, out, 1000 + seed0 + i, seconds)
        print(name, "tamam", flush=True)
