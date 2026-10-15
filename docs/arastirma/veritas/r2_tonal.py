"""R2 / S3: öğrenme olmadan tonal deliklerde ne kadar gidilebilir? Kâhin tavanları ve yinelemeli tonal izdüşüm.

Kâhin ayrıştırması: deliklerin (q = 0) hangi alt kümesi doldurulunca MOS ne kadar artıyor (canlı bant / sıfır bant,
5 kHz altı / üstü). Tonal tavan: GERÇEK sinyalin uzun STFT'de seçilen tepelerinden kurulan tonal parçası deliklere
yazılır (kutuya kırpılarak); hiçbir tonal yöntem bundan ileri gidemez. Tonal POCS: yalnızca çözücünün bilgisiyle,
tepe maskesi izdüşümü ile kutu izdüşümü (P_C) arasında yineleme (Seviye 2'nin kâhinsiz hâli).
Kullanım: VERITAS_LAME=… python r2_tonal.py <döküm klasörü> <ad> […]"""
import os
import sys

import numpy as np
from scipy.ndimage import maximum_filter, median_filter

import deney as D


def tonal_part(x, n_fft=8192, prom_db=12.0, halfw=2, max_peaks=None):
    """Uzun STFT'de belirgin tepeler (±halfw kutu) → tonal parça (zaman alanı). max_peaks verilirse kare başına en
    güçlü o kadar tepe alınır."""
    out = np.zeros_like(x)
    for ch in range(x.shape[1]):
        X = D.stft(x[:, ch], n_fft)
        mag = 20 * np.log10(np.abs(X) + 1e-12)
        med = median_filter(mag, size=(31, 1), mode="nearest")
        peak = (mag == maximum_filter(mag, size=(3, 1))) & (mag - med > prom_db) & (mag > mag.max() - 100)
        if max_peaks is not None:
            thr = -np.sort(-np.where(peak, mag, -np.inf), axis=0)[max_peaks - 1 : max_peaks]
            peak &= mag >= thr
        mask = maximum_filter(peak.astype(np.uint8), size=(2 * halfw + 1, 1)).astype(bool)
        out[:, ch] = D.istft(np.where(mask, X, 0), n_fft, x.shape[0])
    return out


def fill_holes_with(tr, pcm):
    """Verilen sesin katsayılarını yalnızca deliklere yaz, kutuya kırp (P_C'nin delik kısmı)."""
    z = tr.analyze(pcm)
    return np.where(tr.q == 0, np.clip(z, tr.lo, tr.hi), tr.r)


def tonal_pocs(tr, iters=8, prom_db=12.0, halfw=2, n_fft=8192):
    """Kâhinsiz: x₀ = çözücü; x ← P_C(tonal(x) + (x − tonal(x))·0) … yalnızca deliklerde tonal parça; canlı
    katsayılar kutularında kalır. Her yinelemede tepeler yeniden seçilir."""
    est = tr.r.copy()
    for _ in range(iters):
        x = tr.pcm(est)
        xt = tonal_part(x, n_fft, prom_db, halfw)
        est = fill_holes_with(tr, xt)
    return est


def main(folder, names):
    for name in names:
        tr = D.Track(os.path.join(folder, name + ".bin"))
        hole = tr.q == 0
        f = np.arange(576)[None, None, :] * tr.rate / 1152
        live_hole = hole & ~tr.zero_band
        zero_hole = hole & tr.zero_band
        truth_tonal = {p: tonal_part(tr.truth, prom_db=p) for p in (6.0, 12.0, 20.0)}
        methods = {
            "çözücü": tr.r,
            "kâhin: tüm delikler gerçek": np.where(hole, tr.theta, tr.r),
            "kâhin: yalnız canlı bant delikleri": np.where(live_hole, tr.theta, tr.r),
            "kâhin: yalnız sıfır bant delikleri": np.where(zero_hole, tr.theta, tr.r),
            "kâhin: delikler < 5 kHz": np.where(hole & (f < 5000), tr.theta, tr.r),
            "kâhin: delikler ≥ 5 kHz": np.where(hole & (f >= 5000), tr.theta, tr.r),
            "kâhin: delikler, yalnız işaret doğru |θ|=E": None,
            "kâhin tonal tavan (tepe 6 dB)": fill_holes_with(tr, truth_tonal[6.0]),
            "kâhin tonal tavan (tepe 12 dB)": fill_holes_with(tr, truth_tonal[12.0]),
            "kâhin tonal tavan (tepe 20 dB)": fill_holes_with(tr, truth_tonal[20.0]),
            "kâhin tonal tavan 12 dB, en çok 30 tepe": fill_holes_with(tr, tonal_part(tr.truth, max_peaks=30)),
            "kâhin: gerçek − tonal(12 dB) kalıntısı deliklere": fill_holes_with(tr, tr.truth - truth_tonal[12.0]),
            "tonal POCS ×1 (çözücü bilgisi)": tonal_pocs(tr, iters=1),
            "tonal POCS ×8 (çözücü bilgisi)": tonal_pocs(tr, iters=8),
            "tonal POCS ×8, tepe 6 dB": tonal_pocs(tr, iters=8, prom_db=6.0),
        }
        # işaret kâhini: deliklerde gerçek işaret, genlik bandın gerçek delik RMS'i (yapı mı, enerji mi?)
        ms = D.oracle_hole_energy(tr)
        methods["kâhin: delikler, yalnız işaret doğru |θ|=E"] = np.where(hole, np.sign(tr.theta) * np.sqrt(ms), tr.r)
        print(f"\n== {name} ({tr.rate} Hz): delik payı {hole.mean():.3f}, sıfır bant {zero_hole.mean():.3f}", flush=True)
        for label, est in methods.items():
            x = tr.pcm(est)
            outside = np.mean((est < tr.lo - 1e-12) | (est > tr.hi + 1e-12))
            print(f"  {label:46s} SNR {D.snr(tr.truth, x):6.2f} dB  MOS {D.mos(tr.truth, x, tr.rate):.3f}  kutu dışı {outside:.0e}", flush=True)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2:])
