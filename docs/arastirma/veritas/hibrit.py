"""MP3 Layer III hibrit süzgeç bankası (ISO 11172-3): çözücünün sentezi S ve onun eşleniği Sᵀ.

Katsayılar granül sırasında, kanal başına (G, 576) dizisidir; LAME dökümündeki sırayla (kısa blokta bit akışı
sırası: ölçek bandı, pencere, frekans). Sentez: kısa blok yeniden sıralama → (uzun blokta) örtüşme giderme
kelebekleri → IMDCT + pencere + üst üste ekleme → frekans çevirme → 32 bantlı polifaz sentez.
Polifaz penceresi D, LAME kaynağındaki mpglib/tabinit.c'den çalışma anında okunur (depoya kopyalanmaz)."""
import re

import numpy as np

CI = np.array([-0.6, -0.535, -0.33, -0.185, -0.095, -0.041, -0.0142, -0.0037])
CS = 1 / np.sqrt(1 + CI**2)
CA = CI / np.sqrt(1 + CI**2)


def read_window(tabinit_c):
    """ISO 11172-3 sentez penceresi D (512). mpglib/tabinit.c'deki tablo, simetrik ve pürüzsüz ilk örnek süzgecin
    (p[512 − i] = p[i]) ilk 257 değeridir; D her 64 örnekte işaret değiştirir: D[i] = (−1)^⌊i/64⌋ · p[i].
    Doğrulama: ISO analiziyle birlikte yeniden kurma hatası −84,5 dB; S·Sᵀ = I/32."""
    txt = open(tabinit_c).read()
    body = txt[txt.index("dewin[512]") : txt.index("};", txt.index("dewin[512]"))]
    vals = [float(v) for v in re.findall(r"-?\d+\.\d+", body)]
    assert len(vals) == 257
    p = np.zeros(512)
    p[:257] = vals
    p[257:] = p[1:256][::-1]
    return p * (-1.0) ** (np.arange(512) // 64)


def _imdct_matrices():
    i = np.arange(36)[:, None]
    k = np.arange(18)[None, :]
    long_m = np.cos(np.pi / 72 * (2 * i + 1 + 18) * (2 * k + 1))  # (36, 18)
    i = np.arange(12)[:, None]
    k = np.arange(6)[None, :]
    short_m = np.cos(np.pi / 24 * (2 * i + 1 + 6) * (2 * k + 1))  # (12, 6)
    n = np.arange(36)
    w = np.zeros((4, 36))
    w[0] = np.sin(np.pi / 36 * (n + 0.5))
    w[1, :18] = w[0, :18]
    w[1, 18:24] = 1
    w[1, 24:30] = np.sin(np.pi / 12 * (n[24:30] - 18 + 0.5))
    w[3, 6:12] = np.sin(np.pi / 12 * (n[6:12] - 6 + 0.5))
    w[3, 12:18] = 1
    w[3, 18:] = w[0, 18:]
    ws = np.sin(np.pi / 12 * (np.arange(12) + 0.5))
    return long_m, short_m, w, ws


LONG_M, SHORT_M, WIN, WIN_S = _imdct_matrices()
N_MAT = np.cos((16 + np.arange(64))[:, None] * (2 * np.arange(32)[None, :] + 1) * np.pi / 64)  # (64, 32)


def short_order(widths):
    """Kısa blok: bit akışı sırası → (alt bant, pencere, 6) sırası için indeks dizisi."""
    perm = np.zeros(576, int)
    pos = [0, 0, 0]  # pencere başına frekans sayacı
    j = 0
    win = 0
    for w in widths:
        if w <= 0 or j >= 576:
            break
        for _ in range(w):
            f = pos[win]
            perm[(f // 6) * 18 + win * 6 + f % 6] = j
            pos[win] += 1
            j += 1
        win = (win + 1) % 3
    return perm


def to_subband(coef, blocks, widths):
    """(G, 576) → (G, 32, 18): yeniden sıralama ve uzun blokta kelebekler."""
    g = coef.shape[0]
    out = np.empty((g, 576))
    for t in range(g):
        x = coef[t]
        if blocks[t] == 2:
            x = x[short_order(widths[t])]
        else:
            x = x.copy()
            for sb in range(1, 32):
                a = x[18 * sb - 1 - np.arange(8)].copy()
                b = x[18 * sb + np.arange(8)].copy()
                x[18 * sb - 1 - np.arange(8)] = a * CS - b * CA
                x[18 * sb + np.arange(8)] = b * CS + a * CA
        out[t] = x
    return out.reshape(g, 32, 18)


def from_subband(sub, blocks, widths):
    """to_subband'ın eşleniği (G, 32, 18) → (G, 576)."""
    g = sub.shape[0]
    x_all = sub.reshape(g, 576)
    out = np.empty((g, 576))
    for t in range(g):
        x = x_all[t].copy()
        if blocks[t] == 2:
            y = np.empty(576)
            y[short_order(widths[t])] = x
            out[t] = y
        else:
            for sb in range(1, 32):
                a = x[18 * sb - 1 - np.arange(8)].copy()
                b = x[18 * sb + np.arange(8)].copy()
                x[18 * sb - 1 - np.arange(8)] = a * CS + b * CA
                x[18 * sb + np.arange(8)] = b * CS - a * CA
            out[t] = x
    return out


def imdct(sub, blocks):
    """(G, 32, 18) → alt bant örnekleri (G*18, 32): IMDCT, pencere, üst üste ekleme, frekans çevirme."""
    g = sub.shape[0]
    y = np.zeros((g + 1, 32, 18))
    prev = np.zeros((32, 18))
    for t in range(g):
        if blocks[t] == 2:
            z = np.zeros((32, 36))
            for w in range(3):
                seg = sub[t][:, w * 6 : w * 6 + 6] @ SHORT_M.T * WIN_S  # (32, 12)
                z[:, 6 + 6 * w : 18 + 6 * w] += seg
        else:
            z = sub[t] @ LONG_M.T * WIN[blocks[t]]  # (32, 36)
        y[t] = z[:, :18] + prev
        prev = z[:, 18:]
    y = y[:g]
    y[:, 1::2, 1::2] *= -1  # frekans çevirme
    return y.transpose(0, 2, 1).reshape(g * 18, 32)


def imdct_adjoint(s, blocks):
    g = len(blocks)
    y = s.reshape(g, 18, 32).transpose(0, 2, 1).copy()
    y[:, 1::2, 1::2] *= -1
    sub = np.zeros((g, 32, 18))
    for t in range(g):
        z = np.zeros((32, 36))
        z[:, :18] = y[t]
        if t + 1 < g:
            z[:, 18:] = y[t + 1]
        if blocks[t] == 2:
            for w in range(3):
                sub[t][:, w * 6 : w * 6 + 6] = (z[:, 6 + 6 * w : 18 + 6 * w] * WIN_S) @ SHORT_M
        else:
            sub[t] = (z * WIN[blocks[t]]) @ LONG_M
    return sub


def polyphase(s, d):
    """Alt bant örnekleri (T, 32) → PCM (T*32,). ISO sentezi: V = N·S, U ve W pencerelemesi."""
    t_n = s.shape[0]
    v = s @ N_MAT.T  # (T, 64)
    vp = np.vstack([np.zeros((16, 64)), v])  # geçmiş sıfır
    out = np.zeros((t_n, 32))
    for i in range(8):
        a = vp[16 - 2 * i : 16 - 2 * i + t_n, :32]  # V_{t-2i}[j]
        b = vp[16 - 2 * i - 1 : 16 - 2 * i - 1 + t_n, 32:]  # V_{t-2i-1}[32+j]
        out += a * d[64 * i : 64 * i + 32] + b * d[64 * i + 32 : 64 * i + 64]
    return out.reshape(-1)


def polyphase_adjoint(pcm, d):
    t_n = len(pcm) // 32
    out = pcm[: t_n * 32].reshape(t_n, 32)
    vp = np.zeros((t_n + 16, 64))
    for i in range(8):
        vp[16 - 2 * i : 16 - 2 * i + t_n, :32] += out * d[64 * i : 64 * i + 32]
        vp[16 - 2 * i - 1 : 16 - 2 * i - 1 + t_n, 32:] += out * d[64 * i + 32 : 64 * i + 64]
    return vp[16:] @ N_MAT


def synth(coef, blocks, widths, d):
    return polyphase(imdct(to_subband(coef, blocks, widths), blocks), d)


def synth_adjoint(pcm, blocks, widths, d):
    return from_subband(imdct_adjoint(polyphase_adjoint(pcm, d), blocks), blocks, widths)
