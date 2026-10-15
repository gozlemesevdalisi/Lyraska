"""R2 sonuç tabloları: r2_degerlendir.py'nin JSON çıktılarını VERITAS.md için Markdown tablolarına çevirir.
Kullanım: python r2_tablo.py <çıktı.json> […]  (aynı ad birden çok dosyada varsa sonuncusu alınır)."""
import json
import sys

ORDER = ["pop", "piyano", "akustik", "elektronik", "orkestra", "vurmali"]


def tr(x):
    return f"{x:.3f}".replace(".", ",")


def tr1(x, sign=True):
    return (f"{x:+.2f}" if sign else f"{x:.2f}").replace(".", ",")


def main(paths):
    rows = {}
    for p in paths:
        for r in json.load(open(p)):
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


if __name__ == "__main__":
    main(sys.argv[1:])
