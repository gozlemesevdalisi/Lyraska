"""Bağımsız denetim (Fable'ın betiklerini kullanmadan): deliklerin ince yapısı hakkında komşu granüllerin GERÇEK
değerleri ne kadar bilgi taşıyor? Seviye kâhinle ayrılır (v = θ / bandın gerçek delik RMS'i), yalnızca yapı kalır.

1) Doğrusal: bin başına ayrı katsayılarla θ_t(k) ← θ_{t±1,2}(k) (çift granüllerde uydur, tek granüllerde sına);
   Gauss MI = −½ log₂(1 − R²).
2) Doğrusal olmayan: KSG kNN karşılıklı bilgi (k = 4, maks-norm), merkez v ile 4 komşu v.
Olumlu denetim: aynı ölçüler q ≠ 0 katsayılarda (< 2 kHz) — bilgi varsa ölçü onu görmeli.
Kullanım: VERITAS_LAME=… python -I r2_bagimsiz_komsu.py <döküm klasörü> <betik klasörü> <ad> …"""
import sys

import numpy as np
from scipy.spatial import cKDTree
from scipy.special import digamma

sys.path.insert(0, sys.argv[2])
import deney as D  # noqa: E402


def band_rms(tr, mask):
    out = np.zeros_like(tr.theta)
    for ch in range(2):
        for t in range(tr.g):
            for s in tr.bands(ch, t):
                m = mask[ch, t, s]
                if m.sum() >= 2:
                    v = out[ch, t, s]
                    v[m] = np.sqrt(np.mean(tr.theta[ch, t, s][m] ** 2))
                    out[ch, t, s] = v
    return out


def windows(tr):
    """Merkez granül t: t−2…t+2 hepsi uzun blok ve aynı M/S kipinde."""
    ok = []
    for t in range(2, tr.g - 2):
        if all(tr.ms[t + d] == tr.ms[t] for d in (-2, -1, 1, 2)):
            ok.append(t)
    return np.array(ok)


def ksg_mi(x, y, k=4):
    """Kraskov–Stögbauer–Grassberger (1. tür), bit."""
    n = len(x)
    xy = np.hstack([x, y])
    tree = cKDTree(xy)
    d, _ = tree.query(xy, k=k + 1, p=np.inf)
    eps = d[:, -1] - 1e-12
    nx = cKDTree(x).query_ball_point(x, eps, p=np.inf, return_length=True) - 1
    ny = cKDTree(y).query_ball_point(y, eps, p=np.inf, return_length=True) - 1
    return (digamma(k) + digamma(n) - np.mean(digamma(nx + 1) + digamma(ny + 1))) / np.log(2)


def collect(tr, sel, rms, ts, ch):
    """sel (2, g, 576) maskesindeki merkez katsayılar: (merkez v, komşu v ×4, bin)."""
    rows = []
    for t in ts:
        if tr.blocks[ch, t - 2 : t + 3].any():  # yalnız uzun blok
            continue
        m = sel[ch, t] & (rms[ch, t] > 0)
        k = np.nonzero(m)[0]
        if len(k) == 0:
            continue
        c = tr.theta[ch, t, k] / rms[ch, t, k]
        nb = []
        for d in (-2, -1, 1, 2):
            r = rms[ch, t + d, k]
            # komşunun kendi bandının ölçeği; komşuda delik değilse kendi |θ| ölçeği yerine aynı merkez ölçeği
            scale = np.where(r > 0, r, rms[ch, t, k])
            nb.append(tr.theta[ch, t + d, k] / scale)
        rows.append(np.column_stack([c, np.column_stack(nb), k, np.full(len(k), t)]))
    return np.vstack(rows) if rows else np.zeros((0, 7))


def linear_r2(a):
    """Bin başına ayrı doğrusal kestirici; çift granüllerde uydur, tek granüllerde sına."""
    y, X, k, t = a[:, 0], a[:, 1:5], a[:, 5].astype(int), a[:, 6].astype(int)
    pred = np.zeros_like(y)
    used = np.zeros(len(y), bool)
    for kk in np.unique(k):
        idx = k == kk
        tr_i, te_i = idx & (t % 2 == 0), idx & (t % 2 == 1)
        if tr_i.sum() < 20 or te_i.sum() < 5:
            continue
        w, *_ = np.linalg.lstsq(X[tr_i], y[tr_i], rcond=None)
        pred[te_i] = X[te_i] @ w
        used |= te_i
    yt, pt = y[used], pred[used]
    r2 = 1 - np.sum((yt - pt) ** 2) / np.sum(yt**2)
    return r2, used.sum()


def report(label, a, rng):
    if len(a) < 500:
        print(f"    {label:34s} az örnek ({len(a)})")
        return
    r2, n = linear_r2(a)
    gmi = -0.5 * np.log2(max(1e-9, 1 - max(r2, 0)))
    pick = rng.choice(len(a), size=min(len(a), 20000), replace=False)
    s = a[pick]
    # KSG'de eşit değerler sorun: çok küçük titreşim
    x = s[:, :1] + 1e-10 * rng.standard_normal((len(s), 1))
    y = s[:, 1:5] + 1e-10 * rng.standard_normal((len(s), 4))
    mi = ksg_mi(x, y)
    yshuf = y[rng.permutation(len(y))]
    mi0 = ksg_mi(x, yshuf)
    print(f"    {label:34s} n={len(a):7d}  doğrusal R²={r2:+.4f} (Gauss MI {gmi:.3f} bit, sınama {n})  "
          f"KSG MI {mi:+.3f} bit (karıştırılmış {mi0:+.3f})", flush=True)


def main(folder, names):
    rng = np.random.default_rng(0)
    for name in names:
        tr = D.Track(f"{folder}/{name}.bin")
        hole = tr.q == 0
        f = np.arange(576)[None, None, :] * tr.rate / 1152
        live, zero = hole & ~tr.zero_band, hole & tr.zero_band
        coded_low = (tr.q != 0) & (f < 2000)
        rms_h = band_rms(tr, hole)
        rms_c = band_rms(tr, tr.q != 0)
        ts = windows(tr)
        print(f"\n== {name} ({tr.rate} Hz), pencere {len(ts)}/{tr.g}", flush=True)
        for label, sel, rms in (("delik, canlı bant", live, rms_h), ("delik, sıfır bant", zero, rms_h),
                                ("delik, < 5 kHz", hole & (f < 5000), rms_h),
                                ("OLUMLU DENETİM: q≠0, < 2 kHz", coded_low, rms_c)):
            a = np.vstack([collect(tr, sel, rms, ts, ch) for ch in range(2)])
            report(label, a, rng)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[3:])
