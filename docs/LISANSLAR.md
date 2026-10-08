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
