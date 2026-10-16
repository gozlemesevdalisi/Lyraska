"""Bağımsız denetim: deliklerin İŞARETİ komşu granüllerin gerçek değerlerinden (doğrusal olmayan) kestirilebilir mi?
Bin başına kNN sınıflandırıcı (özellik: v_{t±1,2}(k), bant RMS'iyle ölçekli; ayrıca |v| ve işaret çarpımları),
çift/tek granül çapraz geçerleme. Bilgi alt sınırı: 1 − H₂(hata) bit. Olumlu denetim: q ≠ 0 katsayılar < 2 kHz.
Kullanım: VERITAS_LAME=… python -I r2_bagimsiz_isaret.py <döküm klasörü> <betik klasörü> <ad> …"""
import sys

import numpy as np
from scipy.spatial import cKDTree

sys.path.insert(0, sys.argv[2])
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import deney as D  # noqa: E402
from r2_bagimsiz_komsu import band_rms, collect, windows  # noqa: E402


def h2(p):
    p = np.clip(p, 1e-9, 1 - 1e-9)
    return -(p * np.log2(p) + (1 - p) * np.log2(1 - p))


def sign_acc(a, kn=15):
    y = a[:, 0] > 0
    X = np.arcsinh(a[:, 1:5] * 3)
    k, t = a[:, 5].astype(int), a[:, 6].astype(int)
    correct = total = 0
    for kk in np.unique(k):
        idx = np.nonzero(k == kk)[0]
        tr_i, te_i = idx[t[idx] % 2 == 0], idx[t[idx] % 2 == 1]
        if len(tr_i) < 40 or len(te_i) < 10:
            continue
        _, nb = cKDTree(X[tr_i]).query(X[te_i], k=min(kn, len(tr_i)))
        pred = y[tr_i][nb].mean(1) > 0.5
        correct += np.sum(pred == y[te_i])
        total += len(te_i)
    acc = correct / max(total, 1)
    return acc, total


def main(folder, names):
    for name in names:
        tr = D.Track(f"{folder}/{name}.bin")
        hole = tr.q == 0
        f = np.arange(576)[None, None, :] * tr.rate / 1152
        rms_h, rms_c = band_rms(tr, hole), band_rms(tr, tr.q != 0)
        ts = windows(tr)
        print(f"== {name}", flush=True)
        for label, sel, rms in (("delik, canlı bant", hole & ~tr.zero_band, rms_h), ("delik, sıfır bant", hole & tr.zero_band, rms_h),
                                ("delik, < 5 kHz", hole & (f < 5000), rms_h), ("OLUMLU: q≠0, < 2 kHz", (tr.q != 0) & (f < 2000), rms_c)):
            a = np.vstack([collect(tr, sel, rms, ts, ch) for ch in range(2)])
            acc, n = sign_acc(a)
            print(f"  {label:22s} işaret isabeti {acc:.3f} (sınama {n})  → en az {max(0, 1 - h2(1 - acc)):.3f} bit/katsayı", flush=True)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[3:])
