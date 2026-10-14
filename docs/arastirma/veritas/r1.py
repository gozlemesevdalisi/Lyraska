"""Veritas R1 ölçümleri: çözücü doğrulaması, yöntem karşılaştırması ve kâhin tavanları.

Kullanım: VERITAS_LAME=<lame-3.100 klasörü> python r1.py <döküm klasörü> [parça_bithızı …]
Döküm klasöründe her ad için <ad>.bin (yamalı LAME dökümü) ve <ad>.mp3 bulunur."""
import os
import subprocess
import sys

import numpy as np

import deney as D


def ffmpeg_pcm(mp3):
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", mp3, "-f", "f32le", "-ac", "2", "-"], capture_output=True, check=True).stdout
    return np.frombuffer(raw, np.float32).astype(np.float64).reshape(-1, 2)


def decoder_check(tr, mp3):
    """Kendi çözümümüz ile ffmpeg: LAME gecikmesi (1105 örnek) hizalanır, fark dB."""
    ref = ffmpeg_pcm(mp3)
    ours = tr.pcm(tr.r)
    lag = 1105
    m = min(len(ref), len(ours) - lag)
    e = ref[2000:m - 2000] - ours[lag + 2000:lag + m - 2000]
    return 10 * np.log10(np.sum(e**2) / np.sum(ref[2000:m - 2000] ** 2))


def oracle_wiener(tr, n_fft=4096):
    """Tavan: gerçek sinyalin ve hatanın STFT gücü bilinirse Wiener + kutuya izdüşüm."""
    x0 = tr.pcm(tr.r)
    L = x0.shape[0]
    X = np.stack([D.stft(x0[:, c], n_fft) for c in range(2)])
    T = np.stack([D.stft(tr.truth[:, c], n_fft) for c in range(2)])
    E = np.stack([D.stft((x0 - tr.truth)[:, c], n_fft) for c in range(2)])
    G = np.abs(T) ** 2 / (np.abs(T) ** 2 + np.abs(E) ** 2 + 1e-30)
    x1 = np.stack([D.istft(G[c] * X[c], n_fft, L) for c in range(2)], 1)
    return np.clip(tr.analyze(x1), tr.lo, tr.hi)


def main(folder, names):
    rows = []
    for name in names:
        tr = D.Track(os.path.join(folder, name + ".bin"))
        hole = tr.q == 0
        f = np.arange(576)[None, None, :] * tr.rate / 1152
        wf = D.wiener_fusion(tr)
        methods = {
            "çözücü": tr.r,
            "Laplace ağırlık merkezi": D.laplace_mmse(tr),
            "Wiener + kesik Gauss": wf,
            "Wiener + kesik Gauss + enerji": D.energy_match(tr, wf),
            "faz-tutarlı tonal delik doldurma": D.tonal_fill(tr),
            "bit akışından enerjiyle gürültü": D.noise_fill(tr, D.hole_energy(tr)),
            "kâhin: Wiener (gerçek güç)": oracle_wiener(tr),
            "kâhin: delik enerjisiyle gürültü": D.noise_fill(tr, D.oracle_hole_energy(tr)),
            "kâhin: q=0 gerçek, ≥5 kHz": np.where(hole & (f >= 5000), tr.theta, tr.r),
            "kâhin: q=0 gerçek (tümü)": np.where(hole, tr.theta, tr.r),
            "kâhin: q≠0 gerçek (tümü)": np.where(~hole, tr.theta, tr.r),
        }
        check = decoder_check(tr, os.path.join(folder, name + ".mp3"))
        print(f"\n== {name} ({tr.rate} Hz, {tr.g} granül): çözücümüz − ffmpeg = {check:.1f} dB", flush=True)
        for label, est in methods.items():
            x = tr.pcm(est)
            outside = np.mean((est < tr.lo - 1e-12) | (est > tr.hi + 1e-12))
            line = f"  {label:34s} SNR {D.snr(tr.truth, x):6.2f} dB  MOS {D.mos(tr.truth, x, tr.rate):.3f}"
            print(line + ("" if label.startswith("kâhin") else f"  kutu dışı {outside:.0e}"), flush=True)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2:])
