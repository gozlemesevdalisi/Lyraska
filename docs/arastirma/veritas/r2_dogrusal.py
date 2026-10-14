"""R2 yapılabilirlik: deliklerin (q=0) değerini komşu (granül, frekans) çözülmüş değerlerinden doğrusal kestirmek.
Birini dışarıda bırak: beş parçada eğit, altıncıda sına. Tahmin kutuya kırpılır (dosyayla çelişmez)."""
import sys
import numpy as np
import deney as D

DT, DK = 2, 3
NREG = 12

def features(tr):
    """Her (kanal, granül, k) için komşu r değerleri (5 × 7) ve |r| ölçeği; uzun bloklar."""
    r = tr.r
    pad = np.pad(r, ((0, 0), (DT, DT), (DK, DK)))
    feats = []
    for dt in range(-DT, DT + 1):
        for dk in range(-DK, DK + 1):
            if dt == 0 and dk == 0:
                continue
            feats.append(pad[:, DT + dt:DT + dt + tr.g, DK + dk:DK + dk + 576])
    return np.stack(feats, -1)  # (2, g, 576, F)

def dataset(tr):
    F = features(tr)
    longb = np.broadcast_to((tr.blocks != 2)[:, :, None], tr.q.shape)
    m = (tr.q == 0) & longb
    # ölçek: komşuların kök ortalama karesi + Δ/2 (birimsiz öğrenme için)
    scale = np.sqrt(np.mean(F ** 2, -1)) + np.where(tr.zero_band, tr.dglob, tr.delta) / 2
    k = np.broadcast_to(np.arange(576)[None, None, :], tr.q.shape)
    return F[m] / scale[m][:, None], tr.theta[m] / scale[m], (k[m] * NREG) // 576, m, scale

names = sys.argv[2:]
tracks = {n: D.Track(f"{sys.argv[1]}/{n}.bin") for n in names}
data = {n: dataset(tr) for n, tr in tracks.items()}
for test in names:
    W = []
    for reg in range(NREG):
        X = np.concatenate([data[n][0][data[n][2] == reg] for n in names if n != test])
        y = np.concatenate([data[n][1][data[n][2] == reg] for n in names if n != test])
        A = X.T @ X + 1e-3 * len(X) * np.eye(X.shape[1])
        W.append(np.linalg.solve(A, X.T @ y))
    W = np.stack(W)
    tr = tracks[test]; X, y, reg, m, scale = data[test]
    pred = np.einsum("nf,nf->n", X, W[reg]) * scale[m]
    est = tr.r.copy(); est[m] = pred
    est = np.clip(est, tr.lo, tr.hi)
    hole_err0 = np.sum(tr.theta[m] ** 2); hole_err1 = np.sum((est[m] - tr.theta[m]) ** 2)
    x0, x1 = tr.pcm(tr.r), tr.pcm(est)
    print(f"{test:14s} delik hatası {10*np.log10(hole_err0/hole_err1):+.2f} dB | SNR {D.snr(tr.truth, x0):.2f} → {D.snr(tr.truth, x1):.2f} | "
          f"MOS {D.mos(tr.truth, x0, tr.rate):.3f} → {D.mos(tr.truth, x1, tr.rate):.3f}", flush=True)
