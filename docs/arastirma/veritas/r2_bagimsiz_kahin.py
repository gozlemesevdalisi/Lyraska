"""Bağımsız denetim: kâhin ayrıştırması (raporun 8.2 tablosu) kendi kodumuzla + iki yeni satır:
gerçek genlik + rastgele işaret (genliğin ince yapısının değeri) ve bant RMS'i + rastgele işaret (R1'in 4,670'i).
Kullanım: VERITAS_LAME=… python -I r2_bagimsiz_kahin.py <döküm klasörü> <betik klasörü> <ad> …"""
import sys

import numpy as np

sys.path.insert(0, sys.argv[2])
import deney as D  # noqa: E402


def band_hole_rms(tr):
    hole = tr.q == 0
    out = np.zeros_like(tr.theta)
    for ch in range(2):
        for t in range(tr.g):
            for s in tr.bands(ch, t):
                h = hole[ch, t, s]
                if h.any():
                    v = out[ch, t, s]
                    v[h] = np.sqrt(np.mean(tr.theta[ch, t, s][h] ** 2))
                    out[ch, t, s] = v
    return out


def main(folder, names):
    rows = {}
    for name in names:
        tr = D.Track(f"{folder}/{name}.bin")
        rng = np.random.default_rng(7)
        hole = tr.q == 0
        rms = band_hole_rms(tr)
        rsign = rng.choice([-1.0, 1.0], size=tr.theta.shape)
        tsign = np.where(tr.theta >= 0, 1.0, -1.0)
        m = {
            "çözücü": tr.r,
            "tüm delikler gerçek": np.where(hole, tr.theta, tr.r),
            "canlı bant delikleri gerçek": np.where(hole & ~tr.zero_band, tr.theta, tr.r),
            "sıfır bant delikleri gerçek": np.where(hole & tr.zero_band, tr.theta, tr.r),
            "işaret gerçek, genlik bant RMS": np.where(hole, tsign * rms, tr.r),
            "işaret rastgele, genlik bant RMS": np.where(hole, rsign * rms, tr.r),
            "genlik gerçek, işaret rastgele": np.where(hole, rsign * np.abs(tr.theta), tr.r),
        }
        print(f"\n== {name}", flush=True)
        for k, est in m.items():
            assert np.all(est >= tr.lo - 1e-9) and np.all(est <= tr.hi + 1e-9), k
            x = tr.pcm(est)
            v = D.mos(tr.truth, x, tr.rate)
            rows.setdefault(k, []).append(v)
            print(f"  {k:36s} MOS {v:.3f}  SNR {D.snr(tr.truth, x):6.2f}", flush=True)
    print("\nOrtalama:")
    for k, v in rows.items():
        print(f"  {k:36s} {np.mean(v):.3f}  ({' '.join(f'{x:.3f}' for x in v)})")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[3:])
