"""Müziğe benzer sentetik deney sesi (44,1 kHz): piyano benzeri notalar, titreşimli yaylı katman,
davul (kick) ve zil (hi-hat). Çalışma anında üretilir; depoya ses dosyası girmez."""
import numpy as np

RATE = 44_100


def mono(seconds=12.0, seed=7):
    rng = np.random.default_rng(seed)
    n = int(seconds * RATE)
    t = np.arange(n) / RATE
    x = np.zeros(n)
    # Piyano benzeri: her 0,5 sn bir nota; 1/k genlikli, hafif uyumsuz kısmi sesler 12 kHz'e kadar.
    notes = [220, 262, 330, 392, 440, 523, 392, 330]
    for i, start in enumerate(np.arange(0, seconds - 1, 0.5)):
        f0 = notes[i % len(notes)] * (1 + 0.0005 * i)
        s = int(start * RATE)
        tt = t[: n - s]
        env = np.exp(-tt * 2.5) * (1 - np.exp(-tt * 400))
        note = np.zeros_like(tt)
        k = 1
        while f0 * k < 12_000:
            partial = f0 * k * np.sqrt(1 + 0.0004 * k * k)
            note += np.sin(2 * np.pi * partial * tt + rng.uniform(0, 2 * np.pi)) / k * np.exp(-tt * 0.6 * k)
            k += 1
        x[s:] += 0.25 * env * note
    # Yaylı katman: titreşimli (vibrato) testere dalgası.
    vib = 1 + 0.004 * np.sin(2 * np.pi * 5.5 * t)
    pad = sum(np.sin(2 * np.pi * 110 * k * np.cumsum(vib) / RATE) / k for k in range(1, 30))
    x += 0.05 * pad * (0.6 + 0.4 * np.sin(2 * np.pi * 0.1 * t))
    # Kick: her 0,5 sn, perdesi düşen sinüs.
    for start in np.arange(0, seconds, 0.5):
        s = int(start * RATE)
        tt = t[: min(n - s, int(0.3 * RATE))]
        freq = 50 + 100 * np.exp(-tt * 30)
        x[s : s + len(tt)] += 0.6 * np.sin(2 * np.pi * np.cumsum(freq) / RATE) * np.exp(-tt * 12)
    # Hi-hat: her 0,25 sn, 6–20 kHz gürültü patlaması.
    for start in np.arange(0.125, seconds, 0.25):
        s = int(start * RATE)
        m = min(n - s, int(0.08 * RATE))
        spec = np.fft.rfft(rng.standard_normal(m))
        f = np.fft.rfftfreq(m, 1 / RATE)
        spec[(f < 6000) | (f > 20000)] = 0
        burst = np.fft.irfft(spec, m)
        x[s : s + m] += 0.15 * burst / np.max(np.abs(burst)) * np.exp(-np.arange(m) / RATE * 40)
    return x / np.max(np.abs(x)) * 10 ** (-1 / 20)  # tepe −1 dBFS


def stereo(seconds=12.0):
    """Sol ve sağ kısmen farklı (gerçek bir miks gibi): orta + ±0,35 yan kanal."""
    a, b = mono(seconds, seed=7), mono(seconds, seed=11)
    st = np.stack([a + 0.35 * b, a - 0.35 * b], axis=1)
    return st / np.max(np.abs(st)) * 10 ** (-1 / 20)


def write_wav(path, x, bits=32):
    import scipy.io.wavfile as wav

    if bits == 16:
        wav.write(path, RATE, np.round(np.clip(x, -1, 1) * 32767).astype(np.int16))
    else:
        wav.write(path, RATE, x.astype(np.float32))
