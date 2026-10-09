# CLAUDE.md — Lyraska

Bu dosya, bu depoda çalışan Claude oturumları için kalıcı talimatlardır.
Her oturumun başında okunur. Burada yazan kurallar tartışmaya açık değildir;
değişmesi gerekiyorsa önce proje sahibine sorulur.

## Proje sahibiyle çalışma

- Proje sahibi yazılımcı değildir. Kodu Claude yazar; sahibi test eder ve karar verir.
- Sahibiyle **her zaman Türkçe**, sade ve adım adım konuşulur; iş sırasındaki kısa ara bilgilendirmeler de
  Türkçedir. Teknik terim gerekiyorsa kısaca açıklanır.
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

Yol haritası ve kalite hedefleri: [docs/ROADMAP.md](docs/ROADMAP.md). Mimari, iş parçacıkları ve hangi kuralı
neyin denetlediği: [docs/MIMARI.md](docs/MIMARI.md).

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
│   ├── components/           # React bileşenleri: PlayerScreen (sahne tüm pencere; üstünde cam katmanlar:
│   │                         # NowPlaying büyük başlık, InfoStack drop sayacı ve sıradaki, SeekBar şarkı haritası
│   │                         # şeridi; sağdan açılan çekmecede kütüphane, ekolayzer, işaretleme, senkron, ayarlar;
│   │                         # her panel ve sahne bir ErrorBoundary içinde: hata yalnızca o bölümü kapatır)
│   ├── hooks/                # usePlayback (oynatıcı + çalma sırası), useLibrary, useEqualizer, useHeadphone,
│   │                         # useMarker (işaretleme), useSync (ses–görüntü senkronu), useVisualFeed (görsel
│   │                         # verisi), useSongMap (çalan şarkının haritası), useDrawer (çekmece), useScene
│   │                         # (sahne, 1–4), useDropToLibrary (sürükle-bırak), useIdle (sinema görünümü),
│   │                         # useVisualSafe (epilepsi güvenli modu), usePlaybackOptions (ses yüksekliği eşitleme),
│   │                         # useSkyLook (gece göğünün manzarası: göl, korona, karlı vadi), useCover (kapak)
│   ├── lib/                  # Saf yardımcılar (format, meter, dotFont, vu, sky, highway, queue, sync, songMap,
│   │                         # timeline, cover), Rust köprüsü (backend.ts; veri tipleri bindings/ altında
│   │                         # Rust'tan üretilir, elle düzenlenmez), arayüz hatalarını günlüğe yazma (errorReporting.ts), parlama sayacı
│   │                         # (flash.ts), WebGL2 çizimi (gl.ts ortak; skyRenderer.ts, highwayRenderer.ts),
│   │                         # WebGL2 açılamazsa gece göğü için 2D yedek çizim (skyFallback.ts)
│   └── styles/               # CSS; renkler :root değişkenlerinde
├── src-tauri/                # Rust çekirdeği
│   ├── src/
│   │   ├── lib.rs            # Tauri kurulumu, komut kaydı
│   │   ├── commands.rs       # Arayüzün çağırdığı ince Tauri komutları
│   │   ├── audio/            # Ses motoru
│   │   │   ├── decode.rs     #   symphonia ile çözme → f64 örnekler, etiketler, kapak resmi
│   │   │   ├── gapless.rs    #   boşluksuz çalma: MP4/AAC kodlayıcı dolgusu (iTunSMPB, elst)
│   │   │   ├── eq.rs         #   10 bant ekolayzer: taşma düzeltmeli tasarım, kilitsiz ayar, yumuşak geçiş
│   │   │   ├── bass.rs       #   bas düğmesi (raf, 0–18 dB), derinlik (alt oktav), küçük hoparlör bası, bas tepesi ölçümü
│   │   │   ├── punch.rs      #   vuruş: davulun ilk anını güçlendiren dinamik bas rafı (sürekli bas aynen kalır)
│   │   │   ├── biquad.rs     #   ortak ikinci dereceden süzgeçler (RBJ): kulaklık düzeltmesi ve bas kullanır
│   │   │   ├── loudness.rs   #   EBU R128 ses yüksekliği (LUFS) ve gerçek tepe (dBTP) ölçümü
│   │   │   ├── normalize.rs  #   çalarken eşitleme (−14 LUFS) ve ekolayzerin boşluğa göre taşma koruması
│   │   │   ├── peq.rs        #   kulaklık düzeltmesi: AutoEq/Equalizer APO profili, parametrik EQ (RBJ)
│   │   │   ├── resample.rs   #   şarkıyı aygıtın hızına çevirme (rubato FFT, yüksek kalite), mono → stereo
│   │   │   ├── limiter.rs    #   taşma koruması: ileriye bakan tepe sınırlayıcı (0 dBFS)
│   │   │   ├── render.rs     #   gerçek zamanlı doldurma, ekolayzer, eşitleme, taşma koruması, duraklatma geçişi
│   │   │   ├── output.rs     #   çıkış soyutlaması, özel mod tamsayı biçimi; output/wasapi.rs = Windows WASAPI
│   │   │   │                 #   (paylaşımlı; ayarla özel mod = bit-perfect), output/simulated.rs = testlerde
│   │   │   │                 #   aygıtsız sanal çıkış
│   │   │   └── player.rs     #   oturumlar, iş parçacıkları, halka tampon (rtrb), boşluksuz geçiş
│   │   ├── library/          # Müzik kütüphanesi (SQLite, uygulama veri klasöründe)
│   │   │   ├── db.rs         #   kaynak (klasör ya da tek şarkı) ve şarkı tabloları, Türkçe arama, BPM
│   │   │   ├── scan.rs       #   paralel etiket okuma, değişmeyeni atlama, silineni çıkarma
│   │   │   └── service.rs    #   arka plan taraması; komutların kullandığı katman
│   │   ├── storage.rs        # library.sqlite3 şeması ve sıralı göçleri (kütüphane + analiz önbelleği)
│   │   ├── diagnostics.rs    # Yerel hata ve çökme günlüğü (logs/lyraska.log; gerçek zamanlı yoldan çağrılmaz)
│   │   ├── settings.rs       # Kalıcı ayarlar (settings.json; ekolayzer, kulaklık, eşitleme, güvenli mod, ses gecikmesi)
│   │   ├── analysis/         # Şarkı haritası: beat, ölçü, bölüm, drop, enerji
│   │   │   ├── spectrogram.rs #  şarkı açılınca arka planda spektrum (60 kare/sn, 32 bant), seviyeler, LUFS, bas tepeleri
│   │   │   ├── levels.rs     #   sol/sağ RMS ve tepe; şarkıya göre 0 VU referansı
│   │   │   ├── annotation.rs #   kullanıcının işaretlediği beat/drop anları (JSON, ses içermez)
│   │   │   ├── evaluate.rs   #   analizi işaretlere göre ölçme (F-ölçüsü, parmak gecikmesi)
│   │   │   ├── beats.rs      #   başlangıç gücü, tempo ve vuruşlar (dinamik programlama)
│   │   │   ├── structure.rs  #   ölçü başları, bölümler (Foote yeniliği), droplar, enerji eğrisi
│   │   │   ├── cache.rs      #   şarkı haritası önbelleği (SQLite; yol + boyut + değişme zamanı; parça sürümleri)
│   │   │   └── background.rs #   kütüphanenin arka plan analizi (düşük öncelik; çalan → sıradaki → geri kalan)
│   │   ├── director/         # Görsel Yönetmen: şarkı haritasından koreografi (atmosfer, ritim, doku),
│   │   │                     # drop beklentisi/açılımı; nabız hız garantisi (≤ 3/sn, güvenli modda ≤ 1/sn)
│   │   └── visual_bridge/    # Çalma zamanı + analiz → görseller (VisualFrame); gecikme telafisi (ekran +
│   │                         # ses aygıtı), calibration.rs: senkron ölçümü için tıklama kaydı
│   ├── examples/             # ses_denemesi.rs: gerçek ses aygıtıyla uçtan uca deneme
│   ├── tests/                # Gerçek kodek testleri (formats.rs) ve sentetik test verisi (data/)
│   ├── tauri.conf.json       # Pencere, güvenlik, Windows NSIS kurulum ayarları
│   └── deny.toml             # cargo-deny lisans kuralları
├── scripts/                  # Yardımcı betikler (npm lisans denetimi, depoda ses dosyası denetimi, sahne denetimi,
│                             # komut sözleşmesi denetimi)
├── docs/
│   ├── ROADMAP.md            # Yol haritası
│   ├── MIMARI.md             # Mimari ve kuralları denetleyen testler
│   ├── ISARETLEME.md         # İşaretleme aracı kılavuzu (proje sahibi için)
│   ├── LISANSLAR.md          # Dışarıdan alınan varlıklar (yazı tipleri vb.) ve lisansları
│   └── devlog/               # Oturum devir notları (Türkçe)
└── .github/                  # CI, test sürümü yayını, hata kaydı formu
```

Kurallar:

- Ağır iş (çözme, DSP, analiz) **Rust'ta** yapılır; arayüz yalnızca gösterir ve komut gönderir.
- `commands.rs` ince kalır: iş ilgili modülde yapılır, komut yalnızca veriyi arayüze uygun biçime çevirir.
- Rust → arayüz veri yapıları `#[serde(rename_all = "camelCase")]` ve
  `#[cfg_attr(test, derive(ts_rs::TS), ts(export))]` kullanır. TypeScript karşılıkları `cargo test` ile
  `src/lib/bindings/` altına **üretilir**, elle yazılmaz; `src/lib/backend.ts` onları dışa verir. CI üretilenin
  depodakiyle aynı olduğunu, `npm run check:commands` komut adlarının ve argümanlarının eşleştiğini denetler.
- Veritabanı yapısı yalnızca `storage.rs`'teki sıralı göç adımlarıyla değişir (listenin sonuna eklenir).
- Ses çıkış geri çağrısında (real-time thread) **bellek ayırma, kilit bekleme, dosya/ağ erişimi ve panic yoktur**.
- Yeni modüller ilgili klasörün altında alt modül olarak açılır (ör. `audio/decode.rs`).
- Görseller ses yolundan veri çekmez: şarkı önceden analiz edilir (`analysis`), görseller çalma
  konumuna karşılık gelen analiz karesini `visual_bridge` üzerinden okur.
- Analiz sonuçları önbellekte (`analysis/cache.rs`) saklanır. Analiz dört parçadır ve her birinin kendi sürümü
  vardır (`cache::VERSIONS`): spektrum (kareler), ritim (beat, ölçü, bölüm, drop, enerji), ses yüksekliği, bas
  tepeleri. Bir hesabı değiştiren iş **yalnızca o parçanın** sürümünü artırır; kütüphanede yalnızca o parça yeniden
  hesaplanır (ritim karelerden, şarkı çözülmeden). Bütün kütüphaneyi baştan analiz ettiren spektrum sürümü yalnızca
  karelerin kendisi değişince artırılır.
- Sahneler hareket ve parlaklığı Görsel Yönetmen'in notundan (`VisualFrame.director`) türetir; parlaklığı
  yalnızca nabız olaylarına ve yumuşak değerlere bağlar ve kendi parlama testini taşır.
- Oynatıcı akışları (aç, sar, durdur, şarkı sonu) birim testlerinde sanal çıkışla sınanır; gerçek
  aygıt davranışı `examples/ses_denemesi.rs` ile denenir.

## Komutlar

Hepsi depo kökünde çalıştırılır.

| İş                             | Komut                                                                                                 |
| ------------------------------ | ----------------------------------------------------------------------------------------------------- |
| Bağımlılıkları kur             | `npm ci`                                                                                              |
| Programı geliştirme modunda aç | `npm run tauri dev`                                                                                   |
| Yalnızca arayüz (tarayıcıda)   | `npm run dev`                                                                                         |
| Arayüz testleri                | `npm test`                                                                                            |
| Arayüz lint / tür / biçim      | `npm run lint`, `npm run typecheck`, `npm run format:check`                                           |
| Biçimlendir                    | `npm run format` ve `cd src-tauri && cargo fmt`                                                       |
| Rust testleri                  | `cd src-tauri && cargo test` (arayüz veri tiplerini de `src/lib/bindings/`'e üretir)                  |
| Komut sözleşmesi               | `npm run check:commands` (arayüzün çağrıları ↔ çekirdeğin komutları)                                  |
| Rust lint                      | `cd src-tauri && cargo clippy --all-targets -- -D warnings`                                           |
| Lisans denetimi                | `npm run check:licenses` ve `cd src-tauri && cargo deny check licenses bans sources`                  |
| Depoda ses dosyası yok mu      | `npm run check:audio` (telif: yalnızca `src-tauri/tests/data/` sentetik sesleri)                      |
| Sahneler gerçek tarayıcıda     | `npm run check:scenes` (önce `npm run build`; CI'da Windows Edge)                                     |
| Windows'a özel kodu denetle    | `cd src-tauri && cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D warnings` (bkz. not) |
| Gerçek ses aygıtıyla deneme    | `cd src-tauri && cargo run --example ses_denemesi`                                                    |
| Kurulum dosyası (Windows)      | `npm run tauri build`                                                                                 |

Not: Rust derlemesi için önce `npm run build` ile `dist/` oluşturulmuş olmalıdır.
Windows'a özel kodun (WASAPI) Linux'tan denetimi için bir kez `rustup target add x86_64-pc-windows-gnu` ve
`apt-get install gcc-mingw-w64-x86-64` gerekir; CI'daki Windows işi de aynı denetimi Windows'ta yapar.
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
- Kütüphane kodunda `unwrap()`/`expect()`/`panic!` yerine hata türleri döndürülür (testler hariç). Clippy
  denetler (`unwrap_used`, `expect_used`, `panic`); sürüm derlemesi panikte kapanır (`panic = "abort"`).
- Ses çıkış yolunun bellek ayırmadığını `audio::render` testi `ses_yolu_bellek_ayirmaz` ölçer; ses yoluna
  eklenen her işlem bu testin kapsamına alınır.
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
  LGPL, GPL-3.0, yazı tipleri için OFL-1.1 vb.). Listede olmayan bir lisans gerekirse önce proje sahibine sorulur.
- CI iki denetim yapar: `cargo deny check licenses bans sources` ve `npm run check:licenses`.
- Dışarıdan alınan veri ve varlıkların (AutoEq profilleri, MilkDrop presetleri, örnek şarkılar, yazı tipleri)
  lisansı eklemeden önce doğrulanır ve kaynağıyla birlikte `docs/LISANSLAR.md` içinde not edilir. Lisans metni
  dağıtım gerektiriyorsa `public/lisanslar/` altına konur (programla birlikte paketlenir).
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
kısa bir Türkçe devir notu yazılır, commit edilir ve push edilir.

Devir notunun **"Durum"** ve **"Sıradaki adım"** bölümleri oturum sonunu beklemeden **her PR'dan sonra** güncellenir:
her PR, o günün devir notundaki bu iki bölümü kendi içinde günceller (CI durumu, açık PR'lar, biten iş artık
"Sıradaki adım"da durmaz). Bir PR birleştikten sonra değişen durum bir sonraki PR'da yazılır.

Şablon:

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
