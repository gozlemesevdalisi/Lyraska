"""Bağımsız denetim: kâhin bağlamın ÇÖZÜCÜ BAĞLAMINA EK bilgisi, u = θ/s biriminde (raporun bit ölçüsüyle aynı birim).

X = merkez delik u_t(k); Y = komşu granüllerin GERÇEK u_{t±1}(k); Z = çözücünün gördüğü: komşuların r/s'i (t±1),
merkezin log s'i. Her değişken ayrı ayrı sıralamaya çevrilir (karşılıklı bilgi bundan etkilenmez; KSG'yi
sağlamlaştırır). Zincir kuralı: I(X;Y|Z) = I(X;Y,Z) − I(X;Z), ikisi de KSG (k = 4, maks-norm).
Ayrıca seviye: bandın delik log-RMS'i, çözücü öznitelikleri ile / + komşuların gerçek delik log-RMS'i (kNN regresyon).
Kullanım: VERITAS_LAME=… python -I r2_bagimsiz_kosullu.py <döküm klasörü> <betik klasörü> <ad> …"""
import sys

import numpy as np
from scipy.spatial import cKDTree
from scipy.special import digamma
from scipy.stats import rankdata

sys.path.insert(0, sys.argv[2])
import deney as D  # noqa: E402


def ksg(x, y, k=4):
    xy = np.hstack([x, y])
    d, _ = cKDTree(xy).query(xy, k=k + 1, p=np.inf)
    eps = d[:, -1] - 1e-12
    nx = cKDTree(x).query_ball_point(x, eps, p=np.inf, return_length=True) - 1
    ny = cKDTree(y).query_ball_point(y, eps, p=np.inf, return_length=True) - 1
    return (digamma(k) + digamma(len(x)) - np.mean(digamma(nx + 1) + digamma(ny + 1))) / np.log(2)


def ranks(a, rng):
    a = a + 1e-9 * np.abs(a).max(0, keepdims=True) * rng.standard_normal(a.shape)  # eşitleri rastgele böl
    return np.column_stack([rankdata(a[:, j]) / len(a) for j in range(a.shape[1])])


def scale_of(tr):
    return np.where(tr.zero_band, tr.dglob, tr.delta)


def rows(tr, sel):
    s = scale_of(tr)
    u = tr.theta / s
    ud = tr.r / s
    out = []
    for ch in range(2):
        for t in range(1, tr.g - 1):
            if tr.blocks[ch, t - 1 : t + 2].any() or tr.ms[t - 1] != tr.ms[t] or tr.ms[t + 1] != tr.ms[t]:
                continue
            k = np.nonzero(sel[ch, t])[0]
            if len(k) == 0:
                continue
            out.append(np.column_stack([u[ch, t, k], u[ch, t - 1, k], u[ch, t + 1, k], ud[ch, t - 1, k], ud[ch, t + 1, k],
                                        np.log2(s[ch, t, k]), k]))
    return np.vstack(out)


def level_rows(tr, mask):
    """Bant başına: (gerçek delik log2 RMS, log2 kutu, bant başlangıcı, komşu çözülmüş log2 enerji ±1, komşu gerçek delik log2 RMS ±1)."""
    s = scale_of(tr)
    hole = tr.q == 0
    out = []
    for ch in range(2):
        for t in range(1, tr.g - 1):
            if tr.blocks[ch, t - 1 : t + 2].any():
                continue
            for sl in tr.bands(ch, t):
                h = hole[ch, t, sl] & mask[ch, t, sl]
                if h.sum() < 2:
                    continue
                e = np.mean(tr.theta[ch, t, sl][h] ** 2)
                if e <= 0:
                    continue
                feat = [0.5 * np.log2(e), np.log2(s[ch, t, sl.start]), sl.start]
                ok = True
                for d in (-1, 1):
                    rd = tr.r[ch, t + d, sl]
                    feat.append(0.5 * np.log2(np.mean(rd**2) + 1e-30))
                for d in (-1, 1):
                    hd = hole[ch, t + d, sl]
                    if hd.sum() < 2:
                        ok = False
                        break
                    feat.append(0.5 * np.log2(np.mean(tr.theta[ch, t + d, sl][hd] ** 2) + 1e-30))
                if ok:
                    out.append(feat)
    return np.array(out)


def knn_cv_err(X, y, k=10):
    """2 katlı çapraz geçerleme, kNN ortalaması; özellikler standartlaştırılır. Mutlak hatanın ortancası (dB)."""
    X = (X - X.mean(0)) / (X.std(0) + 1e-9)
    n = len(y)
    idx = np.arange(n)
    fold = idx % 2
    err = np.zeros(n)
    for f in (0, 1):
        tr_i, te_i = fold != f, fold == f
        _, nb = cKDTree(X[tr_i]).query(X[te_i], k=k)
        err[te_i] = y[tr_i][nb].mean(1) - y[te_i]
    return np.median(np.abs(err)) * 6.02, np.mean(err) * 6.02  # log2 genlik → dB


def main(folder, names):
    rng = np.random.default_rng(1)
    for name in names:
        tr = D.Track(f"{folder}/{name}.bin")
        hole = tr.q == 0
        print(f"\n== {name}", flush=True)
        for label, sel in (("canlı bant delikleri", hole & ~tr.zero_band), ("sıfır bant delikleri", hole & tr.zero_band)):
            a = rows(tr, sel)
            if len(a) < 2000:
                print(f"  {label}: az örnek {len(a)}")
                continue
            a = a[rng.choice(len(a), size=min(len(a), 20000), replace=False)]
            R = ranks(a[:, :7], rng)
            X, Y, Z = R[:, :1], R[:, 1:3], np.column_stack([R[:, 3:5], R[:, 5:7]])
            i_xz = ksg(X, Z)
            i_xyz = ksg(X, np.hstack([Y, Z]))
            i_xy = ksg(X, Y)
            Ysh = Y[rng.permutation(len(Y))]
            i_xyz_sh = ksg(X, np.hstack([Ysh, Z]))
            print(f"  {label:22s} n={len(a)}  I(X;Z) {i_xz:+.3f}  I(X;Y) {i_xy:+.3f}  I(X;Y,Z) {i_xyz:+.3f}  "
                  f"→ I(X;Y|Z) {i_xyz - i_xz:+.3f} bit  (Y karıştırılmış: {i_xyz_sh - i_xz:+.3f})", flush=True)
        for label, m in (("canlı bant", ~tr.zero_band), ("sıfır bant", tr.zero_band)):
            L = level_rows(tr, m)
            if len(L) < 200:
                print(f"  seviye {label}: az bant {len(L)}")
                continue
            dec = knn_cv_err(L[:, 1:5], L[:, 0])
            orc = knn_cv_err(L[:, 1:7], L[:, 0])
            only = knn_cv_err(L[:, 5:7], L[:, 0])
            print(f"  seviye {label:10s} bant {len(L):6d}: çözücü öznitelikleri ortanca |hata| {dec[0]:4.1f} dB, "
                  f"+ komşuların gerçek seviyesi {orc[0]:4.1f} dB, yalnız komşu gerçek seviye {only[0]:4.1f} dB", flush=True)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[3:])
