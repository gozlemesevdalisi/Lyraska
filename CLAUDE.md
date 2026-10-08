# CLAUDE.md — Lyraska

Bu dosya, bu depoda çalışan Claude oturumları için kalıcı talimatlardır.
Her oturumun başında okunur. Burada yazan kurallar tartışmaya açık değildir;
değişmesi gerekiyorsa önce proje sahibine sorulur.

## Proje sahibiyle çalışma

- Proje sahibi yazılımcı değildir. Kodu Claude yazar; sahibi test eder ve karar verir.
- Sahibiyle **her zaman Türkçe**, sade ve adım adım konuşulur. Teknik terim gerekiyorsa kısaca açıklanır.
- **Hiçbir teknik iş sahibine bırakılmaz.** Komut çalıştırmak, ayar yapmak, dosya düzenlemek Claude'un işidir.
  Sahibinden yalnızca GitHub arayüzünde bir butona basmak gibi Claude'un yapamadığı işler istenir;
  o zaman da hangi sayfada hangi butona basılacağı tam olarak yazılır.
- Sahibine karar sorulacaksa seçenekler ve Claude'un önerisi birlikte sunulur.
- Test için sahibine her zaman sabit indirme linki verilir:
  https://github.com/gozlemesevdalisi/Lyraska/releases/download/test-surumu/Lyraska-Kurulum.exe

## Ürün

Adı **Lyraska** (lir çalgısı ve Lyra takımyıldızından). Depo: `gozlemesevdalisi/Lyraska`.

Görselleri şarkıyı **önceden bilen**, tamamen **çevrimdışı** çalışan bir Windows müzik çalar.

- Şarkı çalmadan önce analiz edilir: beat, ölçü, bölümler, drop, enerji → "şarkı haritası".
- **Görsel Yönetmen** görselleri üç katmanda koreografe eder: **atmosfer**, **ritim**, **doku**.
- Profesyonel ses: parametrik EQ, AutoEq kulaklık profilleri, loudness (EBU R128), crossfeed, bit-perfect mod.
- Nostalji modları (eski araba teybi, VU ibreleri vb.) **özgün tasarımdır**.
  **Pioneer adı, logosu, yazı tipi veya herhangi bir varlığı asla kullanılmaz**; başka markaların da.
- Program hiçbir zaman internete bağlanmak zorunda değildir. Telemetri yoktur.

Yol haritası ve kalite hedefleri: [docs/ROADMAP.md](docs/ROADMAP.md).

## Teknik kararlar (sabit)

| Konu            | Karar                                                        |
| --------------- | ------------------------------------------------------------ |
| Uygulama kabuğu | Tauri 2                                                      |
| Ses motoru      | Rust; çözme `symphonia`, Windows çıkışı `wasapi` (wasapi-rs) |
| İç ses işleme   | 64-bit kayan nokta (`audio::Sample = f64`)                   |
| Analiz          | Rust                                                         |
| Arayüz          | React + TypeScript + Vite                                    |
| Görseller       | WebGL2                                                       |
| Veri            | SQLite                                                       |
| Platform        | Önce Windows; macOS/Linux v1.0'dan sonra                     |
| Lisans          | GPL-3.0-only                                                 |

## Mimari

```
lyraska/
├── src/                      # Arayüz (React + TypeScript)
│   ├── components/           # React bileşenleri (oynatıcı ekranı, nokta matris vb.)
│   ├── hooks/                # usePlayer, useLibrary, useEqualizer, useHeadphone, useMarker (işaretleme),
│   │                         # useVisualFeed (görsel verisi)
│   ├── lib/                  # Saf yardımcılar (format, meter, dotFont, vu, sky, queue), Rust köprüsü (backend.ts),
│   │                         # arayüz hatalarını günlüğe yazma (errorReporting.ts),
│   │                         # WebGL2 çizimi (skyRenderer.ts)
│   └── styles/               # CSS; renkler :root değişkenlerinde
├── src-tauri/                # Rust çekirdeği
│   ├── src/
│   │   ├── lib.rs            # Tauri kurulumu, komut kaydı
│   │   ├── commands.rs       # Arayüzün çağırdığı ince Tauri komutları
│   │   ├── audio/            # Ses motoru
│   │   │   ├── decode.rs     #   symphonia ile çözme → f64 örnekler, etiketler
│   │   │   ├── gapless.rs    #   boşluksuz çalma: MP4/AAC kodlayıcı dolgusu (iTunSMPB, elst)
│   │   │   ├── eq.rs         #   10 bant ekolayzer: taşma düzeltmeli tasarım, kilitsiz ayar, yumuşak geçiş
│   │   │   ├── peq.rs        #   kulaklık düzeltmesi: AutoEq/Equalizer APO profili, parametrik EQ (RBJ)
│   │   │   ├── resample.rs   #   şarkıyı aygıtın hızına çevirme (rubato FFT, yüksek kalite), mono → stereo
│   │   │   ├── limiter.rs    #   taşma koruması: ileriye bakan tepe sınırlayıcı (−0,3 dBFS)
│   │   │   ├── render.rs     #   gerçek zamanlı doldurma, ekolayzer, taşma koruması, duraklatma geçişi
│   │   │   ├── output.rs     #   çıkış soyutlaması; output/wasapi.rs = Windows WASAPI,
│   │   │   │                 #   output/simulated.rs = testlerde aygıtsız sanal çıkış
│   │   │   └── player.rs     #   oturumlar, iş parçacıkları, halka tampon (rtrb), boşluksuz geçiş
│   │   ├── library/          # Müzik kütüphanesi (SQLite, uygulama veri klasöründe)
│   │   │   ├── db.rs         #   kaynak (klasör ya da tek şarkı) ve şarkı tabloları, Türkçe arama
│   │   │   ├── scan.rs       #   paralel etiket okuma, değişmeyeni atlama, silineni çıkarma
│   │   │   └── service.rs    #   arka plan taraması; komutların kullandığı katman
│   │   ├── diagnostics.rs    # Yerel hata ve çökme günlüğü (logs/lyraska.log; gerçek zamanlı yoldan çağrılmaz)
│   │   ├── settings.rs       # Kalıcı ayarlar (settings.json, uygulama veri klasöründe; ekolayzer, kulaklık)
│   │   ├── analysis/         # Şarkı haritası: beat, ölçü, bölüm, drop, enerji
│   │   │   ├── spectrogram.rs #  şarkı açılınca arka planda spektrum (60 kare/sn, 32 bant) ve seviyeler
│   │   │   ├── levels.rs     #   sol/sağ RMS ve tepe; şarkıya göre 0 VU referansı
│   │   │   ├── annotation.rs #   kullanıcının işaretlediği beat/drop anları (JSON, ses içermez)
│   │   │   ├── evaluate.rs   #   analizi işaretlere göre ölçme (F-ölçüsü, parmak gecikmesi)
│   │   │   └── beats.rs      #   başlangıç gücü, tempo ve vuruşlar (dinamik programlama)
│   │   └── visual_bridge/    # Çalma zamanı + analiz → görseller (VisualFrame); gecikme telafisi
│   ├── examples/             # ses_denemesi.rs: gerçek ses aygıtıyla uçtan uca deneme
│   ├── tests/                # Gerçek kodek testleri (formats.rs) ve sentetik test verisi (data/)
│   ├── tauri.conf.json       # Pencere, güvenlik, Windows NSIS kurulum ayarları
│   └── deny.toml             # cargo-deny lisans kuralları
├── scripts/                  # Yardımcı betikler (npm lisans denetimi, depoda ses dosyası denetimi)
├── docs/
│   ├── ROADMAP.md            # Yol haritası
│   ├── ISARETLEME.md         # İşaretleme aracı kılavuzu (proje sahibi için)
│   └── devlog/               # Oturum devir notları (Türkçe)
└── .github/                  # CI, test sürümü yayını, hata kaydı formu
```

Kurallar:

- Ağır iş (çözme, DSP, analiz) **Rust'ta** yapılır; arayüz yalnızca gösterir ve komut gönderir.
- `commands.rs` ince kalır: iş ilgili modülde yapılır, komut yalnızca veriyi arayüze uygun biçime çevirir.
- Rust → arayüz veri yapıları `#[serde(rename_all = "camelCase")]` kullanır ve `src/lib/backend.ts`
  içinde TypeScript karşılığı tutulur.
- Ses çıkış geri çağrısında (real-time thread) **bellek ayırma, kilit bekleme, dosya/ağ erişimi ve panic yoktur**.
- Yeni modüller ilgili klasörün altında alt modül olarak açılır (ör. `audio/decode.rs`).
- Görseller ses yolundan veri çekmez: şarkı önceden analiz edilir (`analysis`), görseller çalma
  konumuna karşılık gelen analiz karesini `visual_bridge` üzerinden okur.
- Oynatıcı akışları (aç, sar, durdur, şarkı sonu) birim testlerinde sanal çıkışla sınanır; gerçek
  aygıt davranışı `examples/ses_denemesi.rs` ile denenir.

## Komutlar

Hepsi depo kökünde çalıştırılır.

| İş                             | Komut                                                                                |
| ------------------------------ | ------------------------------------------------------------------------------------ |
| Bağımlılıkları kur             | `npm ci`                                                                             |
| Programı geliştirme modunda aç | `npm run tauri dev`                                                                  |
| Yalnızca arayüz (tarayıcıda)   | `npm run dev`                                                                        |
| Arayüz testleri                | `npm test`                                                                           |
| Arayüz lint / tür / biçim      | `npm run lint`, `npm run typecheck`, `npm run format:check`                          |
| Biçimlendir                    | `npm run format` ve `cd src-tauri && cargo fmt`                                      |
| Rust testleri                  | `cd src-tauri && cargo test`                                                         |
| Rust lint                      | `cd src-tauri && cargo clippy --all-targets -- -D warnings`                          |
| Lisans denetimi                | `npm run check:licenses` ve `cd src-tauri && cargo deny check licenses bans sources` |
| Depoda ses dosyası yok mu      | `npm run check:audio` (telif: yalnızca `src-tauri/tests/data/` sentetik sesleri)     |
| Windows'a özel kodu denetle    | CI'daki Windows işi (SQLite C kodu yüzünden Linux'tan çapraz denetim yapılamıyor)    |
| Gerçek ses aygıtıyla deneme    | `cd src-tauri && cargo run --example ses_denemesi`                                   |
| Kurulum dosyası (Windows)      | `npm run tauri build`                                                                |

Not: Rust derlemesi için önce `npm run build` ile `dist/` oluşturulmuş olmalıdır.
Linux'ta Tauri derlemek için `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf` gerekir.

## Kod standartları

Genel:

- Kod tanımlayıcıları (değişken, fonksiyon, tür adları) **İngilizce**; yorumlar, belgeler,
  commit mesajları ve kullanıcıya görünen metinler **Türkçe**.
- Çevredeki kodun üslubuna uyulur. Gereksiz soyutlama yapılmaz; ihtiyaç doğunca eklenir.
- Uyarısız derleme zorunludur: CI'da lint uyarıları hata sayılır.

Rust:

- `cargo fmt` ve `cargo clippy -D warnings` temiz olmalı.
- `unsafe` yasaktır (`unsafe_code = "deny"`). WASAPI gibi zorunlu bir durumda yalnızca ilgili modülde,
  `// GÜVENLİK:` yorumuyla gerekçelendirilerek ve proje sahibine bildirilerek açılır.
- Kütüphane kodunda `unwrap()`/`expect()` yerine hata türleri döndürülür (testler hariç).
- İç ses işleme `f64`'tür; dönüşüm yalnızca çözme girişinde ve aygıt çıkışında yapılır.

TypeScript / React:

- `strict` TypeScript; `any` kullanılmaz.
- Fonksiyon bileşenleri ve hook'lar. Hesaplama mantığı saf fonksiyonlara ayrılır ve test edilir.
- Renkler ve tema değerleri `src/styles/global.css` içindeki `:root` değişkenlerinden gelir.
- Prettier biçimi (satır genişliği 100) zorunludur.

Görsel güvenlik (zorunlu):

- **Epilepsi güvenliği**: hiçbir sahne saniyede 3'ten fazla parlama üretmez. Ani tam ekran beyaz/kırmızı
  geçiş yoktur. Faz 2'de bu kural bir "epilepsi güvenli modu" ile denetlenebilir hale gelir.
- İşletim sistemindeki "animasyonları azalt" tercihine (`prefers-reduced-motion`) uyulur.

## Test kuralları

- Her yeni işlev testleriyle birlikte gelir. Hata düzeltmelerinde önce hatayı yakalayan test yazılır.
- Rust: birim testleri ilgili modülde `#[cfg(test)] mod tests` içinde. Ses/analiz algoritmaları için
  bilinen sonuçlu küçük ses örnekleriyle testler (Faz 1'den itibaren `src-tauri/tests/` ve test verisi).
- TypeScript: Vitest + Testing Library; dosya adı `*.test.ts(x)`.
- Kalite hedefleri ölçülebilir testlere bağlanır: beat F-ölçüsü ≥ 0,80; senkron ±20 ms;
  1080p dahili GPU'da 60 fps; 1 saat çalmada sıfır takılma.
- Push'tan önce yerelde şunlar temiz olmalı: biçim, lint, tür denetimi, testler, lisans denetimi.

## Lisans kuralları

- Proje lisansı **GPL-3.0-only** (`LICENSE`). Yeni kaynak dosyalarına lisans başlığı gerekmez.
- **AGPL lisanslı** ve **ticari kullanımı yasaklayan** (NonCommercial, SSPL, BUSL, Commons Clause,
  Elastic, PolyForm Noncommercial vb.) hiçbir bağımlılık, veri seti, yazı tipi, shader veya görsel eklenmez.
- İzin verilen lisanslar `src-tauri/deny.toml` içinde listelidir (MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0,
  LGPL, GPL-3.0 vb.). Listede olmayan bir lisans gerekirse önce proje sahibine sorulur.
- CI iki denetim yapar: `cargo deny check licenses bans sources` ve `npm run check:licenses`.
- Dışarıdan alınan veri ve varlıkların (AutoEq profilleri, MilkDrop presetleri, örnek şarkılar, yazı tipleri)
  lisansı eklemeden önce doğrulanır ve kaynağıyla birlikte `docs/` altında not edilir.
- Marka adları ve varlıkları (özellikle Pioneer) kullanılmaz; nostalji tasarımları özgün çizilir.
- **Ses dosyaları depoya asla girmez** (işaretleme için kullanılan şarkılar dahil). Tek istisna projenin
  ffmpeg ile ürettiği sentetik test sesleri (`src-tauri/tests/data/`). `.gitignore` ve CI (`check:audio`) denetler.

## Git ve yayın akışı

- **Her iş birimi bitince commit + push yapılır.** Büyük işler küçük, anlamlı commit'lere bölünür.
- Commit mesajları Türkçe, ilk satır kısa ve emir kipinde (ör. "Ses motoruna WASAPI çıkışı ekle").
- Geliştirme oturuma atanan dalda yapılır; `main`'e doğrudan push yapılmaz, PR açılır.
- PR'da CI yeşil olmadan birleştirme yapılmaz. Testler atlanmaz, devre dışı bırakılmaz.
- `main` her güncellendiğinde "Test sürümü" iş akışı Windows kurulum dosyasını derler ve
  `test-surumu` Release'ini yeniler. Sabit link yukarıda.
- Sürüm numarası `package.json` içindedir; `src-tauri/Cargo.toml` ile aynı tutulur.

## Oturum sonu: devir notu

Her oturumun sonunda `docs/devlog/YYYY-AA-GG.md` dosyasına (aynı gün ikinci oturumsa sonuna `-2` eklenir)
kısa bir Türkçe devir notu yazılır, commit edilir ve push edilir. Şablon:

```markdown
# Devir notu — GG Ay YYYY

## Bu oturumda yapılanlar

- ...

## Durum

- CI: yeşil / kırmızı (neden)
- Açık PR'lar: ...

## Sıradaki adım

- ...

## Notlar / açık sorular

- ...
```
