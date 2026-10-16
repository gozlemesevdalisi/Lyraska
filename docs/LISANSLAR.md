# Dışarıdan alınan varlıklar ve lisansları

Programa kod dışında eklenen veri ve varlıkların (yazı tipleri, profiller, görseller) kaynağı ve lisansı burada
kayıtlıdır. Her yeni varlığın lisansı eklenmeden önce doğrulanır ve buraya yazılır (bkz. `CLAUDE.md`, lisans
kuralları). Kod bağımlılıklarının lisanslarını CI denetler (`npm run check:licenses`, `cargo deny`).

## Inter yazı tipi

- **Ne için:** arayüzün yazı tipi (başlıklar, düğmeler, paneller). Büyük boyutlarda yazı tipinin "Display"
  kesimi (optik boyut ekseni) kendiliğinden devreye girer.
- **Kaynak:** The Inter Project Authors, <https://github.com/rsms/inter>
- **Paket:** `@fontsource-variable/inter` 5.3.0 (npm). Yalnızca `opsz.css` kullanılır: düz stil, ağırlık ve
  optik boyut eksenleri. Yazı tipi dosyaları programın içine paketlenir; çalışırken internete bağlanılmaz.
- **Lisans:** SIL Open Font License 1.1 (OFL-1.1). Ticari kullanıma açıktır. Yazı tipi programla birlikte
  dağıtılabilir; tek başına satılamaz, değiştirilirse "Inter" adıyla dağıtılamaz. Lyraska yazı tipini
  değiştirmeden kullanır.
- **Lisans metni:** programla birlikte dağıtılır: `public/lisanslar/Inter-OFL-1.1.txt` (paketteki `LICENSE`
  dosyasının kopyası).
- **Doğrulama ve onay:** paketin `package.json` lisans alanı ve `LICENSE` dosyası OFL-1.1; AGPL ya da ticari
  kullanım yasağı yok. OFL-1.1 izin listesinde olmadığı için proje sahibine soruldu, 8 Ekim 2026'da onaylandı;
  `src-tauri/deny.toml` izin listesine eklendi.

## Programa girmeyen araştırma dosyaları

### LAME 3.100 ölçüm yaması

- **Ne için:** Veritas araştırmasında kodlayıcının nicemlediği katsayıları ölçmek
  ([VERITAS.md](VERITAS.md), [arastirma/veritas/](arastirma/veritas/)).
- **Dosya:** `docs/arastirma/veritas/lame-3.100-veritas-dump.patch`. LAME kodu depoya girmez; yama yalnızca
  `libmp3lame/encoder.c`'nin birkaç bağlam satırını içerir. Yamalı LAME depo dışında, yalnızca ölçüm için derlenir;
  programla derlenmez, paketlenmez, dağıtılmaz.
- **Kaynak:** The LAME Project, <https://lame.sourceforge.io> (`lame-3.100.tar.gz`, sha256
  `ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e`)
- **Lisans:** GNU LGPL 2.0 ya da sonrası (`COPYING`: Library General Public License, Version 2; dosya başlıkları "or
  (at your option) any later version"). GPL-3.0 projeyle uyumlu; AGPL ya da ticari kullanım yasağı yok.

### R1 deney müziği ve algısal ölçü

- **Ne için:** Veritas R1 ölçümleri (gerçek enstrüman örnekleriyle çalınan deney müziği; algısal kalite ölçüsü).
  Programa girmez, dağıtılmaz; çalışma klasöründe kurulur ve üretilir.
- **FluidR3_GM ses bankası:** Frank Wen ve katkıda bulunanlar; Ubuntu paketi `fluid-soundfont-gm` 3.1. Lisans: MIT
  (paketin `copyright` dosyası: "I hereby release Fluid under the MIT license"). Parçalar `beste.py` ile programla
  yazılır; hiçbir hazır şarkı kullanılmaz.
- **FluidSynth:** Ubuntu paketi `fluidsynth` 2.3.4; LGPL-2.1 ya da sonrası. Yalnızca çalıştırılır.
- **Zimtohrli:** Google, PyPI `zimtohrli` 0.2.1; Apache-2.0 (kaynak dosya başlığı). Yalnızca ölçüm için.

### R2 öğrenmeli önsel araştırması

- **Ne için:** Veritas R2 ([VERITAS.md](VERITAS.md) bölüm 8): delik katsayıları için koşullu yoğunluk ve akış
  örnekleyici eğitimi. Programa girmez, dağıtılmaz; yalnızca araştırma betiklerinde (`r2_model.py`,
  `r2_degerlendir.py`) kullanılır.
- **PyTorch:** PyPI `torch` 2.14.1 (işlemci kullanımı); lisans BSD-3-Clause (`LICENSE`), paket üst verisi bileşenler
  için Apache-2.0, MIT, BSD-2-Clause ve BSL-1.0 da sayar. AGPL ya da ticari kullanım yasağı yok.
- **Eğitim müziği:** `r2_beste.py` ile programla yazılan rastgele parçalar, yine FluidR3_GM (MIT) ile çalınır. Hazır
  şarkı, MUSDB18, MoisesDB ya da telifli kayıt kullanılmadı. Eğitilen ağırlıklar depoya konmadı (kapı geçilmedi).
