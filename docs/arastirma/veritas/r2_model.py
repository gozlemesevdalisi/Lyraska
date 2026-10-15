"""R2 / S1–S2: delik (q = 0) katsayıları için öğrenilmiş koşullu model.

İki baş, aynı gövde (frekans ekseninde genişletilmiş 1-B evrişim, granül bağlamı ±2):
- `yogunluk`: her delik için kesik Gauss karışımı p(u | bağlam), u = θ/s ∈ [−½, ½], s = Δ (canlı bant) ya da
  Δ_genel (sıfır bant). Kutu içinde düzgün dağılımın yoğunluğu 1 olduğundan −log₂ p(u) doğrudan "düzgüne göre
  kazanılan bit"tir (S1: karşılıklı bilgi kestirimi).
- `akis`: akış eşleme (flow matching) ile granülün bütün deliklerini birlikte üreten örnekleyici; her Euler adımından
  sonra kutuya kırpma (P_C'nin katsayı alanındaki hâli). (S2)
Bağlam seçenekleri: `cozucu` (yalnızca çözücünün gördüğü r, kutular, blok, M/S), `kahin` (komşu granüllerin
GERÇEK θ'sı; yalnızca tavan ölçümü için) ve `yok` (komşu granül ve öteki kanal yok; yalnızca hedef granülün kendi
çözülmüş katsayıları ve kutuları: bağlamın katkısını ayırmak için taban). Yalnız işlemci; parametre sayısı ~0,2–0,5 M.
Kullanım: python r2_model.py egit <veri klasörü> <model.pt> --bas yogunluk|akis [--baglam cozucu|kahin]
          [--adim N] [--kanal C] [--yigin B] [--tohum S]"""
import argparse
import glob
import math
import os
import time

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

CTX = 2  # granül bağlamı ±2
N_FEAT = 5 + 5 + 5 + 1 + 3 + 1 + 1 + 1 + 3  # bkz. features()
K_MIX = 4
U_MAX = 0.5


def load_npz(path):
    d = np.load(path)
    return {k: d[k] for k in d.files}


def scale_of(d):
    """s: canlı bantta Δ, bütün q'ları sıfır bantta Δ_genel (kutu yarı genişliğinin iki katı)."""
    return np.where(d["zero_band"], d["dglob"], d["delta"]).astype(np.float32)


class TrackData:
    """Bir parçanın özellik dizileri (2, g, 576) ve hedefleri; bağlam kâhin ise komşu değerler θ'dan."""

    def __init__(self, d, baglam="cozucu"):
        s = scale_of(d)
        self.s = s
        self.baglam = baglam
        self.g = d["r"].shape[1]
        self.hole = d["q"] == 0
        src = d["theta"] if baglam == "kahin" else d["r"]
        self.rho = np.arcsinh(src / s).astype(np.float32)  # çözücü bağlamında yalnızca q'nun işlevi
        self.rho_center = np.arcsinh(d["r"] / s).astype(np.float32)  # hedef granül her zaman çözücünün gördüğü
        self.logs = (np.log2(s) * 0.25).astype(np.float32)
        self.u = np.clip(d["theta"] / s, -U_MAX, U_MAX).astype(np.float32)
        self.zero_band = d["zero_band"]
        self.blocks = d["blocks"].astype(np.int64)
        self.ms = d["ms"].astype(np.float32)
        self.rate = float(d["rate"])



def features(td, ch, t_idx):
    """(B, N_FEAT, 576) özellik tensörü: bağlam granülleri uçlarda kenara kıstırılır."""
    g = td.g
    t_idx = np.asarray(t_idx)
    feats = []
    for dd in range(-CTX, CTX + 1):
        tt = np.clip(t_idx + dd, 0, g - 1)
        feats.append(td.rho_center[ch, tt] if dd == 0 else td.rho[ch, tt])
    for dd in range(-CTX, CTX + 1):
        tt = np.clip(t_idx + dd, 0, g - 1)
        feats.append(td.logs[ch, tt] - (td.logs[ch, t_idx] if dd else 2.5))
    for dd in range(-CTX, CTX + 1):
        tt = np.clip(t_idx + dd, 0, g - 1)
        feats.append(td.hole[ch, tt].astype(np.float32))
    feats.append(td.zero_band[ch, t_idx].astype(np.float32))
    b = td.blocks[ch, t_idx]
    ones = np.ones((len(t_idx), 576), np.float32)
    feats.append(ones * (b == 2)[:, None])
    feats.append(ones * ((b == 1) | (b == 3))[:, None])
    feats.append(ones * (b == 0)[:, None])
    feats.append(ones * td.ms[t_idx][:, None])
    feats.append(ones * (td.rate / 48000.0))
    feats.append(ones * (np.arange(576, dtype=np.float32) / 576.0)[None, :])
    oc = 1 - ch  # öteki kanal (M/S karelerinde S ya da M), yalnızca hedef granülde
    feats.append(td.rho_center[oc, t_idx])
    feats.append(td.logs[oc, t_idx] - td.logs[ch, t_idx])
    feats.append(td.hole[oc, t_idx].astype(np.float32))
    f = np.stack(feats, 1)
    if td.baglam == "yok":  # bağlamsız taban: komşu granüller ve öteki kanal silinir, yalnızca hedef granülün kendisi
        for row in (0, 1, 3, 4, 5, 6, 8, 9, 10, 11, 13, 14, 22, 23, 24):
            f[:, row] = 0.0
    return torch.from_numpy(f)


class Block(nn.Module):
    def __init__(self, c, dil):
        super().__init__()
        self.a = nn.Conv1d(c, c, 3, padding=dil, dilation=dil)
        self.b = nn.Conv1d(c, c, 1)
        self.n = nn.GroupNorm(8, c)

    def forward(self, x):
        return x + self.b(F.gelu(self.a(self.n(x))))


class Net(nn.Module):
    DIL = (1, 2, 4, 8, 16, 32, 64, 1, 2, 4, 8, 16)

    def __init__(self, bas, c=64, extra_in=0, n_blocks=12):
        super().__init__()
        self.bas = bas
        self.stem = nn.Conv1d(N_FEAT + extra_in, c, 3, padding=1)
        self.blocks = nn.ModuleList(Block(c, self.DIL[i % len(self.DIL)]) for i in range(n_blocks))
        out = 3 * K_MIX if bas == "yogunluk" else 1
        self.head = nn.Conv1d(c, out, 1)
        self.time = nn.Linear(16, c) if bas == "akis" else None

    def forward(self, feats, tau=None):
        h = self.stem(feats)
        if self.time is not None:
            freqs = torch.exp(torch.linspace(0, math.log(1000.0), 8, device=feats.device))
            ang = tau[:, None] * freqs[None, :]
            h = h + self.time(torch.cat([ang.sin(), ang.cos()], 1))[:, :, None]
        for b in self.blocks:
            h = b(h)
        return self.head(h)


def mixture_logp(out, u):
    """Kesik Gauss karışımının log yoğunluğu (doğal log) [−½, ½] üzerinde; out (B, 3K, 576), u (B, 576)."""
    B, _, L = out.shape
    o = out.view(B, 3, K_MIX, L)
    logit, mu, logsig = o[:, 0], torch.tanh(o[:, 1]) * U_MAX, o[:, 2].clamp(-7, 2)
    sig = logsig.exp()
    logw = F.log_softmax(logit, 1)
    uu = u[:, None, :]
    log_phi = -0.5 * ((uu - mu) / sig) ** 2 - logsig - 0.5 * math.log(2 * math.pi)
    nd = torch.distributions.Normal(0.0, 1.0)
    mass = nd.cdf((U_MAX - mu) / sig) - nd.cdf((-U_MAX - mu) / sig)
    log_z = torch.logsumexp(logw + torch.log(mass.clamp_min(1e-12)), 1)
    return torch.logsumexp(logw + log_phi, 1) - log_z


def mixture_sample(out, gen):
    """Karışımdan örnek: bileşen seç, kesik Gauss'tan ters CDF ile çek."""
    B, _, L = out.shape
    o = out.view(B, 3, K_MIX, L)
    logit, mu, logsig = o[:, 0], torch.tanh(o[:, 1]) * U_MAX, o[:, 2].clamp(-7, 2)
    sig = logsig.exp()
    nd = torch.distributions.Normal(0.0, 1.0)
    a, b = nd.cdf((-U_MAX - mu) / sig), nd.cdf((U_MAX - mu) / sig)
    logw = F.log_softmax(logit, 1) + torch.log((b - a).clamp_min(1e-12))
    comp = torch.distributions.Categorical(logits=logw.permute(0, 2, 1)).sample()  # (B, L)
    idx = comp[:, None, :]
    mu_c, sig_c = mu.gather(1, idx)[:, 0], sig.gather(1, idx)[:, 0]
    a_c, b_c = a.gather(1, idx)[:, 0], b.gather(1, idx)[:, 0]
    p = a_c + (b_c - a_c) * torch.rand(B, L, generator=gen)
    return (mu_c + sig_c * nd.icdf(p.clamp(1e-6, 1 - 1e-6))).clamp(-U_MAX, U_MAX)


def mixture_mean(out):
    """Kesik karışımın beklenen değeri (en küçük ortalama kare hata; enerjiyi kısar)."""
    B, _, L = out.shape
    o = out.view(B, 3, K_MIX, L)
    logit, mu, logsig = o[:, 0], torch.tanh(o[:, 1]) * U_MAX, o[:, 2].clamp(-7, 2)
    sig = logsig.exp()
    nd = torch.distributions.Normal(0.0, 1.0)
    al, be = (-U_MAX - mu) / sig, (U_MAX - mu) / sig
    mass = (nd.cdf(be) - nd.cdf(al)).clamp_min(1e-12)
    pdf = lambda z: torch.exp(-0.5 * z * z) / math.sqrt(2 * math.pi)
    tmean = mu + sig * (pdf(al) - pdf(be)) / mass
    w = F.softmax(F.log_softmax(logit, 1) + mass.log(), 1)
    return (w * tmean).sum(1).clamp(-U_MAX, U_MAX)


SIGMA0 = 0.3  # akış başlangıç gürültüsü (u birimi)


def flow_sample(net, feats, hole, steps=32, gen=None):
    """Euler ile akış örneği; her adımda kutuya kırpma. Delik olmayan yerler 0 (sonra r ile değiştirilir)."""
    B, L = hole.shape
    x = torch.randn(B, L, generator=gen) * SIGMA0 * hole
    for i in range(steps):
        tau = torch.full((B,), i / steps)
        v = net(torch.cat([feats, x[:, None, :], hole[:, None, :]], 1), tau)[:, 0]
        x = ((x + v / steps) * hole).clamp(-U_MAX, U_MAX)
    return x


def load_tracks(folder, baglam, limit=None):
    files = sorted(glob.glob(os.path.join(folder, "*.npz")))
    if limit:
        files = files[:limit]
    return [TrackData(load_npz(f), baglam) for f in files]


def train(args):
    torch.manual_seed(args.tohum)
    np.random.seed(args.tohum)
    torch.set_num_threads(args.is_parcacigi)
    tracks = load_tracks(args.veri, args.baglam)
    index = [(i, ch, t) for i, td in enumerate(tracks) for ch in range(2) for t in range(td.g)]
    print(f"{len(tracks)} parça, {len(index)} granül-kanal; baş {args.bas}, bağlam {args.baglam}", flush=True)
    net = Net(args.bas, args.kanal, extra_in=2 if args.bas == "akis" else 0, n_blocks=args.blok)
    print("parametre", sum(p.numel() for p in net.parameters()), flush=True)
    opt = torch.optim.AdamW(net.parameters(), lr=args.lr, weight_decay=0.01)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=args.lr, total_steps=args.adim, pct_start=0.05)
    rng = np.random.default_rng(args.tohum)
    gen = torch.Generator().manual_seed(args.tohum)
    t0 = time.time()
    run = 0.0
    for step in range(1, args.adim + 1):
        pick = rng.integers(len(index), size=args.yigin)
        by_track = {}
        for p in pick:
            i, ch, t = index[p]
            by_track.setdefault((i, ch), []).append(t)
        fl, ul, hl = [], [], []
        for (i, ch), ts in by_track.items():
            td = tracks[i]
            fl.append(features(td, ch, ts))
            ul.append(torch.from_numpy(td.u[ch, ts]))
            hl.append(torch.from_numpy(td.hole[ch, ts].astype(np.float32)))
        feats, u, hole = torch.cat(fl), torch.cat(ul), torch.cat(hl)
        if args.bas == "yogunluk":
            out = net(feats)
            nll = -mixture_logp(out, u)
            loss = (nll * hole).sum() / hole.sum()
            shown = loss.item() / math.log(2)
        else:
            x0 = torch.randn(u.shape, generator=gen) * SIGMA0
            tau = torch.rand(u.shape[0], generator=gen)
            xt = (1 - tau[:, None]) * x0 + tau[:, None] * u
            xt = xt * hole
            v = net(torch.cat([feats, xt[:, None, :], hole[:, None, :]], 1), tau)[:, 0]
            loss = (((v - (u - x0)) ** 2) * hole).sum() / hole.sum()
            shown = loss.item()
        opt.zero_grad()
        loss.backward()
        nn.utils.clip_grad_norm_(net.parameters(), 1.0)
        opt.step()
        sched.step()
        run = 0.98 * run + 0.02 * shown if step > 1 else shown
        if step % 50 == 0 or step == args.adim:
            print(f"adım {step:5d}  kayıp {run:.4f}  ({time.time() - t0:.0f} s)", flush=True)
    torch.save({"state": net.state_dict(), "bas": args.bas, "baglam": args.baglam, "kanal": args.kanal, "blok": args.blok}, args.model)
    print("kaydedildi", args.model, flush=True)


def load_model(path):
    ck = torch.load(path, map_location="cpu")
    net = Net(ck["bas"], ck["kanal"], extra_in=2 if ck["bas"] == "akis" else 0, n_blocks=ck.get("blok", 12))
    net.load_state_dict(ck["state"])
    net.eval()
    return net, ck


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("komut", choices=["egit"])
    ap.add_argument("veri")
    ap.add_argument("model")
    ap.add_argument("--bas", default="yogunluk", choices=["yogunluk", "akis"])
    ap.add_argument("--baglam", default="cozucu", choices=["cozucu", "kahin", "yok"])
    ap.add_argument("--adim", type=int, default=3000)
    ap.add_argument("--kanal", type=int, default=64)
    ap.add_argument("--blok", type=int, default=12)
    ap.add_argument("--yigin", type=int, default=64)
    ap.add_argument("--lr", type=float, default=2e-3)
    ap.add_argument("--tohum", type=int, default=0)
    ap.add_argument("--is-parcacigi", type=int, default=4)
    a = ap.parse_args()
    if a.komut == "egit":
        train(a)
