"""Veritas şartnamesinin hesaplarının bağımsız doğrulaması (Δ = 1 birimi): nicemleme kutuları
(ISO ve LAME kuralı), Seviye 0 kazancı (Laplace), 2·MMSE özdeşliği, çoklu kopya formülü, dither."""
import numpy as np
from scipy import integrate
c = 1 - 2 ** (-0.75)
print(f"c = 1 − 2^(−3/4) = {c:.6f};  ölü bölge sınırı (2^(−3/4))^(4/3) = {(2**-0.75)**(4/3):.6f}")

def iso_cell(q):   # ISO: is = floor((|x|)^(3/4) + c)
    return (0.0, 0.5) if q == 0 else ((q - c) ** (4/3), (q + 1 - c) ** (4/3))
def lame_cell(q):  # LAME adj43: karar sınırı komşu yeniden kurma noktalarının ortası (|x| alanında)
    lo = 0.0 if q == 0 else ((q - 1) ** (4/3) + q ** (4/3)) / 2
    hi = (q ** (4/3) + (q + 1) ** (4/3)) / 2
    return (lo, hi)

print("\nq | ISO [lo, hi)      | LAME [lo, hi)     | r = q^(4/3)")
for q in range(6):
    a, b = iso_cell(q); la, lb = lame_cell(q)
    print(f"{q} | [{a:.3f}, {b:.3f}) | [{la:.3f}, {lb:.3f}) | {q**(4/3):.3f}")

def gains(cell, b):
    """Laplace(b) kaynağı: çözücü noktası (q^(4/3)) ve koşullu ortalama (ağırlık merkezi) MSE'leri."""
    pdf = lambda x: np.exp(-x / b) / b  # |x| için (işaret simetrik)
    mse_dec = mse_cen = 0.0
    q = 0
    while True:
        lo, hi = cell(q)
        if lo > 60 * b: break
        p = integrate.quad(pdf, lo, hi)[0]
        if p > 0:
            m1 = integrate.quad(lambda x: x * pdf(x), lo, hi)[0] / p
            r = 0.0 if q == 0 else q ** (4/3)
            mse_dec += integrate.quad(lambda x: (x - r) ** 2 * pdf(x), lo, hi)[0]
            # q = 0: işaret bilinmez, ağırlık merkezi 0 (simetrik) → çözücüyle aynı.
            cen = 0.0 if q == 0 else m1
            mse_cen += integrate.quad(lambda x: (x - cen) ** 2 * pdf(x), lo, hi)[0]
        q += 1
    return 10 * np.log10(mse_dec / mse_cen)

print("\nSeviye 0 (ağırlık merkezi) kazancı, dB:")
print("b/Δ   | ISO kuralı | LAME kuralı")
for bb in [0.3, 0.5, 0.8, 1.0, 2.0, 4.0]:
    print(f"{bb:<5} | {gains(iso_cell, bb):9.2f}  | {gains(lame_cell, bb):9.2f}")

# Bölüm 7: E||θ̃−θ||² = 2·MMSE (θ̃, θ ~ p(θ|q) bağımsız). Monte-Carlo: kutu içinde kesik Laplace.
rng = np.random.default_rng(0)
lo, hi, bb = 1.0, 2.0, 0.7
def sample(n):
    # Kutuya kesilmiş üstel dağılımdan ters dağılım yöntemiyle örnek.
    F = lambda x: 1 - np.exp(-x / bb)
    v = rng.uniform(F(lo), F(hi), n)
    return -bb * np.log(1 - v)
th, tt = sample(400_000), sample(400_000)
mmse = np.var(th)
print(f"\n2·MMSE özdeşliği: E||θ̃−θ||² / MMSE = {np.mean((tt - th) ** 2) / mmse:.4f} (beklenen 2,0000 → 3,01 dB)")

# Bölüm 8: M kopya, bağımsız rastgele kaymalı δ-genişlikli hücreler; kesişimin ortası.
for M in [1, 2, 3, 4, 5]:
    u = rng.uniform(0, 1, (400_000, M))
    err = (np.min(1 - u, axis=1) - np.min(u, axis=1)) / 2
    print(f"M={M}: Monte-Carlo kazanç {10*np.log10((1/12)/np.mean(err**2)):.2f} dB | formül {10*np.log10((M+1)*(M+2)/6):.2f} dB")

print(f"\nTPDF dither: 16 bit SQNR {6.02*16+1.76:.1f} dB → {6.02*16+1.76-10*np.log10(3):.1f} dB")
