"""R2 değerlendirme: bilgi ölçümü (S1), örnekleyiciler (S2) ve bilgi–algı eğrisi, R1 ölçüleriyle.

Her parça için:
- S1: delik katsayısı başına düzgün dağılıma göre kazanılan bit (−log₂ p(u), u = θ/s ∈ [−½, ½]): kutu içinde düzgün
  (0 bit), sıfırda Laplace (bant uyumu), öğrenilmiş marjinal (bağlamsız), öğrenilmiş koşullu (çözücü bağlamı),
  kâhin bağlam (komşu granüllerin gerçek değeri). Canlı/sıfır bant ve 5 kHz altı/üstü ayrımıyla.
- S2: çözücü, karışımdan örnek, karışım ortalaması (MMSE), akış örneği; SNR, MOS, kutu dışı oranı, süre.
- Bilgi–algı eğrisi: gürültülü kâhin y = u + N(0, σ²) kutuya kırpılmış; bit kazancı −log₂ p(u | y) (kutuda düzgün\n  önselle kesik Gauss sonsal) ve MOS; σ kutu ölçeğinde ya da gerçek delik RMS'i ölçeğinde.
Kullanım: VERITAS_LAME=… python r2_degerlendir.py <döküm klasörü> <npz klasörü> <çıkış.json> <ad> […]
          [--model ad=yol …] [--akis yol] [--egri]"""
import argparse
import json
import math
import os
import time

import numpy as np
import torch

import deney as D
import r2_model as M


def laplace_bits(d):
    """Sıfırda Laplace, ölçek her ölçek bandında q'dan (sansürlenmiş en çok olabilirlik); u = θ/Δ üzerinde kesik.
    Bütün q'ları sıfır bantta uyum yapılamaz: düzgün (0 bit)."""
    q, theta, delta = d["q"], d["theta"], d["delta"]
    bits = np.zeros(q.shape)
    for ch in range(2):
        for t in range(q.shape[1]):
            j = 0
            for w in d["widths"][ch, t]:
                if w <= 0 or j >= 576:
                    break
                s = slice(j, j + w)
                qq = q[ch, t, s]
                if qq.any():
                    b, _, _ = D.laplace_fit(qq)
                    u = np.abs(theta[ch, t, s] / delta[ch, t, s])
                    logp = -u / b - np.log(2 * b * (1 - np.exp(-0.5 / b)))
                    bits[ch, t, s] = np.where(qq == 0, logp / np.log(2), 0)
                j += w
    return bits


def model_bits_and_fill(net, td, hole, kind, gen, steps=32):
    """Bütün granüller için −log₂ p(u) (yoğunluk) ve/veya örnek/ortalama (u biriminde)."""
    bits = np.zeros(hole.shape)
    sample = np.zeros(hole.shape)
    mean = np.zeros(hole.shape)
    e2 = np.zeros(hole.shape)
    with torch.no_grad():
        for ch in range(2):
            for t0 in range(0, td.g, 128):
                ts = np.arange(t0, min(td.g, t0 + 128))
                feats = M.features(td, ch, ts)
                h = torch.from_numpy(hole[ch, ts].astype(np.float32))
                if kind == "yogunluk":
                    out = net(feats)
                    u = torch.from_numpy(td.u[ch, ts])
                    bits[ch, ts] = (-M.mixture_logp(out, u) / math.log(2)).numpy()
                    sample[ch, ts] = M.mixture_sample(out, gen).numpy()
                    m1, m2 = M.mixture_moments(out)
                    mean[ch, ts], e2[ch, ts] = m1.numpy(), m2.numpy()
                else:
                    sample[ch, ts] = M.flow_sample(net, feats, h, steps, gen).numpy()
    return bits, sample, mean, e2


def band_energy_error(tr, pred_ms, mask):
    """Delik enerjisi kestiriminin bant başına hatası (dB, kestirim − gerçek): ortanca ve %10/%90, canlı ve sıfır
    bantlar ayrı. pred_ms: katsayı başına kestirilen θ² (θ biriminde)."""
    out = {}
    for label, m in (("canlı bant", mask & ~tr.zero_band), ("sıfır bant", mask & tr.zero_band)):
        errs = []
        for ch in range(2):
            for t in range(tr.g):
                for sl in tr.bands(ch, t):
                    h = m[ch, t, sl]
                    if h.sum() >= 2:
                        e_true, e_pred = np.sum(tr.theta[ch, t, sl][h] ** 2), np.sum(pred_ms[ch, t, sl][h])
                        if e_true > 0 and e_pred > 0:
                            errs.append(10 * np.log10(e_pred / e_true))
        errs = np.array(errs)
        out[label] = {"ortanca dB": float(np.median(errs)), "%10": float(np.percentile(errs, 10)), "%90": float(np.percentile(errs, 90)), "bant": int(len(errs))}
    return out


def scale_to_energy(tr, est, target_ms, hole):
    """Deliklerdeki kestirimi bant başına hedef enerjiye (Σ target_ms) ölçekle, kutuya kırp; yapı kalır."""
    out = est.copy()
    for ch in range(2):
        for t in range(tr.g):
            for sl in tr.bands(ch, t):
                h = hole[ch, t, sl]
                if h.any():
                    e1, e0 = np.sum(est[ch, t, sl][h] ** 2), np.sum(target_ms[ch, t, sl][h])
                    if e1 > 0 and e0 > 0:
                        v = out[ch, t, sl]
                        v[h] = est[ch, t, sl][h] * np.sqrt(e0 / e1)
                        out[ch, t, sl] = v
    return np.clip(out, tr.lo, tr.hi)


def entropy_bits(x, bins=512):
    """1-B diferansiyel entropi (bit), histogramla."""
    h, edges = np.histogram(x, bins=bins)
    p = h / h.sum()
    w = edges[1] - edges[0]
    p = p[p > 0]
    return -np.sum(p * np.log2(p)) + np.log2(w)


def evaluate(tr, d, name, models, akis, egri, seed=0):
    res = {"ad": name, "rate": tr.rate}
    hole = tr.q == 0
    f = np.arange(576)[None, None, :] * tr.rate / 1152
    live, zero = hole & ~tr.zero_band, hole & tr.zero_band
    regions = {"tümü": hole, "canlı bant": live, "sıfır bant": zero, "< 5 kHz": hole & (f < 5000), "≥ 5 kHz": hole & (f >= 5000)}
    res["delik payı"] = float(hole.mean())
    s = M.scale_of(d)
    u_true = np.clip(tr.theta / s, -0.5, 0.5)

    def summarize(bits):
        return {k: float(np.mean(bits[m])) for k, m in regions.items() if m.any()}

    res["bit"] = {"düzgün": summarize(np.zeros(hole.shape)), "Laplace (bant)": summarize(-laplace_bits(d))}
    res["bit"]["marjinal h(u) (kâhin histogram)"] = {k: float(-entropy_bits(u_true[m])) for k, m in regions.items() if m.sum() > 1000}
    gen = torch.Generator().manual_seed(seed)
    ests = {"çözücü": tr.r}
    for label, (net, ck) in models.items():
        td = M.TrackData(d, ck["baglam"])
        t0 = time.time()
        bits, sample, mean, e2 = model_bits_and_fill(net, td, hole, "yogunluk", gen)
        res.setdefault("süre", {})[label] = time.time() - t0
        res["bit"][label] = summarize(-bits)  # kazanılan bit = −NLL
        pred_ms = e2 * s * s
        res.setdefault("enerji hatası", {})[label] = band_energy_error(tr, pred_ms, hole)
        ests[f"{label}: örnek"] = np.where(hole, sample * s, tr.r)
        ests[f"{label}: ortalama (MMSE)"] = np.where(hole, mean * s, tr.r)
        ests[f"{label}: ortalama, model enerjisine ölçekli"] = scale_to_energy(tr, ests[f"{label}: ortalama (MMSE)"], pred_ms, hole)
        ests[f"{label}: model enerjisiyle gürültü"] = D.noise_fill(tr, pred_ms, seed)
        # Sıfır bantların seviyesi bit akışında yok ve modelin oradaki hatası büyük: yalnızca canlı bant deliklerini
        # örnekle, sıfır bantları 0'da bırak (kâhin tavanı "yalnız canlı bant delikleri").
        ests[f"{label}: örnek, yalnız canlı bant"] = np.where(live, sample * s, tr.r)
    res["enerji hatası"] = res.get("enerji hatası", {})
    res["enerji hatası"]["R1 Laplace / komşu granül (hole_energy)"] = band_energy_error(tr, D.hole_energy(tr), hole)
    if akis is not None:
        net, ck = akis
        td = M.TrackData(d, ck["baglam"])
        t0 = time.time()
        _, sample, _, _ = model_bits_and_fill(net, td, hole, "akis", gen)
        res.setdefault("süre", {})["akış"] = time.time() - t0
        ests["akış: örnek"] = np.where(hole, sample * s, tr.r)
        ests["akış: örnek + bant enerjisi koruma"] = D.energy_match(tr, ests["akış: örnek"])
    ests["kâhin: delikler gerçek"] = np.where(hole, tr.theta, tr.r)
    if egri:
        # Gürültülü kâhin: y = u + N(0, σ²). Sonsal (kutuda düzgün önsel × Gauss olabilirlik) kesik Gauss'tur;
        # bit kazancı = −log₂ p(u | y) ile ölçülür: modellerle aynı ölçü. İki ölçek: kutu (σ = α/2) ve gerçek
        # delik RMS'i (σ = α·RMS_bant/s; kâhin enerjisi). Kutu ölçeği sıfır bantlarda gerçek seviyenin çok üstündedir.
        from scipy import stats

        rng = np.random.default_rng(seed)
        rms_u = np.sqrt(D.oracle_hole_energy(tr)) / s
        res["eğri"] = []
        for olcek, base in (("kutu", np.full(s.shape, 0.5)), ("RMS", rms_u)):
            for alpha in (0.05, 0.1, 0.25, 0.5, 1.0):
                sigma = np.maximum(alpha * base, 1e-6)
                y = u_true + rng.standard_normal(u_true.shape) * sigma
                z = stats.norm.cdf((0.5 - y) / sigma) - stats.norm.cdf((-0.5 - y) / sigma)
                logp = stats.norm.logpdf(u_true, y, sigma) - np.log(np.maximum(z, 1e-300))
                gain = float(np.mean(logp[hole]) / np.log(2))
                est = np.where(hole, np.clip(y, -0.5, 0.5) * s, tr.r)
                x = tr.pcm(est)
                res["eğri"].append({"ölçek": olcek, "alpha": alpha, "bit": gain, "MOS": float(D.mos(tr.truth, x, tr.rate)), "SNR": float(D.snr(tr.truth, x))})
    res["yöntem"] = {}
    for label, est in ests.items():
        x = tr.pcm(est)
        outside = float(np.mean((est < tr.lo - 1e-9) | (est > tr.hi + 1e-9)))
        res["yöntem"][label] = {"SNR": float(D.snr(tr.truth, x)), "MOS": float(D.mos(tr.truth, x, tr.rate)), "kutu dışı": outside}
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dokum")
    ap.add_argument("npz")
    ap.add_argument("cikti")
    ap.add_argument("adlar", nargs="+")
    ap.add_argument("--model", action="append", default=[])
    ap.add_argument("--akis")
    ap.add_argument("--egri", action="store_true")
    ap.add_argument("--is-parcacigi", type=int, default=4)
    a = ap.parse_args()
    torch.set_num_threads(a.is_parcacigi)
    models = {}
    for item in a.model:
        label, path = item.split("=", 1)
        models[label] = M.load_model(path)
    akis = M.load_model(a.akis) if a.akis else None
    results = []
    for name in a.adlar:
        tr = D.Track(os.path.join(a.dokum, name + ".bin"))
        d = M.load_npz(os.path.join(a.npz, name + ".npz"))
        res = evaluate(tr, d, name, models, akis, a.egri)
        results.append(res)
        print(f"\n== {name} ({tr.rate} Hz) delik payı {res['delik payı']:.3f}", flush=True)
        for k, v in res["bit"].items():
            print("  bit  " + f"{k:40s}" + "  ".join(f"{r}: {b:+.3f}" for r, b in v.items()), flush=True)
        for k, v in res.get("enerji hatası", {}).items():
            print("  enerji " + f"{k:38s}" + "  ".join(f"{r}: {e['ortanca dB']:+.1f} dB [{e['%10']:+.1f}, {e['%90']:+.1f}]" for r, e in v.items()), flush=True)
        for k, v in res["yöntem"].items():
            print(f"  {k:46s} SNR {v['SNR']:6.2f}  MOS {v['MOS']:.3f}  kutu dışı {v['kutu dışı']:.0e}", flush=True)
        for e in res.get("eğri", []):
            print(f"  eğri {e['ölçek']:5s} α={e['alpha']:.2f}  bit {e['bit']:+.2f}  MOS {e['MOS']:.3f}  SNR {e['SNR']:.2f}", flush=True)
        json.dump(results, open(a.cikti, "w"), ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
