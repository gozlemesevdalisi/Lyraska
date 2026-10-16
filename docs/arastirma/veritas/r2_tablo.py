"""R2 sonuç tabloları: r2_degerlendir.py'nin JSON çıktılarını VERITAS.md için Markdown tablolarına çevirir.
Kullanım: python r2_tablo.py <çıktı.json> […] [--sec <yöntem alt dizesi> …] [--ozet]  (aynı parça birden çok dosyada varsa sözlükleri birleştirilir)."""
import json
import sys

ORDER = ["pop", "piyano", "akustik", "elektronik", "orkestra", "vurmali"]


def tr(x):
    return f"{x:.3f}".replace(".", ",")


def tr1(x, sign=True):
    return (f"{x:+.2f}" if sign else f"{x:.2f}").replace(".", ",")


def main(paths, secim=None):
    """secim: yöntem adlarında aranan alt dizeler; verilirse yalnızca eşleşen yöntemler (ve çözücü/kâhin) yazılır."""
    rows = {}
    for p in paths:
        for r in json.load(open(p)):
            if r["ad"] in rows:  # aynı parça birden çok dosyada: sözlükler birleştirilir (yöntem, bit, enerji, eğri)
                for k, v in r.items():
                    if isinstance(v, dict) and isinstance(rows[r["ad"]].get(k), dict):
                        rows[r["ad"]][k].update(v)
                    else:
                        rows[r["ad"]][k] = v
            else:
                rows[r["ad"]] = r
    by_rate = {}
    for ad, r in rows.items():
        by_rate.setdefault(ad.rsplit("_", 1)[1], {})[ad.rsplit("_", 1)[0]] = r
    for rate, parts in sorted(by_rate.items(), key=lambda kv: int(kv[0])):
        names = [n for n in ORDER if n in parts] + sorted(n for n in parts if n not in ORDER)
        print(f"\n### {rate} kbps: MOS (yüksek iyi), parantez içinde SNR farkı (çözücüye göre)\n")
        print("| Yöntem | " + " | ".join(names) + " | Ortalama |")
        print("|" + "---|" * (len(names) + 2))
        methods = list(parts[names[0]]["yöntem"])
        if secim:
            methods = [m for m in methods if m == "çözücü" or m.startswith("kâhin:") or any(x in m for x in secim)]
        for m in methods:
            vals, cells = [], []
            for n in names:
                v = parts[n]["yöntem"].get(m)
                if v is None:
                    cells.append("—")
                    continue
                base = parts[n]["yöntem"]["çözücü"]["SNR"]
                vals.append(v["MOS"])
                cells.append(tr(v["MOS"]) + ("" if m == "çözücü" else f" ({tr1(v['SNR'] - base)})"))
            print(f"| {m} | " + " | ".join(cells) + f" | {tr(sum(vals) / len(vals))} |")
        print(f"\n### {rate} kbps: delik katsayısı başına düzgün dağılıma göre kazanılan bit (ortalama, {len(names)} parça)\n")
        regions = list(parts[names[0]]["bit"]["düzgün"])
        print("| Model | " + " | ".join(regions) + " |")
        print("|" + "---|" * (len(regions) + 1))
        for m in parts[names[0]]["bit"]:
            cells = []
            for reg in regions:
                vs = [parts[n]["bit"][m][reg] for n in names if reg in parts[n]["bit"].get(m, {})]
                cells.append(tr1(sum(vs) / len(vs)) if vs else "—")
            print(f"| {m} | " + " | ".join(cells) + " |")
        if any("enerji hatası" in parts[n] for n in names):
            print(f"\n### {rate} kbps: delik enerjisi kestirimi, bant başına hata (kestirim − gerçek, dB; ortanca [%10, %90]), parça başına\n")
            print("| Kestirici | Bant | " + " | ".join(names) + " |")
            print("|" + "---|" * (len(names) + 2))
            for m in parts[names[0]]["enerji hatası"]:
                for band in ("canlı bant", "sıfır bant"):
                    cells = []
                    for n in names:
                        e = parts[n]["enerji hatası"][m][band]
                        cells.append(f"{tr1(e['ortanca dB'])} [{tr1(e['%10'])}, {tr1(e['%90'])}]")
                    print(f"| {m} | {band} | " + " | ".join(cells) + " |")
        if any("eğri" in parts[n] for n in names):
            print(f"\n### {rate} kbps: bilgi–algı eğrisi (gürültülü kâhin), MOS parça başına\n")
            egri = parts[names[0]]["eğri"]
            print("| Ölçek | α | bit (ort.) | " + " | ".join(names) + " | Ortalama |")
            print("|" + "---|" * (len(names) + 4))
            for i, e in enumerate(egri):
                mos = [parts[n]["eğri"][i]["MOS"] for n in names if "eğri" in parts[n]]
                bits = [parts[n]["eğri"][i]["bit"] for n in names if "eğri" in parts[n]]
                print(f"| {e['ölçek']} | {tr1(e['alpha'], False)} | {tr1(sum(bits) / len(bits))} | " + " | ".join(tr(x) for x in mos) + f" | {tr(sum(mos) / len(mos))} |")


def ozet(paths, secim=None):
    """Grup özeti (K2): verilen dosyalardaki bütün parçalar tek grup; bit ortalamaları ve yöntem başına MOS farkı
    (ortalama, en kötü, en iyi)."""
    rows = []
    for p in paths:
        rows += json.load(open(p))
    names = [r["ad"] for r in rows]
    print(f"\n{len(rows)} parça: " + ", ".join(names))
    print("\n| Model | " + " | ".join(rows[0]["bit"]["düzgün"]) + " |")
    print("|" + "---|" * (len(rows[0]["bit"]["düzgün"]) + 1))
    for m in rows[0]["bit"]:
        cells = []
        for reg in rows[0]["bit"]["düzgün"]:
            vs = [r["bit"][m][reg] for r in rows if reg in r["bit"].get(m, {})]
            cells.append(tr1(sum(vs) / len(vs)) if vs else "—")
        print(f"| {m} | " + " | ".join(cells) + " |")
    print("\n| Yöntem | MOS ort. | fark ort. | en kötü parça | en iyi parça |")
    print("|---|---|---|---|---|")
    methods = list(rows[0]["yöntem"])
    if secim:
        methods = [m for m in methods if m == "çözücü" or m.startswith("kâhin:") or any(x in m for x in secim)]
    for m in methods:
        v = [r["yöntem"][m]["MOS"] for r in rows if m in r["yöntem"]]
        d = [r["yöntem"][m]["MOS"] - r["yöntem"]["çözücü"]["MOS"] for r in rows if m in r["yöntem"]]
        print(f"| {m} | {tr(sum(v) / len(v))} | {tr1(sum(d) / len(d))} | {tr1(min(d))} | {tr1(max(d))} |")
    if "enerji hatası" in rows[0]:
        print("\n| Kestirici | canlı bant ortanca dB (ort., en kötü) | sıfır bant ortanca dB (ort., en kötü) |")
        print("|---|---|---|")
        for m in rows[0]["enerji hatası"]:
            cells = []
            for band in ("canlı bant", "sıfır bant"):
                v = [r["enerji hatası"][m][band]["ortanca dB"] for r in rows]
                cells.append(f"{tr1(sum(v) / len(v))}, {tr1(max(v))}")
            print(f"| {m} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    args = sys.argv[1:]
    secim = None
    if "--sec" in args:
        i = args.index("--sec")
        secim, args = args[i + 1 :], args[:i]
    if "--ozet" in args:
        args.remove("--ozet")
        ozet(args, secim)
    else:
        main(args, secim)
