"""Veritas R1 deney düzeneği: döküm → çözücü görünümü → yöntemler → ölçüler.

Gerçek değer θ (kodlayıcının nicemlediği katsayı) yamalı LAME'in dökümünden gelir; yöntemler yalnızca çözücünün
görebildiği bilgiyi (q, Δ, blok türü, M/S, kutular) kullanır. Ölçüler zaman alanında S(θ)'ya göre: SNR ve
Zimtohrli (Google, Apache-2.0; 48 kHz). LAME kaynak klasörü VERITAS_LAME ortam değişkeninden okunur."""
import os
import subprocess
from math import gcd

import numpy as np
from scipy import signal as sps
from scipy import special
from scipy.ndimage import maximum_filter, median_filter, uniform_filter

import hibrit as H
import kapsama as K

LAME = os.environ.get("VERITAS_LAME", "lame-3.100")
C_LONG, C_SHORT = 288.0, 96.0  # SᵀS: uzun (ve geçiş) blokta 288, kısa blokta 96 (ölçüldü)


class Track:
    """Bir dökümün çözücü görünümü + gerçek değer. Katsayılar (kanal, granül, 576), kodlama alanında
    (M/S karelerinde M ve S)."""

    def __init__(self, path):
        mp3 = path[:-4] + ".mp3"
        out = subprocess.run(
            ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries", "stream=sample_rate", "-of", "csv=p=0", mp3],
            capture_output=True, text=True,
        ).stdout.strip().strip(",")
        self.rate = int(out) if out else 44100
        self.window = H.read_window(os.path.join(LAME, "mpglib", "tabinit.c"))
        recs = K.read(path)
        steps = K.decoder_steps(recs)
        g = len(recs) // 2
        self.g = g
        shape = (2, g, 576)
        self.q, self.theta, self.delta, self.dglob = (np.zeros(shape) for _ in range(4))
        self.zero_band = np.zeros(shape, bool)
        self.blocks = np.zeros((2, g), int)
        self.widths = np.zeros((2, g, 39), int)
        self.ms = np.zeros(g, bool)
        for i, ((ints, theta, qq), (delta, block, mixed)) in enumerate(zip(recs, steps)):
            t, ch = i // 2, i % 2
            assert ints[1] == ch and not mixed
            self.q[ch, t], self.theta[ch, t], self.delta[ch, t] = qq, theta, delta
            self.dglob[ch, t] = 2.0 ** ((ints[4] - 210) / 4)
            self.blocks[ch, t] = block
            w = ints[16 + K.SFBMAX : 16 + 2 * K.SFBMAX]
            self.widths[ch, t] = w
            self.ms[t] = bool(ints[5] & 2)
            j = 0
            for wi in w:
                if wi <= 0 or j >= 576:
                    break
                self.zero_band[ch, t, j : j + wi] = not np.any(qq[j : j + wi])
                j += wi
        # LAME'in l3_enc'i işaretsizdir; q ≠ 0 için işaret bit akışında (θ'nın işareti), q = 0'da bilinmez.
        self.sign = np.where(self.q > 0, np.sign(self.theta), 0.0)
        self.r = self.sign * self.q ** (4 / 3) * self.delta
        self.lo, self.hi = self.boxes()
        self.cscale = np.where(self.blocks == 2, C_SHORT, C_LONG)[:, :, None] * np.ones((1, 1, 576))
        self.truth = self.pcm(self.theta)

    def boxes(self):
        """LAME 3.100 kuralı (adj43) + bant kuralı (bütün q'ları 0 olan bantta Δ_genel); işaretli [lo, hi]."""
        q = self.q
        a = np.where(q == 0, 0.0, (np.maximum(q - 1, 0) ** (4 / 3) + q ** (4 / 3)) / 2) * self.delta
        b = (q ** (4 / 3) + (q + 1) ** (4 / 3)) / 2 * self.delta
        half = np.where(self.zero_band, self.dglob, self.delta) / 2
        lo = np.where(q == 0, -half, np.where(self.sign > 0, a, -b))
        hi = np.where(q == 0, half, np.where(self.sign > 0, b, -a))
        return lo, hi

    def bands(self, ch, t):
        j = 0
        for w in self.widths[ch, t]:
            if w <= 0 or j >= 576:
                break
            yield slice(j, j + w)
            j += w

    def _ms(self, c):
        """M/S ↔ L/R: [[1, 1], [1, −1]]/√2 kendi tersidir."""
        out = c.copy()
        a, b = c[0][self.ms], c[1][self.ms]
        out[0][self.ms] = (a + b) / np.sqrt(2)
        out[1][self.ms] = (a - b) / np.sqrt(2)
        return out

    def pcm(self, coded):
        """Kodlama alanındaki katsayılar → PCM (L/R). Standart çözücüyle aynı (ffmpeg'e göre −125 dB)."""
        lr = self._ms(coded)
        return np.stack([H.synth(lr[ch], self.blocks[ch], self.widths[ch], self.window) for ch in range(2)], 1)

    def analyze(self, pcm):
        """PCM (L/R) → kodlama alanı: A = (SᵀS)⁻¹ Sᵀ, granül başına ölçek."""
        lr = np.stack([H.synth_adjoint(pcm[:, ch], self.blocks[ch], self.widths[ch], self.window) for ch in range(2)])
        return self._ms(lr / self.cscale)


def snr(ref, x, skip=4096):
    e = ref[skip:-skip] - x[skip:-skip]
    return 10 * np.log10(np.sum(ref[skip:-skip] ** 2) / np.sum(e**2))


def mos(ref, x, rate):
    """Zimtohrli uzaklığından tahmini MOS (1–5), kanalların ortalaması."""
    import zimtohrli as Z

    z = Z.Pyohrli()
    g = gcd(48000, rate)
    dist = []
    for ch in range(ref.shape[1]):
        a = sps.resample_poly(ref[:, ch], 48000 // g, rate // g)
        b = sps.resample_poly(x[:, ch], 48000 // g, rate // g)
        dist.append(z.distance(a, b))
    return Z.mos_from_zimtohrli(float(np.mean(dist)))


def stft(x, n):
    return sps.stft(x, nperseg=n, noverlap=3 * n // 4, window="hann", boundary="even", padded=True)[2]


def istft(z, n, length):
    return sps.istft(z, nperseg=n, noverlap=3 * n // 4, window="hann", boundary=True)[1][:length]


# ---------- yöntemler (hepsi yalnızca çözücünün bilgisini kullanır) ----------


def laplace_fit(q, grid=np.exp(np.linspace(np.log(0.02), np.log(200), 60))[:, None]):
    """Bir ölçek bandındaki |θ|/Δ için Laplace ölçeği b: aralıkla sansürlenmiş gözlemlerden en çok olabilirlik."""
    lo = np.where(q == 0, 0.0, (np.maximum(q - 1, 0) ** (4 / 3) + q ** (4 / 3)) / 2)
    hi = (q ** (4 / 3) + (q + 1) ** (4 / 3)) / 2
    ll = np.sum(-lo / grid + np.log(-np.expm1(-(hi - lo) / grid) + 1e-300), axis=1)
    return grid[np.argmax(ll), 0], lo, hi


def laplace_mmse(tr):
    """Uyarlamalı Laplace ağırlık merkezi (Seviye 0+): kutu içindeki koşullu ortalama; q = 0 → 0."""
    est = tr.r.copy()
    for ch in range(2):
        for t in range(tr.g):
            for s in tr.bands(ch, t):
                qq = tr.q[ch, t, s]
                if not qq.any():
                    continue
                b, lo, hi = laplace_fit(qq)
                ea, eb = np.exp(-lo / b), np.exp(-hi / b)
                mean = np.clip(b + (lo * ea - hi * eb) / (ea - eb + 1e-300), lo, hi)
                est[ch, t, s] = np.where(qq == 0, 0.0, tr.sign[ch, t, s] * mean * tr.delta[ch, t, s])
    return est


def trunc_normal_mean(mu, s, lo, hi):
    """N(μ, s²)'nin [lo, hi]'ye kesilmiş ortalaması (erfcx ile kuyruklarda kararlı)."""
    flip = (lo - mu) / s > 0
    m = np.where(flip, -mu, mu)
    a, b = np.where(flip, -hi, lo), np.where(flip, -lo, hi)
    al, be = (a - m) / s, (b - m) / s
    out = np.empty_like(mu)
    left = be <= 0
    x1, x2 = -be[left], -al[left]
    e = np.exp((x1**2 - x2**2) / 2)
    rr = special.erfcx(x2 / np.sqrt(2)) / special.erfcx(x1 / np.sqrt(2))
    out[left] = m[left] - s[left] * np.sqrt(2 / np.pi) / special.erfcx(x1 / np.sqrt(2)) * (1 - e) / (1 - e * rr + 1e-300)
    mid = ~left
    pa, pb = special.ndtr(al[mid]), special.ndtr(be[mid])
    fa, fb = (np.exp(-v**2 / 2) / np.sqrt(2 * np.pi) for v in (al[mid], be[mid]))
    out[mid] = m[mid] + s[mid] * (fa - fb) / np.maximum(pb - pa, 1e-300)
    return np.clip(np.where(flip, -out, out), lo, hi)


def wiener_fusion(tr, n_fft=4096, k_mc=4, alpha=1.0, seed=0):
    """Uzun pencere Wiener önseli + kesik Gauss füzyonu (Sadık, en küçük ortalama hata).

    Gürültü gücü kutulardan Monte Carlo ile: u ~ düzgün(kutu), n = S(u − r) → |STFT|². G = ξ/(1+ξ),
    ξ = max(P_x/P_n − 1, 0) (3×3 yumuşatılmış). Önsel μ = A·iSTFT(G·X); s² = α · yerel ortalama (μ − r)².
    Sonuç θ̂ = E[θ | θ ∈ kutu, θ ~ N(μ, s²)]: kutu dışına çıkamaz."""
    rng = np.random.default_rng(seed)
    length = tr.truth.shape[0]
    x0 = tr.pcm(tr.r)
    noise_p = 0
    for _ in range(k_mc):
        n = tr.pcm(tr.lo + (tr.hi - tr.lo) * rng.random(tr.lo.shape) - tr.r)
        noise_p = noise_p + np.abs(np.stack([stft(n[:, c], n_fft) for c in range(2)])) ** 2 / k_mc
    X = np.stack([stft(x0[:, c], n_fft) for c in range(2)])
    xi = np.maximum(uniform_filter(np.abs(X) ** 2, (1, 3, 3)) / (uniform_filter(noise_p, (1, 3, 3)) + 1e-30) - 1, 0)
    x1 = np.stack([istft(xi[c] / (1 + xi[c]) * X[c], n_fft, length) for c in range(2)], 1)
    mu = tr.analyze(x1)
    dv = ((mu - tr.r) ** 2).reshape(2, tr.g, 32, 18).mean(-1, keepdims=True)
    s = np.sqrt(alpha * np.broadcast_to(dv, (2, tr.g, 32, 18)).reshape(2, tr.g, 576) + 1e-20)
    return trunc_normal_mean(mu, s, tr.lo, tr.hi)


def energy_match(tr, est):
    """Her (kanal, granül, ölçek bandı) için enerjiyi çözücününkine eşitle, sonra kutuya kırp: kestirimin
    yapısı kalır, kulağın duyduğu bant enerjisi değişmez (algı–bozulma ödünleşimi)."""
    out = est.copy()
    for ch in range(2):
        for t in range(tr.g):
            for s in tr.bands(ch, t):
                e1, e0 = np.sum(est[ch, t, s] ** 2), np.sum(tr.r[ch, t, s] ** 2)
                if e1 > 0 and e0 > 0:
                    out[ch, t, s] = est[ch, t, s] * np.sqrt(e0 / e1)
    return np.clip(out, tr.lo, tr.hi)


def tonal_fill(tr, n_fft=8192, prom_db=12.0, halfw=2):
    """Faz-tutarlı tonal önsel ile delik (q = 0) doldurma: uzun STFT'de belirgin tepeler (±halfw kutu) → tonal
    parça → μ = A·x_tonal → deliklerde kesik Gauss füzyonu (s = Δ/4)."""
    x0 = tr.pcm(tr.r)
    xt = np.zeros_like(x0)
    for ch in range(2):
        X = stft(x0[:, ch], n_fft)
        mag = 20 * np.log10(np.abs(X) + 1e-12)
        med = median_filter(mag, size=(31, 1), mode="nearest")
        peak = (mag == maximum_filter(mag, size=(3, 1))) & (mag - med > prom_db) & (mag > mag.max() - 100)
        mask = maximum_filter(peak.astype(np.uint8), size=(2 * halfw + 1, 1)).astype(bool)
        xt[:, ch] = istft(np.where(mask, X, 0), n_fft, x0.shape[0])
    mu = tr.analyze(xt)
    s = 0.25 * np.where(tr.zero_band, tr.dglob, tr.delta)
    return np.where(tr.q == 0, trunc_normal_mean(mu, s, tr.lo, tr.hi), tr.r)


def hole_energy(tr):
    """Delik katsayısı başına beklenen θ², yalnızca bit akışından. Canlı bantta Laplace uyumundan E[x² | x < Δ/2];
    sıfır bantta komşu granüllerdeki aynı bandın çözülmüş enerjisi (Δ_genel sınırına kırpılmış)."""
    ms = np.zeros_like(tr.r)
    dec = {}
    for ch in range(2):
        for t in range(tr.g):
            for bi, s in enumerate(tr.bands(ch, t)):
                dec[ch, t, bi] = np.mean(tr.r[ch, t, s] ** 2)
    for ch in range(2):
        for t in range(tr.g):
            for bi, s in enumerate(tr.bands(ch, t)):
                qq = tr.q[ch, t, s]
                if qq.any():
                    b, _, _ = laplace_fit(qq)
                    c = 0.5
                    e2 = (2 * b * b - np.exp(-c / b) * (c * c + 2 * b * c + 2 * b * b)) / (1 - np.exp(-c / b))
                    ms[ch, t, s] = np.where(qq == 0, e2 * tr.delta[ch, t, s] ** 2, 0)
                else:
                    nb = [dec.get((ch, u, bi), 0) for u in (t - 1, t + 1)]
                    nb = [v for v in nb if v > 0]
                    cap = (tr.dglob[ch, t, s.start] / 2) ** 2 / 3
                    ms[ch, t, s] = min(np.exp(np.mean(np.log(nb))), cap) if nb else 0.0
    return ms


def noise_fill(tr, mean_square, seed=0):
    """Deliklere verilen ortalama kareyle Gauss gürültü, kutuya kırpılmış (Doğal kip denemesi)."""
    rng = np.random.default_rng(seed)
    n = rng.standard_normal(tr.r.shape) * np.sqrt(mean_square)
    return np.where(tr.q == 0, np.clip(n, tr.lo, tr.hi), tr.r)


def oracle_hole_energy(tr):
    """Kâhin: her bandın deliklerindeki gerçek enerji (yalnızca tavan ölçümü için)."""
    ms = np.zeros_like(tr.r)
    hole = tr.q == 0
    for ch in range(2):
        for t in range(tr.g):
            for s in tr.bands(ch, t):
                h = hole[ch, t, s]
                if h.any():
                    v = ms[ch, t, s]
                    v[h] = np.mean(tr.theta[ch, t, s][h] ** 2)
                    ms[ch, t, s] = v
    return ms
