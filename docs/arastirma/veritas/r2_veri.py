"""R2 veri hazırlığı: yamalı LAME dökümlerini `deney.Track` ile çözücü görünümüne çevirip sıkışık .npz yazar.

Her parça için: theta (gerçek), r (çözücü), lo/hi (kutular), delta/dglob (adımlar), q, zero_band, blocks, widths, ms,
rate. Ses dosyası yazılmaz.
Kullanım: VERITAS_LAME=… python r2_veri.py <döküm klasörü> <çıkış klasörü> [iş sayısı]"""
import glob
import os
import sys
from multiprocessing import Pool

import numpy as np

import deney as D


def convert(args):
    src, dst = args
    if os.path.exists(dst):
        return dst, "var"
    tr = D.Track(src)
    np.savez(
        dst,
        theta=tr.theta.astype(np.float32),
        r=tr.r.astype(np.float32),
        lo=tr.lo.astype(np.float32),
        hi=tr.hi.astype(np.float32),
        delta=tr.delta.astype(np.float32),
        dglob=tr.dglob.astype(np.float32),
        q=tr.q.astype(np.int16),
        zero_band=tr.zero_band,
        blocks=tr.blocks.astype(np.int8),
        widths=tr.widths.astype(np.int16),
        ms=tr.ms,
        rate=tr.rate,
    )
    return dst, f"{tr.g} granül"


if __name__ == "__main__":
    src_dir, out_dir = sys.argv[1], sys.argv[2]
    jobs = int(sys.argv[3]) if len(sys.argv) > 3 else 3
    os.makedirs(out_dir, exist_ok=True)
    tasks = [(f, os.path.join(out_dir, os.path.basename(f)[:-4] + ".npz")) for f in sorted(glob.glob(os.path.join(src_dir, "*.bin")))]
    with Pool(jobs) as pool:
        for dst, info in pool.imap_unordered(convert, tasks):
            print(os.path.basename(dst), info, flush=True)
