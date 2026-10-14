"""F0 kapsama testi. Ölçüm yamalı LAME'in döktüğü her granül için (θ = kodlayıcının MDCT katsayısı,
q = nicemlenmiş tamsayı, yan bilgi) θ'nın, çözücünün görebildiği bilgiden kurulan I(q) kutusunun
içinde olup olmadığına bakar. ISO kuralı (c = 0,4054) ve LAME kuralı (adj43: komşu yeniden kurma
noktalarının ortası) ayrı ayrı. Ayrıca: B sınıfı ihlallerinin büyüklüğü ve yeri, bant kuralıyla B
kapsaması, hatanın sınıflara dağılımı, Seviye 0'ın (ağırlık merkezi) gerçek veride üst sınırı.

Kullanım: python kapsama.py dokum_128.bin [dokum_320.bin …]"""
import sys

import numpy as np

SFBMAX = 39
PRETAB = [0] * 11 + [1, 1, 1, 1, 2, 2, 3, 3, 3, 2, 0]
C_ISO = 1 - 2**-0.75
HEADER_INTS = 12 + 4 + 3 * SFBMAX
RECORD_BYTES = HEADER_INTS * 4 + 576 * 8


def read(path):
    """Döküm kayıtları: (tamsayı başlık, θ, q). Biçim: lame-3.100-veritas-dump.patch."""
    raw = open(path, "rb").read()
    records = []
    for offset in range(0, len(raw) - RECORD_BYTES + 1, RECORD_BYTES):
        ints = np.frombuffer(raw, np.int32, HEADER_INTS, offset).copy()
        theta = np.frombuffer(raw, np.float32, 576, offset + HEADER_INTS * 4).astype(np.float64)
        q = np.frombuffer(raw, np.int32, 576, offset + HEADER_INTS * 4 + 576 * 4).copy()
        records.append((ints, theta, q))
    return records


def decoder_steps(records):
    """Her kayıt için çözücünün kuralıyla katsayı başına Δ ve blok türü.

    - Uzun blokta sfb21, kısa blokta sfb12 için ölçek çarpanı iletilmez: çözücü 0 alır.
    - scfsi: LAME ikinci granülde birinci granülün çarpanını yeniden kullandığı bantları −1 diye
      işaretler; çözücü o bantlarda birinci granülün değerini kullanır."""
    first_granule = {}
    out = []
    for ints, _, _ in records:
        hdr, sbg = ints[:12], ints[12:16]
        sf = ints[16 : 16 + SFBMAX].copy()
        width = ints[16 + SFBMAX : 16 + 2 * SFBMAX]
        window = ints[16 + 2 * SFBMAX : 16 + 3 * SFBMAX]
        gr, ch, block, gain, sf_scale, preflag = hdr[0], hdr[1], hdr[2], hdr[4], hdr[6], hdr[7]
        if gr == 0:
            first_granule[ch] = sf.copy()
        elif ch in first_granule:
            sf = np.where(sf < 0, first_granule[ch], sf)
        delta = np.full(576, 2.0 ** ((gain - 210) / 4))
        j = 0
        for band in range(SFBMAX):
            if j >= 576 or width[band] <= 0:
                break
            if block == 2:
                sent = band // 3 < 12
                s = gain - ((sf[band] if sent else 0) << (sf_scale + 1)) - 8 * sbg[window[band]]
            else:
                sent = band < 21
                pre = PRETAB[band] if (preflag and sent) else 0
                s = gain - (((sf[band] if sent else 0) + pre) << (sf_scale + 1))
            delta[j : j + width[band]] = 2.0 ** ((s - 210) / 4)
            j += width[band]
        out.append((delta, block, hdr[3]))
    return out


def cell(q, rule):
    """|θ|/Δ için kutu [lo, hi). q = 0: ölü bölge (işaret bilinmez)."""
    q = q.astype(np.float64)
    if rule == "iso":
        lo = np.where(q == 0, 0.0, np.maximum(q - C_ISO, 0) ** (4 / 3))
        hi = np.where(q == 0, 0.5, (q + 1 - C_ISO) ** (4 / 3))
    else:
        qm1 = np.maximum(q - 1, 0)
        lo = np.where(q == 0, 0.0, (qm1 ** (4 / 3) + q ** (4 / 3)) / 2)
        hi = (q ** (4 / 3) + (q + 1) ** (4 / 3)) / 2
    return lo, hi


def report(path):
    records = read(path)
    steps = decoder_steps(records)
    tops = [np.max(np.nonzero(q)[0]) if np.any(q) else 0 for _, _, q in records]
    top = np.percentile(tops, 99)  # kodlayıcının üst kesimi (katsayı indeksi)
    x, q, cls, dl, xg, dead, edge = [], [], [], [], [], [], []
    skipped = 0
    for (ints, theta, qq), (delta, block, mixed) in zip(records, steps):
        if mixed:
            skipped += 1
            continue
        k = np.arange(576)
        x.append(np.abs(theta) / delta)
        dl.append(delta)
        q.append(np.abs(qq))
        cls.append(np.where(qq != 0, 0, np.where((k <= top) | (block == 2), 1, 2)))
        # Bütün katsayıları sıfır olan bant: LAME ölçek çarpanını "fark etmez" sayıp sonradan değiştirir
        # (best_scalefac_store). Orada yalnızca genel kazancın adımı güvenilir: Δ_genel = 2^((gg−210)/4).
        width = ints[16 + SFBMAX : 16 + 2 * SFBMAX]
        zero_band = np.zeros(576, bool)
        j = 0
        for w in width:
            if w <= 0 or j >= 576:
                break
            zero_band[j : j + w] = not np.any(qq[j : j + w])
            j += w
        dead.append(zero_band)
        xg.append(np.abs(theta) / 2.0 ** ((ints[4] - 210) / 4))
        edge.append(k == ints[10])  # max_nonzero_coeff: LAME bu indeksi çoğu zaman sıfırlar
    x, q, cls, dl, xg, dead, edge = map(np.concatenate, (x, q, cls, dl, xg, dead, edge))
    print(f"\n== {path}: {len(records)} granül-kanal (karışık blok atlandı: {skipped}), üst kesim indeksi ≈ {top:.0f}")
    for name, m in (("A (q≠0, kanıtlı)", cls == 0), ("B (q=0, kesimin altı)", cls == 1), ("C (kesimin üstü)", cls == 2)):
        if not m.any():
            continue
        cover = []
        for rule in ("iso", "lame"):
            lo, hi = cell(q[m], rule)
            cover.append(np.mean((x[m] >= lo - 1e-6) & (x[m] < hi + 1e-6)) * 100)
        print(f"  {name:22s} %{m.mean() * 100:5.1f} | kapsama ISO kuralı %{cover[0]:7.3f} | LAME kuralı %{cover[1]:7.3f}")
    b = cls == 1
    over = b & (x >= 0.5)
    if over.any():
        print(f"  B ihlali %{over.sum() / b.sum() * 100:.3f}: |θ|/Δ medyan {np.median(x[over]):.2f}, "
              f"%99'luk {np.percentile(x[over], 99):.2f}, en büyük {x[over].max():.2f}")
        for widen in (0.75, 1.0, 1.5, 2.0):
            print(f"    kutu |θ| < {widen}·Δ olsaydı B kapsaması %{np.mean(x[b] < widen) * 100:.3f}")
        print(f"    ihlallerin %{np.mean(dead[over]) * 100:.2f}'i bütün katsayıları sıfır olan bantta, "
              f"{np.sum(over & ~dead & edge)} tanesi max_nonzero_coeff indeksinde, kalan {np.sum(over & ~dead & ~edge)}")
    live, gone = b & ~dead, b & dead
    if live.any() and gone.any():
        print(f"  B, bant kuralıyla: canlı bantta |θ| < Δ/2 %{np.mean(x[live] < 0.5) * 100:.4f} | "
              f"sıfır bantta |θ| < Δ_genel/2 %{np.mean(xg[gone] < 0.5) * 100:.4f}")
    r = np.where(q == 0, 0.0, q.astype(np.float64) ** (4 / 3))
    err = ((x - r) * dl) ** 2  # fiziksel birimde (duyulan hata Δ'ya bölünmez)
    print("  hata enerjisi: " + ", ".join(f"{n} %{err[cls == c].sum() / err.sum() * 100:.1f}" for n, c in (("A", 0), ("B", 1), ("C", 2)))
          + f" | katsayı alanı SNR {10 * np.log10(np.sum((x * dl) ** 2) / err.sum()):.1f} dB")
    # Seviye 0 üst sınırı: her q değeri için θ/Δ'nın gerçek ortalaması (kâhin) ile çözücü noktası.
    a = q > 0
    centroid = np.zeros(a.sum())
    for value in np.unique(q[a]):
        sel = q[a] == value
        centroid[sel] = x[a][sel].mean()
    w = dl**2
    dec = np.sum((x[a] - r[a]) ** 2 * w[a]) + np.sum(x[b] ** 2 * w[b])
    cen = np.sum((x[a] - centroid) ** 2 * w[a]) + np.sum(x[b] ** 2 * w[b])
    print(f"  Seviye 0 üst sınırı (kâhin ağırlık merkezi): toplam hatada {10 * np.log10(dec / cen):.2f} dB")


if __name__ == "__main__":
    for p in sys.argv[1:]:
        report(p)
