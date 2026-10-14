"""Gerçek LAME (ffmpeg libmp3lame, ortak stereo): bit hızına göre SNR, yan (S) kanal hatası ve
kodlayıcının üst kesim frekansı. Kullanım: python bit_hizi.py <çalışma klasörü>"""
import os
import subprocess
import sys

import numpy as np

from sinyal import RATE, stereo, write_wav


def decode(path):
    raw = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", path, "-f", "f32le", "-ac", "2", "-"], capture_output=True, check=True
    ).stdout
    return np.frombuffer(raw, np.float32).astype(np.float64).reshape(-1, 2)


def psd(x):
    frames = [np.abs(np.fft.rfft(x[i : i + 4096] * np.hanning(4096))) ** 2 for i in range(0, len(x) - 4096, 2048)]
    return np.mean(frames, axis=0)


def main(work):
    os.makedirs(work, exist_ok=True)
    st = stereo()
    src = os.path.join(work, "kaynak.wav")
    write_wav(src, st)
    f = np.fft.rfftfreq(4096, 1 / RATE)
    p0 = psd(st[:, 0])
    print("bit hızı | SNR | yan (S) kanal hata | üst kesim (çözülenin orijinalden 20 dB düştüğü ilk frekans)")
    for kbps in (96, 128, 192, 256, 320):
        mp3 = os.path.join(work, f"lame_{kbps}.mp3")
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", src, "-c:a", "libmp3lame", "-b:a", f"{kbps}k", mp3], check=True)
        y = decode(mp3)
        m = min(len(y), len(st))
        y, r = y[:m], st[:m]
        snr = 10 * np.log10(np.sum(r**2) / np.sum((y - r) ** 2))
        side = (y[:, 0] - y[:, 1]) - (r[:, 0] - r[:, 1])
        side_db = 10 * np.log10(np.sum(side**2) / np.sum((r[:, 0] - r[:, 1]) ** 2))
        ratio = np.convolve(10 * np.log10((psd(y[:, 0]) + 1e-30) / (p0 + 1e-30)), np.ones(9) / 9, mode="same")
        cut = np.where((f > 10_000) & (ratio < -20))[0]
        cut_txt = f"{f[cut[0]] / 1000:.1f} kHz" if len(cut) else "20 kHz'e kadar yok"
        print(f"{kbps:>3} kbps | {snr:5.1f} dB | {side_db:+5.1f} dB | {cut_txt}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "veritas-calisma")
