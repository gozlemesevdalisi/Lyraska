# Lyraska mimarisi

Bu belge programın nasıl çalıştığını ve hangi kuralın neyle denetlendiğini anlatır. Amaç: her yeni aşama
sağlam bir zeminde başlasın, hata birikmesin. Kurallar `CLAUDE.md`'dedir; burada **neden** ve **nasıl
denetlendiği** yazar.

## Genel görünüm

Program iki parçadır ve tek bir Windows penceresinde birlikte çalışır:

- **Çekirdek (Rust, `src-tauri/`):** ses, analiz, kütüphane, ayarlar. Ağır işin hepsi burada.
- **Arayüz (React + TypeScript, `src/`):** gösterir ve komut gönderir. Hesap yapmaz, sesi işlemez.

İkisi Tauri komutlarıyla konuşur: arayüz `invoke("komut", { … })` çağırır, çekirdek yanıtı JSON olarak döner.

```
Arayüz (WebView2)                         Çekirdek (Rust)
───────────────────                       ──────────────────────────────────────────────
PlayerScreen ── usePlayback ── invoke ──▶ commands.rs (ince) ──▶ audio::player
     │          useLibrary                                    ──▶ library::service
     │          useSongMap                                    ──▶ analysis (önbellek)
     └─ sahneler ─ useVisualFeed ─ her karede visual_frame ──▶ visual_bridge
```

## İş parçacıkları

| İş parçacığı            | Ne yapar                                         | Kural                                      |
| ----------------------- | ------------------------------------------------ | ------------------------------------------ |
| Tauri komutları         | Arayüzün isteklerini karşılar                    | Kısa sürer; ağır işi arka plana verir      |
| Çözme (`player.rs`)     | Dosyayı çözer, boşluksuz geçiş, hız dönüştürme   | Halka tampona (rtrb) yazar                 |
| **Ses çıkışı**          | Halka tampondan okur, EQ, eşitleme, aygıta yazar | **Gerçek zamanlı:** ayırma/kilit/panik yok |
| Analiz (şarkı açılınca) | Spektrum, seviye, vuruş, şarkı haritası, LUFS    | Sonuç önbelleğe yazılır                    |
| Hızlı ölçüm             | Yalnızca ses yüksekliği (EBU R128), FFT'siz      | İlk sesten önce bitsin diye ayrı ve hızlı  |
| Arka plan analizi       | Kütüphaneyi düşük öncelikle analiz eder          | Çalan → sıradaki → geri kalan              |
| Kütüphane taraması      | Klasörleri tarar, etiketleri okur                | Değişmeyeni atlar                          |

## Ses yolu

```
dosya ─▶ symphonia (çözme) ─▶ f64 ─▶ boşluksuz kırpma ─▶ rubato (aygıt hızına) ─▶ halka tampon
      ─▶ [ses çıkışı] kulaklık düzeltmesi ─▶ ekolayzer ─▶ bas ─▶ eşitleme × ekolayzer koruması
      ─▶ geçiş (fade) ─▶ taşma koruması (sınırlayıcı) ─▶ f32 ─▶ WASAPI
```

- İç işlem 64-bit kayan noktadır; dönüşüm yalnızca çözme girişinde ve aygıt çıkışında yapılır.
- EQ, kulaklık ve eşitleme ayarları ses iş parçacığına kilitsiz (atomik) iletilir; değişiklik yumuşakça uygulanır.
- **Ses yüksekliği eşitlemesi** (`audio/loudness.rs`, `audio/normalize.rs`):
  - Her şarkının EBU R128 yüksekliği (LUFS) ve gerçek tepesi (dBTP) analizde ölçülür, önbellekte saklanır.
    Önbellekte yoksa şarkı açılınca ayrı bir hızlı ölçüm yapılır.
  - Şarkı −14 LUFS'e getirilir; yükseltme gerçek tepeyi −1 dBTP'nin üstüne çıkarmaz.
  - Ekolayzer artık sesi kendisi kısmaz; yalnızca en büyük yükseltmesini bildirir. Eşitlemenin tepeye açtığı
    boşluk yetiyorsa hiç kısılmaz, yetmezse yalnızca eksik kadar kısılır (hızlı iner, yavaş kalkar).
  - Oturumdaki her şarkının akıştaki başlangıç karesi `TrackLevels`'ta tutulur: boşluksuz geçişte kazanç tam
    şarkı sınırında değişir.
  - İlk ses, ölçüm için en fazla 0,6 sn bekler (sessizlik yazılır, şarkı yerinde durur); önbellekteki şarkıda ve
    sarmada beklenmez. Ölçüm yetişmezse tipik bir kayıt (−10 LUFS) varsayılır, ölçüm gelince yavaşça düzelir.
- **Bas motoru** (`audio/bass.rs`; ayarları ekolayzerinkinin içinde, aynı kilitsiz kanaldan):
  - Bas düğmesi (0–18 dB; 12'nin üstü kulüp düzeyi): 100 Hz alçak raf.
  - Derinlik (alt oktav): bas bandının (40–120 Hz) her tam dalgasında işaret değiştiren kare dalga × bandın zarfı,
    dördüncü dereceden 60 Hz süzgeçle yumuşatılır. Bas bandı sesin geri kalanından 40 dB'den kısıksa yumuşakça susar.
  - Küçük hoparlör bası: dizüstü hoparlörü 100 Hz altını çalamaz. O bölge dördüncü dereceden süzülür (80 Hz),
    yerine basın 2., 3. ve 4. harmonikleri (Chebyshev polinomları, tepe zarfıyla) eklenir; alt oktav eklenmez.
  - **Akıllı taşma koruması:** her şarkının bas tepeleri tam analizde ölçülür (`BassPeakMeter`: 3–18 dB raflarda
    şarkının tepesi ne kadar yükseliyor, alt oktav bandının tepesi). Koruma bas için en kötü durumu (rafın tamamı)
    değil bu ölçülen artışı kullanır: çoğu şarkıda tepe vokal ve davuldan gelir, bas yükselince ses kısılmaz.
    Ölçüm önbellekte saklanır; yeni şarkının ilk çalınışında birkaç saniye en kötü durum geçerlidir.
  - Ayar değişince raf ~40 ms'de, küçük hoparlör ve derinlik geçişleri her örnekte yumuşakça uygulanır (tık yok).
- **Bit-perfect (özel mod, ayarla açılır):**
  - Aygıt WASAPI özel modda, şarkının kendi hızında açılır; biçim aygıtın kabul ettiği en yüksek tamsayıdır
    (32 → 24 → 16 bit, `output::IntFormat`). Yeniden örnekleme yoktur.
  - Ses çıkışı kulaklık düzeltmesini, ekolayzeri ve eşitlemeyi atlar (`RenderControls.bit_perfect`). Çözücü
    tamsayıları 2'nin kuvvetine böldüğü için çıkışta aynı kuvvetle çarpılan örnekler bit bit aynıdır.
  - Özel mod yoklamalıdır (olay güdümlü özel mod bazı USB aygıtlarda takılır); arabellek ~100 ms.
  - Aygıt biçimi desteklemiyorsa ya da başka program aygıtı tutuyorsa ses paylaşımlı yoldan çalar; neden sinyal
    yolunda (`SignalPath.notice`) yazar.
  - Boşluksuz geçişte hızı farklı şarkı dönüştürülmez: oturum biter, sıradaki şarkı kendi hızında açılır.
- Ses çıkışı 64-bit örnek verir; aygıtın biçimine (32-bit kayan nokta ya da tamsayı) çevirme yalnızca
  `output`'ta yapılır.

## Görsel yolu

- Şarkı açılınca (ya da önceden, arka planda) analiz edilir: spektrum (60 kare/sn), seviyeler, vuruşlar,
  ölçü başları, bölümler, droplar, enerji. Sonuç `analysis::cache`'te saklanır.
- Görsel Yönetmen (`director`) şarkı haritasından her an için bir not çıkarır (atmosfer, ritim, doku).
- Arayüz her ekran karesinde `visual_frame` ister; `visual_bridge` çalma konumuna (ekran ve aygıt
  gecikmesi telafi edilerek) karşılık gelen kareyi verir. Görseller ses yolundan veri çekmez.

## Arayüz yapısı

- `PlayerScreen` yalnızca parçaları birleştirir:
  - `SceneStage`: tam pencere sahne.
  - `NowPlaying`: büyük başlık.
  - `InfoStack`: drop sayacı, bilgi kartları, sıradaki.
  - `Dock`: şarkı haritası şeridi ve düğmeler.
  - Çekmece: kütüphane, ekolayzer, işaretleme, senkron, ayarlar.
- Durum ve davranış hook'lardadır:
  - `usePlayback`: oynatıcı ve çalma sırası.
  - `useDrawer`: çekmece ve Esc.
  - `useScene`: sahne seçimi ve 1–4 tuşları.
  - `useDropToLibrary`: sürükle-bırak.
  - `useIdle`: sinema görünümü.
  - `useSongMap`: çalan şarkının haritası.
- Hesaplar saf fonksiyonlardadır (`src/lib/`) ve ayrı ayrı test edilir.
- Renkler `src/styles/global.css` içindeki `:root` değişkenlerinden gelir.

## Veri ve kalıcılık

| Ne                              | Nerede                                    | Sürüm/değişiklik nasıl yönetilir                           |
| ------------------------------- | ----------------------------------------- | ---------------------------------------------------------- |
| Kütüphane ve analiz önbelleği   | `library.sqlite3` (uygulama veri klasörü) | `storage.rs`: sıralı göç adımları (`user_version`)         |
| Analiz sonuçlarının geçerliliği | aynı dosya, `analyses` tablosu            | `ANALYSIS_VERSION` (hesap değişince artırılır; şu an 4)    |
| Ayarlar                         | `settings.json`                           | Eksik alan varsayılanla dolar; bozuk dosyada varsayılanlar |
| İşaretler                       | `isaretler/*.json`                        | Dosyada `format` alanı                                     |
| Hata günlüğü                    | `logs/lyraska.log`                        | 1 MB'ta bir yedeklenir                                     |

## Kurallar ve onları denetleyenler

Her kural ya bir testle ya da CI'daki bir adımla denetlenir. Elle hatırlanması gereken kural yoktur.

| Kural                                                        | Denetleyen                                                       |
| ------------------------------------------------------------ | ---------------------------------------------------------------- |
| Ses çıkışında bellek ayırma yok                              | `audio::render` testi `ses_yolu_bellek_ayirmaz`                  |
| Ses yüksekliği ölçümü EBU R128'e uyar                        | `audio::loudness` testleri (EBU Tech 3341 durumları 1–5)         |
| Eşitleme ve ekolayzer kapalıyken ses bit bit aynı            | `audio::render` ve `audio::normalize` testleri                   |
| Bit-perfect'te dosyadaki tamsayılar aygıta aynen gider       | `audio::player` testi `bit_perfect_zinciri_…`, `output` testleri |
| Ses yolu ≤ −140 dB bozulma + gürültü ekler                   | `audio::render` testi `ses_yolu_bozulma_ve_gurultu_eklemez`      |
| Bas düğmesi bası yükseltir, ortaya dokunmaz, tık yok         | `audio::bass` testleri                                           |
| Bas motorunun korumaya bildirdiği pay tepeleri karşılar      | `audio::bass` testi `yukseltme_payi_tepeleri_karsilar`           |
| Ölçülen bas tepeleri gerçek artışı karşılar, gereksiz kısmaz | `audio::bass` testi `olculen_bas_tepeleri_…`                     |
| Yeniden örneklemede Nyquist üstü ≥ 140 dB bastırılır         | `audio::resample` testleri                                       |
| Windows'a özel kod (WASAPI) temiz                            | Clippy, `--target x86_64-pc-windows-gnu` (yerel) ve Windows CI   |
| Kütüphane kodunda `unwrap`/`expect`/`panic!` yok             | Clippy (`Cargo.toml` `[lints.clippy]`), CI'da `-D warnings`      |
| `unsafe` yok                                                 | `unsafe_code = "deny"`                                           |
| Rust ↔ arayüz veri tipleri aynı                              | ts-rs ile üretilir; CI "Arayüz tipleri güncel mi" adımı          |
| Arayüzün çağırdığı komut ve argümanlar çekirdekte var        | `npm run check:commands` (CI)                                    |
| Veritabanı yapısı değişince eski dosyalar güncellenir        | `storage.rs` testleri (göç, eski dosya, yeni sürüm dosyası)      |
| Analiz hesabı değişince eski sonuçlar kullanılmaz            | `ANALYSIS_VERSION` ve önbellek testleri                          |
| Hiçbir sahne saniyede 3'ten fazla parlamaz                   | Sahne testleri + `npm run check:scenes` (Windows'ta Edge, CI)    |
| Yasaklı lisans yok                                           | `npm run check:licenses`, `cargo deny` (CI)                      |
| Depoda ses dosyası yok                                       | `npm run check:audio` (CI)                                       |
| Biçim ve lint temiz, tür hatası yok                          | Prettier, ESLint, `tsc`, `cargo fmt`, Clippy (CI)                |

## Nasıl eklenir?

**Yeni bir Tauri komutu**

1. İşi ilgili modülde yazıp test edin (`commands.rs` ince kalır).
2. `commands.rs`'e `#[tauri::command]` fonksiyonunu, `lib.rs`'teki `generate_handler!` listesine adını ekleyin.
3. Dönen ya da alınan yeni bir yapı varsa: `#[serde(rename_all = "camelCase")]` ve
   `#[cfg_attr(test, derive(ts_rs::TS), ts(export))]` ekleyin; `cd src-tauri && cargo test` TypeScript
   karşılığını `src/lib/bindings/` altına üretir.
4. `src/lib/backend.ts`'e çağıran fonksiyonu ekleyin, yeni tipi oradan dışa verin.
5. `npm run check:commands` adların ve argümanların eşleştiğini doğrular.

**Veritabanında yeni sütun ya da tablo**

- `storage.rs`'teki `MIGRATIONS` listesinin **sonuna** yeni adım ekleyin (`ALTER TABLE …`). Eski adımları
  değiştirmeyin. Göç testine yeni adımı ekleyin.

**Yeni bir sahne**

- Hareket ve parlaklığı Görsel Yönetmen'in notundan türetin; parlaklığı yalnızca nabız olaylarına ve
  yumuşak değerlere bağlayın; kendi parlama testini yazın; `scripts/check-scenes.mjs`'e ekleyin.

**Analizde yeni bir hesap**

- `ANALYSIS_VERSION`'ı artırın; sonuç yeni bir alansa önbellek kaydına ekleyin (gerekirse göç adımı).
