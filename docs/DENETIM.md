# Kalite denetimi — 16 Ekim 2026

Proje sahibinin isteği: "Daha derin düşün, daha derin araştır; programa kalite denetimi yap; üst düzey mühendislik ve
matematiksel denetim." Bu belge sürüm **0.0.44** (`main`, `294bd80`) üzerinde yapılan denetimin sonucudur. Hiçbir kod
değiştirilmedi; bulgular düzeltme değil, düzeltme listesidir. Doğrulama haftası kuralı gereği buradaki her madde
"kendi bulunan hata" ya da "iyileştirme önerisi" olarak sıralanmıştır; öncelik proje sahibinindir.

**Yöntem.** (1) Depodaki bütün kalite kapıları bu makinede (Linux, 4 çekirdek) çalıştırıldı. (2) Kod beş koldan
okundu: ses motoru, analiz ve Görsel Yönetmen, kütüphane/depolama/komutlar, arayüz, CI/yayın/lisans. Her kol için
bulgular toplandı, sonra en önemlileri kodda ikinci kez doğrulandı; "doğrulandı" demek kod akışı okunarak kanıtlandı
demektir, Windows'ta çalıştırılarak değil. (3) Matematik yoğun yerler referans formüllerle karşılaştırıldı.

## 1. Kısa sonuç

- **Sağlık:** bütün kapılar yeşil. Rust 318 + 4 kodek testi, arayüz 270 test, clippy `-D warnings` temiz, biçim, tür,
  komut sözleşmesi (42 komut), lisanslar (242 npm paketi, cargo-deny), ses dosyası denetimi temiz.
- **Matematik:** denetlenen her formül doğru (bölüm 3). Yanlış bir DSP formülü, yanlış bir ağırlık ya da işaret hatası
  bulunmadı. Bir tanesinde sayısal pay çok dar (nabız seyreltmesi, A1).
- **Mühendislik:** temel sağlam (gerçek zamanlı ses yolu, göçler, sürüm kapıları), ama **23 doğrulanmış kusur** ve
  ~30 olası risk var. Kullanıcıyı en çok etkileyen ilk altısı:
  1. Taramada okunamayan alt klasör "silinmiş" sayılıyor; şarkılar ve analizleri kütüphaneden siliniyor (K1).
  2. Açılamayan tek bir dosya çalma sırasını kilitliyor; Önceki/Sonraki kapanıyor (U1).
  3. Test sürümü CI'yi beklemeden yayımlanıyor; yazma izinli token sabitlenmemiş eylemlerin elinde (C1, C2).
  4. Görsel veri her karede 2–3 kez ayrı ayrı isteniyor; her istek oynatıcı kilidini alıyor (U3).
  5. Yarıda kesilen analiz "tamamlanmış" diye önbelleğe yazılıyor; görseller o şarkıda kalıcı kısalıyor (A2).
  6. 172–176 BPM'de (DnB) nabız düzensiz atlıyor; güvenli modda 120 BPM'de de (A1).
- **Odyofil ve profesyonel gözüyle** (bölüm 6): ses zincirinin matematiği ve ölçüm kültürü güven veriyor; eksikler
  kararlılık (yukarıdaki kusurlar), dağıtım güvenliği (C1–C3) ve birkaç özellik (ASIO, DSD yok; parametrik EQ arayüzü,
  crossfeed yolda).

## 2. Sağlık denetimi (bu makinede çalıştırıldı)

| Denetim                                     | Sonuç                                                          |
| ------------------------------------------- | -------------------------------------------------------------- |
| `cargo fmt --check`                         | temiz                                                          |
| `cargo clippy --all-targets -- -D warnings` | temiz                                                          |
| `cargo test`                                | 318 geçti, 1 yoksayıldı (gerçek aygıt), 0 düştü; kodek 4 geçti |
| `src/lib/bindings` üretimi                  | depodakiyle aynı                                               |
| `cargo deny check licenses bans sources`    | bans, licenses, sources: temiz                                 |
| `npm run lint`, `typecheck`, `format:check` | temiz                                                          |
| `npm test`                                  | 36 dosya, 270 test geçti (43 sn)                               |
| `npm run build`                             | 371 KB JS (gzip 117 KB), 45 KB CSS                             |
| `npm run check:commands`                    | 42 komut, 42 çağrı, eşleşiyor                                  |
| `npm run check:licenses`                    | 242 paket, yasak lisans yok                                    |
| `npm run check:audio`                       | 274 dosya, ses dosyası yok                                     |

Ölçek: Rust 22 300 satır (286 test), TypeScript 15 200 satır (36 test dosyası). `unsafe` yok. `unwrap`/`expect`/
`panic` lintleri açık; sürümde `panic = "abort"`.

## 3. Matematik ve mühendislik doğrulaması

Formüller referanslarıyla karşılaştırıldı; sapma yok.

| Yer                        | Denetlenen                                                                                                                                                                                                                                                                       | Sonuç                            |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| `audio/biquad.rs`          | RBJ Cookbook: tepe, alçak/yüksek raf, alçak/yüksek geçiren katsayıları; \|H\|² formülü; devrik doğrudan biçim II                                                                                                                                                                 | birebir doğru                    |
| `audio/loudness.rs`        | BS.1770-4: K-ağırlıklı süzgeç (analog örnekten her hızda; 48 kHz'te tablo katsayıları testle), 400 ms blok / 100 ms adım, −70 LUFS mutlak ve −10 LU göreli kapı, −0,691 dB sabiti, 5.1 kanal ağırlıkları (LFE yok, arka 1,41), gerçek tepe 4× sık örnekleme (48 vuruş, Blackman) | doğru; EBU 3341 durum 1–5 testli |
| `audio/limiter.rs`         | İleriye bakan sınırlayıcı: kayan en küçük (gecikme+1) → hızlı iniş / yavaş dönüş → gecikme boyu kayan ortalama. İspat: çıkışa varan örnek için uygulanan kazanç, o örneğin istediği kazancın üstüne çıkamaz                                                                      | doğru (kanıt bölüm 3.1)          |
| `visual_bridge/mod.rs`     | Gecikme telafisi işareti: görsel zaman = duyulan + ekran öncülü − aygıt gecikmesi                                                                                                                                                                                                | doğru                            |
| `analysis/key.rs`          | Krumhansl–Kessler majör/minör profilleri (12+12 değer), Pearson ilintisi, Camelot sayısı ((7·tonik+7) mod 12 + 1; minörde +3 yarım ton)                                                                                                                                          | birebir doğru; 24 ton testli     |
| `audio/eq.rs`              | Välimäki–Liski etkileşim matrisli grafik EQ (iki geçişli en küçük kareler); ±1 dB sapma testli                                                                                                                                                                                   | yöntem doğru                     |
| `audio/resample.rs`        | rubato FFT yeniden örnekleyici, Blackman-Harris, sabit oran; gecikme atılıyor, son uzunluk hesaplanıyor                                                                                                                                                                          | doğru                            |
| `analysis/beats.rs`        | Ellis 2007 dinamik programlama; başlangıç gecikmesi örnekleme hızına göre ölçülmüş doğru (tabloyla)                                                                                                                                                                              | doğru                            |
| `audio/biquad.rs` denormal | 1e-200 altı sıfırlanıyor (f64 denormal sınırı 2e-308; ses için anlamsız düzey)                                                                                                                                                                                                   | zararsız                         |

### 3.1 Sınırlayıcı kanıtı

Örnek t girer, t+d'de çıkar (d = gecikme). t+d'de uygulanan kazanç, `held` dizisinin (t, t+d] penceresindeki ortalamasıdır.
Her τ ∈ (t, t+d] için `held(τ) ≤ min{istenen(s) : s ∈ [τ−d, τ]}` ve t bu aralıktadır; dolayısıyla her terim ≤ istenen(t),
ortalama da öyle. Tepe çıkışa vardığında kazanç yeterince inmiştir. ∎

### 3.2 Sayısal payı dar olan yer

`director::MIN_PULSE_INTERVAL = 0,34 s`. 174 BPM'de vuruş aralığı 60/174 = 0,3448 s: pay 4,8 ms. Vuruşlar parabolle
inceltilmiş DP sonucu olduğu için birkaç ms titrer; bazı aralıklar eşiğin altına düşer ve açgözlü seyreltme "her vuruş /
iki vuruşta bir" arasında rastgele gidip gelir (A1). Aynı durum güvenli modda 120 BPM'de (iki vuruş = tam 1,000 s =
`SAFE_PULSE_INTERVAL`) ve 3/4'te 180 BPM'de.

### 3.3 Deneyle doğrulanan hata: MP3'te ilk 47 örnek atılıyor (S1)

Depodaki test dosyası `uc-uc-ton.mp3` (22,05 kHz, mono, 3 sn) Lyraska'nın çözücüsüyle ve ffmpeg ile çözüldü:

| Çözücü  | Örnek sayısı | Hizalama                                          |
| ------- | ------------ | ------------------------------------------------- |
| ffmpeg  | 66 150       | —                                                 |
| Lyraska | 66 103       | Lyraska[0] = ffmpeg[47] (hata 1e-15); sonlar aynı |

Eksik 47 örnek **başta**, sonda değil. Devir notu 2026-10-08'deki "symphonia sondan ~47 örnek fazla atıyor" notu
yanlış teşhisti; `tests/formats.rs`'teki 64 örneklik tolerans hatayı gizliyor. Neden: symphonia 0.6 MP3 okuyucusu ilk
paketlere eksi `pts` verir ve kodlayıcı gecikmesini **kendisi** kırpar (`trim_start`); `decode.rs:201-246` tamponun
`pts`'ten başladığını varsayıp aynı kırpmayı bir kez daha yapar. 44,1/48 kHz MP3'te de 1105 − 1058 = 47 örnek atılır.
Etkisi: boşluksuz çalınan MP3 albümlerde her sınırda ~1–2 ms ses kaybı (dalga kopar, tık duyulabilir); ilk pakete düşen
sarma hedeften geç başlar. Düzeltme: başlangıcı `pts + trim_start` saymak; MP3 toleransını 0'a indirip tam 66 150 beklemek.

## 4. Bulgular

Kodlar: **S** ses motoru, **A** analiz ve Görsel Yönetmen, **K** kütüphane/depolama/komutlar, **U** arayüz, **C** CI,
yayın, lisans. Güven: **kesin** = kod akışı ya da deney kanıtladı; **olası** = okumadan çıkarıldı, çalıştırılmadı.

### 4.1 Yüksek

| Kod | Bulgu                                                                                                                                                                                                                                                                                                               | Yer                                                                                                      | Etki                                                                                                                                              | Güven                         |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- |
| K1  | Taramada okunamayan alt klasör ya da dosya (izin, ağ kopması, USB hatası) `filter_map(Result::ok)` ile sessizce atlanıyor; bilinen ama listede olmayan yollar "silinmiş" sayılıp tek işlemde kütüphaneden çıkarılıyor; `analyses_follow_tracks` tetikleyicisi analizlerini de siliyor                               | `library/scan.rs:138-146, 213-224`; `storage.rs:69`                                                      | Şarkılar listeden kayboluyor; geri gelince spektrum, ritim ve ton saatlerce baştan analiz ediliyor                                                | kesin                         |
| S1  | MP3'te ilk 47 örnek atılıyor (bölüm 3.3)                                                                                                                                                                                                                                                                            | `audio/decode.rs:201-246`                                                                                | Boşluksuz geçişte tık; sarma kayması                                                                                                              | kesin (deney)                 |
| U1  | Açılamayan şarkı sırayı öldürüyor: `restart` hatada `session = None` → `status.track = null` → `queueActive` ve `canNext` false; Önceki/Sonraki/Çal kapanıyor. Otomatik geçişte sıradaki bozuksa aynı                                                                                                               | `hooks/usePlayback.ts:43-51, 96-97`; `components/Dock.tsx:51,71`; `player.rs:615-625`                    | Listede tek okunamayan dosya çalmayı durduruyor; kütüphaneye dönüp elle seçmek gerekiyor                                                          | kesin                         |
| C1  | Test sürümü CI'yi beklemeden yayımlanıyor (`on: push: [main]`; `needs`/`workflow_run` yok; yayın işinde test yok)                                                                                                                                                                                                   | `.github/workflows/test-surumu.yml:6-9`                                                                  | Kırmızı bir main ya da birlikte hiç sınanmamış iki PR'ın toplamı sabit linke düşebilir                                                            | kesin                         |
| C2  | Yazma izinli token bütün iş akışında ve sabitlenmemiş üçüncü taraf eylemlerin erişiminde: `contents: write` iş akışı düzeyinde, `checkout` kimlik bilgilerini bırakıyor, `tauri-action@v0`'a gereksiz `GITHUB_TOKEN`, eylemler SHA yerine etiketle (`@stable`, `@v0`, `@v2`, `@v5`)                                 | `test-surumu.yml:16-17, 40-43`; `ci.yml:156-159`                                                         | 479 crate'in `build.rs`'i ve npm betikleri dahil herhangi bir tedarik zinciri ele geçirmesi sabit linkteki exe'yi değiştirebilir                  | kesin                         |
| C3  | Üçüncü taraf lisans bildirimleri pakette yok: ~470 Rust crate (MIT/BSD/Apache/MPL) ve React vb. için telif ve izin metni dağıtılmıyor; `public/lisanslar/`'da yalnız Inter var; uygulamada lisans/kaynak kod bağlantısı yok                                                                                         | `src-tauri/Cargo.toml`, `public/lisanslar/`                                                              | MIT/BSD ikili dağıtımda bildirim şart koşar; CLAUDE.md'nin kendi kuralı da uygulanmamış                                                           | kesin                         |
| U3  | Görsel veri (`visual_frame`) her ekran karesinde 2–3 kez ayrı ayrı isteniyor: `SyncPanel`'deki `BeatIndicator` çekmece kapalıyken bile `useVisualFeed` çalıştırıyor; gece sürüşünde `DriveScene` ve içindeki `HighwayScene` iki ayrı döngü. Her istek `player.lock()` alıyor; 144 Hz ekranda saniyede 300–430 istek | `components/SyncPanel.tsx:19-29,108`; `DriveScene.tsx:143`; `HighwayScene.tsx:54`; `commands.rs:209-211` | Dahili ekran kartında ve az çekirdekli dizüstünde gereksiz işlemci/IPC yükü; şarkı açılırken kilit tutulduğundan birden çok tokio işçisi bekliyor | kesin                         |
| A2  | Yarıda kesilen çözme (EOF dışı G/Ç hatası: NAS kopması, USB) "tamamlanmış analiz" olarak dosya damgasıyla kalıcı önbelleğe yazılıyor (`Ok(None) \| Err(_) => break` sonra `finish`)                                                                                                                                 | `analysis/spectrogram.rs:414-417, 452`; `player.rs:502-505`; `background.rs:225-236`                     | O şarkının görselleri kesilme noktasından sonra kalıcı kayboluyor; vuruş, bölüm, drop eksik; dosya değişmeden düzelmiyor                          | kesin                         |
| A1  | Nabız seyreltmesi açgözlü (`thin`): 172–176 BPM'de vuruş aralığı 0,34 s eşiğine 1–5 ms yakın; DP titreşimiyle nabız "her vuruş / iki vuruşta bir" arasında rastgele gidip geliyor. Güvenli modda 120 BPM (2 vuruş = tam 1,000 s) ve 3/4'te 180 BPM'de aynı                                                          | `director/mod.rs:31, 125-139, 260-268`                                                                   | DnB ve çok yaygın 120 BPM'de ritim katmanı tökezliyor                                                                                             | kesin (aritmetik), etki olası |

### 4.2 Orta

| Kod | Bulgu                                                                                                                                                                                                                                                                                                  | Yer                                                                                                         | Etki                                                                                                                              | Güven                                                                                                                                                              |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| K2  | Kütüphane sorguları (`analyzed_count`, arama, `pending_library_tracks`) analiz tablosundaki birkaç yüz KB'lık BLOB'lardan **sonra** gelen sütunları (`*_version`, `camelot`) okuyor; SQLite bunun için taşma sayfası zincirini her satırda izliyor. `library_status` taramada 600 ms'de bir çağrılıyor | `storage.rs:56-68` ve V2–V7 göçleri; `library/db.rs:178-190, 290-299, 344-350`; `analysis/cache.rs:334-343` | 10 bin şarkıda her durum sorgusu GB'larca okuma; liste ve arama saniyelerce gecikir, tarama yavaşlar                              | olası (SQLite dosya biçiminden; ölçülmedi). Düzeltme: kapsayan indeks (`analyses(path, file_size, modified, *_version, bpm, camelot)`), `EXPLAIN QUERY PLAN` testi |
| K3  | "Klasör ekle"/"Yeniden tara" sonrası `status()` çoğunlukla `scanning=false` dönüyor (bayrağı iş parçacığı kilidi alınca açıyor, komut hemen soruyor); arayüz yalnızca `scanning` iken yokluyor                                                                                                         | `library/service.rs:94, 195-211`; `commands.rs:226-227, 268-269`; `hooks/useLibrary.ts:97-102`              | Eklenen klasörün şarkıları listede belirmeyebilir; "Şarkı yok." kalır                                                             | olası (yarış)                                                                                                                                                      |
| K4  | Taranan klasör çıkarılınca tarama sürüyor; ilk yazmada yabancı anahtar hatası ("FOREIGN KEY constraint failed") sorunlar listesine düşüyor ve tam taramaya kadar kalıyor                                                                                                                               | `service.rs:184-186`; `scan.rs:201, 206`; `db.rs:90`                                                        | Yanlışlıkla eklenen `C:\` dakikalarca taranıyor; anlamsız hata görünüyor                                                          | kesin (yarış durumunda)                                                                                                                                            |
| K5  | Kör test hazırlığı (iki dosyayı 60 sn'ye kadar çözme, yeniden örnekleme, FFT ilintisi) `Mutex<Player>` tutulurken, `async` komut içinde `spawn_blocking` olmadan yapılıyor; `open_track` ve benzerleri de öyle                                                                                         | `commands.rs:624`; `player.rs:689-700`                                                                      | Hazırlık boyunca (1–3 sn) görseller ve konum donuyor; az çekirdekte ilgisiz komutlar bekliyor                                     | kesin (kilit kapsamı)                                                                                                                                              |
| K6  | Ayar kaydı `fs::write` + `rename` ama `sync_all` yok; bozuk ya da tek alanı geçersiz dosyada **her şey** sessizce varsayılana dönüyor, günlüğe yazılmıyor, ilk değişiklikte üstüne yazılıyor                                                                                                           | `settings.rs:41-45, 84-92`                                                                                  | Elektrik kesintisinde ayarlar gidebilir; EQ ve kulaklık profili "kendiliğinden" kaybolur                                          | (a) olası, (b) kesin                                                                                                                                               |
| S2  | Bit-perfect gerçekte bit-perfect olmayabilir: özel mod biçimi aygıtın kabul ettiği en yüksek tamsayı; şarkının bit derinliği bilinmiyor. 16 bit'e düşen aygıtta 24 bit FLAC dithersiz yuvarlanıyor, 32 bit float WAV kırpılıyor; ekranda yine "bit-perfect"                                            | `audio/output.rs:57-62`; `output/wasapi.rs:110-129`; `player.rs:298-305, 409-422`                           | Kullanıcıya yanlış bilgi; düşük düzeyde nicemleme bozulması                                                                       | kesin                                                                                                                                                              |
| S3  | Özel modda şarkı sonunda arabellek boşalıyor (`padding == 0` beklenip `stop_stream`); yoklamalı özel modda boşalma "glitch"tir, bazı sürücüler eski veriyi tekrar çalar                                                                                                                                | `output/wasapi.rs:290-305, 359-367`                                                                         | Bit-perfect modda her şarkı sonunda 0–5 ms tık/cız olabilir                                                                       | olası (aygıta bağlı)                                                                                                                                               |
| S4  | Ses çıkış iş parçacığının önceliği yükseltilmiyor (MMCSS "Pro Audio" yok); analiz iş parçacıkları aynı öncelikte                                                                                                                                                                                       | `output/wasapi.rs:137-151`                                                                                  | Yük altında takılma riski ("1 saat sıfır takılma" hedefi)                                                                         | olası; `unsafe` gerektirir, sahibin onayı                                                                                                                          |
| A3  | Boşluksuz geçişte görsel şarkı seçimi ses gecikmesini hesaba katmıyor: şarkı aygıtın `heard` değerine göre seçiliyor, zaman ise `heard − delay + lead`                                                                                                                                                 | `player.rs:1027-1031`; `visual_bridge/mod.rs:38-41`                                                         | Bluetooth'ta (200 ms) yeni şarkının ilk 183 ms'inde görseller 0. karede donuyor; eski şarkının son vuruşu/dropu hiç gösterilmiyor | kesin                                                                                                                                                              |
| A4  | Nabız hız garantisi nabız ∪ vurgu birleşiminde sağlanmıyor; `frame_at` durumsuz olduğu için sarma/sürüklemede her kare rastgele nabız üretebiliyor; koruma TS sahne sınırlayıcılarına kalıyor                                                                                                          | `director/mod.rs:125, 135-139, 205-211`                                                                     | Rust tarafındaki "≤ 3/sn" sözü sarmada geçerli değil                                                                              | olası                                                                                                                                                              |
| A5  | Drop zamanı ve güveni ölçü fazına körü körüne bağlı; faz yanlışken (güven düşükken bile) drop 1–3 vuruş kayık konuyor, güveni ≥ 0,6 çıkabiliyor                                                                                                                                                        | `structure.rs:331-339, 553-570`                                                                             | Sayaç, beklenti ve açılım yanlış vuruşa denk geliyor                                                                              | olası                                                                                                                                                              |
| A6  | Önbellek yenilemesinde (`refresh`) ses yüksekliği eskimişse hızlı ölçüm iş parçacığı açılmıyor; seviye bütün şarkı çözülünce geliyor                                                                                                                                                                   | `player.rs:473, 520-557`                                                                                    | `VERSIONS.loudness` artırılırsa her şarkının ilk çalınışında duyulur seviye kayması                                               | kesin                                                                                                                                                              |
| U2  | Boşluksuz olmayan geçiş (hız/kanal farkı, bit-perfect) arayüzün 250 ms yoklamasına bağlı; gizli sayfada WebView2 zamanlayıcıları saniyede bire, 5 dk sonra dakikada bire düşürebilir                                                                                                                   | `hooks/usePlayer.ts:95-101, 224`                                                                            | Küçültülmüş pencerede şarkılar arasında 1–60 sn sessizlik                                                                         | olası                                                                                                                                                              |
| U4  | Kütüphanede arama sonrası seçim eski sıra numarasında kalıyor; `tracks[selected]!` liste kısaldıysa `undefined` → TypeError (olay işleyicisinde, hata sınırı yakalamaz) ya da yanlış şarkı çalar                                                                                                       | `components/LibraryPanel.tsx:49, 90-92`                                                                     | Enter yanlış şarkıyı çalıyor ya da hiçbir şey olmuyor                                                                             | kesin                                                                                                                                                              |
| U5  | Çekmecede odak yönetimi yok: kapanınca odak `body`'ye düşüyor, açılınca panele taşınmıyor, `tablist`'te ok tuşları yok                                                                                                                                                                                 | `components/PlayerScreen.tsx:297-333`; `hooks/useDrawer.ts:38-41`                                           | Klavye kullanıcısı her kapatışta yerini kaybediyor                                                                                | kesin                                                                                                                                                              |
| U6  | Sahne döngüsündeki (`tick`) hata hata sınırına ulaşmıyor; döngü her karede yeniden fırlatıyor, "Sahne çizilemedi" yedeği çıkmıyor                                                                                                                                                                      | `hooks/useVisualFeed.ts:83, 107`                                                                            | Beklenmeyen bir `VisualFrame` sahneyi dondurur; "hata yalnızca o bölümü kapatır" sözü sahne döngüsü için geçerli değil            | mekanizma kesin                                                                                                                                                    |
| U7  | Kütüphane durum yoklaması uçuştaki isteği izlemiyor; SQLite meşgulken sorgular birikir, sırası karışan yanıtlar `scan.scanning` ve sayıları geri alabilir                                                                                                                                              | `hooks/useLibrary.ts:276-288`                                                                               | MIMARI'deki "durum sorguları birikmez" kuralı burada uygulanmamış                                                                 | kesin                                                                                                                                                              |
| C4  | Yayın sırasında iptal (`cancel-in-progress: true`) etiket taşındıktan sonra, `--clobber` eski dosyayı sildikten sonra gelirse sabit link 404 verir                                                                                                                                                     | `test-surumu.yml:11-14, 79-85`                                                                              | Art arda iki birleşmede sahibinin linki boş kalabilir                                                                             | olası                                                                                                                                                              |
| C5  | Güvenlik uyarıları hiç denetlenmiyor (`cargo deny check advisories` ve `npm audit` yok; deny.toml "ayrı iş" diyor, iş yok)                                                                                                                                                                             | `deny.toml:51-52`; `ci.yml:97`                                                                              | Güvenilmeyen ses dosyası açan programda bilinen bir ayrıştırıcı açığı fark edilmez                                                | kesin                                                                                                                                                              |
| C6  | Aynı sürüm numarasıyla farklı ikililer; commit kimliği ekranda yok; değişiklik listesi yok; eski kuruluma dönüş yolu yok; belge PR'ları da exe'yi yeniliyor                                                                                                                                            | `test-surumu.yml:53-72`; `SettingsPanel.tsx:158`                                                            | Sahibi hangi derlemeyi test ettiğini bilemiyor; bozuk sürümden geri dönemiyor                                                     | kesin                                                                                                                                                              |
| C7  | `package.json`, `Cargo.toml` ve ekrandaki sürümün eşitliği testle sınanmıyor                                                                                                                                                                                                                           | `commands.rs:682`                                                                                           | Biri unutulursa bildirilen sürüm ile kurulu farklı                                                                                | kesin                                                                                                                                                              |
| C8  | Yayımcı adı yapılandırılmamış (Tauri varsayılanı identifier'dan "github"); `copyright: "GPL-3.0"` exe özelliklerinde telif satırı                                                                                                                                                                      | `tauri.conf.json:5, 33`                                                                                     | Windows "Uygulamalar"da yayımcı "github"                                                                                          | olası (Tauri varsayılanı)                                                                                                                                          |
| C9  | Bindings denetimi eski (artık üretilmeyen) `.ts` dosyalarını yakalamıyor                                                                                                                                                                                                                               | `ci.yml:85-92`                                                                                              | Silinen bir Rust tipinin TS karşılığı depoda kalır                                                                                | kesin                                                                                                                                                              |
| C10 | CI'da `timeout-minutes` yok (varsayılan 6 saat); Rust araç zinciri `@stable` (yeni clippy lint'leri ilgisiz PR'ları kırar); `rust-version = "1.85"` sınanmıyor; yayın `--locked` değil                                                                                                                 | `ci.yml`, `Cargo.toml:9`                                                                                    | Takılan Edge işi 6 saat yer; dalgalı CI                                                                                           | kesin                                                                                                                                                              |

### 4.3 Düşük (seçilmiş)

- **S5** Ekolayzerin korumaya bildirdiği yükseltme (`boost_db`) yalnızca o anki değer; bas motoru `max(current, target)` bildiriyor. Sürgü yükseltilirken sınırlayıcı kısa süreli devreye girip "pompalama" yapabilir (`eq.rs:646-649`, `render.rs:155-160`).
- **S6** Bas ve vuruş zarfları `flush` edilmiyor; uzun sessizlikte denormal sayılarda takılıp kalıyor (`bass.rs:317-318, 548`; `punch.rs:148-167`). Küçük CPU yükü.
- **S7** Bozuk MP4 `elst`'te `media_time * rate` u64 taşması (`gapless.rs:47`); sürümde sarar, şarkı sessiz başlar.
- **S8** Kör test: bölüm 0,5 sn'den önce başlıyorsa negatif kayma aranamıyor; "aynı kayda benzemiyor" hatası (`abx.rs:369-392`).
- **S9** Hata yutma noktaları: yeniden örnekleyici kurulamazsa "Ses aygıtı hatası" yazıyor (`resample.rs:62`); sıradaki şarkının dönüştürücüsü kurulamazsa sessizce `break` (`player.rs:1228-1234`); `open_next` hatayı yutuyor (`:1308-1310`); şarkı ortasında çözme hatası tamponda 2 sn ses varken Error'a geçiyor, "Bitti" hiç görünmüyor (olası).
- **S10** `normalize.rs` `SLOTS = 8`: 2 sn'lik ön çözmeye 8'den fazla çok kısa şarkı sığarsa yuva ezilir; `Player.next_path` boşluksuz geçişten sonra eski değerde kalıyor (`[X, Y, Y]` sırasında ikinci Y boşluksuz çalmaz); `Session::start` ön dolumu (3 sn'ye kadar) ve `close_session` kısması (300 ms) oynatıcı kilidi altında bekliyor.
- **S11** 5 kanallı (LFE'siz) dosyada arka kanallara 1,0 ağırlık (BS.1770'te 1,41); 7.1'de yan kanallar 1,0 (standart yalnızca 5.1'i tanımlar; düşük).
- **A7** Tek global ölçü fazı: takipçi şarkı ortasında bir vuruş atlarsa sonraki bütün ölçü başları kayar; %70 uyuşmada güven yine 0,57 (`structure.rs:219-231, 391-417`).
- **A8** Arka plan analizinin `skip` kümesi oturum boyunca kalıcı; çıkarılıp geri takılan disk programı yeniden başlatmadan analiz edilmez (`background.rs:124-157`).
- **A9** Oynatıcı kilidi altında analiz iş parçacığı `join` ediliyor (`player.rs:560-566`, `visual_now` yolundan); ritim/koreografi bitişi ya da `store` (5 sn busy_timeout) sırasında arayüz takılabilir.
- **A10** Çalan şarkının `refresh_paced`'ı başarısız olursa (`false`) sıradakinin analizi şarkı değişene kadar bekliyor; `ForegroundGuard` arka planı da durduruyor (`player.rs:496-500, 536-544`).
- **A11** Sessizlik ve breakdown boyunca hayalet vuruş: DP zinciri `local = 0`'da da sürüyor, yalnız uçlar kırpılıyor (`beats.rs:326-351, 368-380`); nabız en az %40 güçle sürüyor.
- **A12** FAT32 sürücüde yaz/kış saati geçişi bütün `mtime`'ları 3600 sn kaydırır; USB kütüphane yılda iki kez baştan analiz edilir (`cache.rs:132-142`).
- **A13** Spektrum karesi gecikmesi (`frame_at` `floor`) örnekleme hızına göre telafi edilmiyor: 44,1 kHz'te ~15 ms, 22,05 kHz'te ~38 ms geç; vuruşlar telafili, bantlar değil (`spectrogram.rs:431-433`).
- **A14** `section_boundaries` içindeki `dedup` işlevsiz; eşit yenilik değerinde 1 ölçülük bölüm çıkabilir (`structure.rs:457-464`). `structure.rs:161` vuruş→kare çevrimine gecikmeyi eklemiyor, `meter.rs:153` ekliyor (tutarsızlık). Sahipsiz `analyses` satırları birikiyor; `VACUUM` yok. İşaret dosya adında Windows ayrılmış adları ("Con", "Aux") (`annotation.rs:190-205`). 192 kHz'te kroma penceresi 32768'e sınırlı: C2 bölgesinde yarım ton çözülmüyor.
- **K7** Bozuk veritabanında (SQLITE_CORRUPT/NOTADB) kullanıcının kurtulma yolu yok; göç öncesi yedek yok (tasarım kararı; sahibe sorulacak).
- **K8** OneDrive yer tutucuları (RECALL_ON_DATA_ACCESS) taramada ve analizde indiriliyor; "internete bağlanmaz" sözüne rağmen GB'larca trafik (olası).
- **K9** Sembolik bağ ve junction sessizce atlanıyor (`follow_links(false)`); uyarı yok.
- **K10** Türkçe arama: ayrışık (NFD) "S + U+0327" biçimi sadeleşmiyor; "sebnem" bulunmuyor (`library/mod.rs:69-96`). Nokta matris yazı da NFD'de "?" basıyor (`dotFont.ts:133-136`).
- **K11** Tarama sürerken liste her şarkı sayısı değişiminde (600 ms) 50 bin satıra kadar baştan isteniyor (`useLibrary.ts:84-94`).
- **K12** Günlükte Windows kullanıcı adı içeren tam yollar; günlük herkese açık hata kaydına ekleniyor (`commands.rs:32-36`); DOGRULAMA "kişisel bilgi içermez" diyor, hata formu "yol geçebilir" diyor (çelişki).
- **K13** Tek kopya koruması yok: iki pencere aynı veritabanı, ayar ve günlüğe yazar, WASAPI özel modda çakışır. `AppData\Roaming`'de GB'lık önbellek (kurumsal profil eşitlemesi). Sürüm derlemesinde `strip = true`: yığın izi çözümsüz.
- **K14** UTF-8 olmayan yollar `to_string_lossy` ile saklanıyor; `scan.rs:160` `on_disk.len() - to_read.len()` alttan taşabilir (hata ayıklamada panic).
- **K15** `indexing_slicing` lint'i açık değil; `panic = "abort"` ile her sınır dışı indeks çökme (en azından `library`, `storage`, `settings`, `commands` için).
- **U8** Şarkı haritası ve ton sorgusu tek geçici hatada vazgeçiyor, 120 denemede (2 dk) duruyor (`useSongMap.ts:25-29`, `useSongKey.ts:112-116`). Ayarlardaki kısayol metni "1 – 4" (5 sahne var). Ekolayzer yüklenemezse ilk dokunuş kayıtlı ayarı siliyor (`useEqualizer.ts:63-112`). Sürükle-bırak dinleyicisi sık yeniden kuruluyor; bırakınca sinema görünümü açılıyor.
- **U9** Güvenli modda VU kırmızı lambası 2/sn'ye kadar ve otoyol şerit hızı sınırsız (`vu.ts:175-178`, `highway.ts:149-158`); kural fiilen `check-scenes` alan sınırıyla korunuyor.
- **U10** Duraklatılmışken ve boştayken sahneler tam hızda çiziliyor; 2D yedek çizim 1080p'de kare başına ~2000 `fillRect`; konum yoklaması `PlayerScreen` ağacını saniyede 4 kez yeniden çiziyor. `aria-valuenow` her karede değişiyor; süre bilinmezken "00:00". WebGL bağlamları `loseContext` ile bırakılmıyor.
- **C11** npm lisans denetimi yalnız yasak listesiyle (GPL-2.0-only, CDDL geçer); "internet yok" ilkesi makineyle denetlenmiyor (`reqwest`/`hyper` tauri bağımlılığı olarak var); `embedBootstrapper` WebView2'si olmayan makinede internetten indirir; kurulum dosyası seçimi sürüme bakmıyor (`-First 1`); denetim betikleri lint edilmiyor; `push: ["**"]` + `pull_request` aynı PR'da işleri iki kez koşturuyor; README "Phase 1 in progress", "3 sahne", "safe mode 3/sn" eski; hata formunda düğme yerleri yanlış; `commands.rs:59` sabit "Faz 1"; kullanıcı kılavuzu yok.

## 5. Sağlam bulunanlar

Denetimin amacı kusur bulmaktı; adil olmak için iyi yapılanlar da yazılmalı, çünkü bunlar "müthiş"in zemini.

- **Gerçek zamanlı ses yolu:** ayırma yok, `unwrap`/`expect`/`panic` yok; `ses_yolu_bellek_ayirmaz` testi bit-perfect
  yolunu, ölçüm beklemesini, ayar değişimini ve veri yetişmemesini kapsıyor. Kilitsiz ayar kanalları sürüm sayaçlı,
  Acquire/Release sıralaması doğru. Çözme girişinde ve aygıt çıkışında NaN/sonsuz temizleniyor.
- **DSP:** sınırlayıcı kanıtlanabilir biçimde doğru; K-ağırlıklama her hızda analog örnekten; EBU 3341 uyum testleri;
  yeniden örnekleyicinin gecikmesi ve süresi birebir telafili; boşluksuz geçişte iki şarkı tek dönüştürücüden;
  eşitleme kazancı tam şarkı sınırında; EQ etkileşim matrisli ve ±1 dB testli; bit-perfect tamsayı yolu testli.
- **Veri:** tek IMMEDIATE işlemli göç zinciri, savepoint, "daha yeni şema" koruması, yalnız önbelleği onaran yol,
  geçmişte üretilmiş her dosya türünün yeni şemaya ulaştığını kendiliğinden denetleyen test; WAL ve `busy_timeout`;
  `ON CONFLICT DO UPDATE` (REPLACE değil: tetikleyici analizi silmiyor); parça sürümleri ve seçici yeniden hesap.
- **Arayüz:** katı TypeScript, `noUncheckedIndexedAccess`, `any` ve `eslint-disable` yok; komut sıralaması, sarma
  birleştirme, "yanıt gelmeden yeni sorgu yok" `usePlayer`/`useAbx`/`useEqualizer`'da testli; kütüphane listesi sanal;
  parlaklık değiştiren her değer hız sınırlı; `check-scenes` gerçek tarayıcıda her sahne/tema/manzarada parlama
  sınırını ölçüyor; `prefers-reduced-motion` JS ve CSS'te; hata günlüğünde tekrar süzgeci.
- **Süreç:** CLAUDE.md'deki her kalite kapısı CI'da gerçekten var; MIMARI'nin "kural → onu denetleyen test"
  tablosundan örneklenen 25 test adı gerçekten var; sentetik test verisi yeniden üretme komutlarıyla duruyor; marka
  varlığı yok; lisans listesi GPL-3.0-only ile uyumlu; CSP sıkı, eklenti izinleri en az düzeyde, updater yok.

## 6. Odyofil ve profesyonel gözüyle (derinleştirilmiş)

Önceki kısa cevabın ("evet, ama") denetimle güncellenmiş hâli.

- **Güven veren:** ses zincirinin matematiği doğru ve ölçülmüş; bit-perfect yol gerçekten işlemsiz; kör test aracı;
  Veritas belgelerinin "olmuyor" diyebilen ölçüm kültürü. Bunlar odyofil çevresinde nadirdir.
- **Güveni zedeleyecek olanlar (bu denetimden):** bit-perfect etiketi 16 bit aygıtta 24 bit dosyada yanıltıyor (S2);
  özel modda şarkı sonu tıkı (S3); MP3'te her sınırda 47 örnek kayıp (S1); özel modda öncelik yükseltmesi yok (S4).
  Odyofil tam olarak bu dört şeyi dener ve yazar. Dördü de düzeltilebilir ve küçük.
- **Eksik özellikler:** ASIO ve DSD yok; örnekleme hızı yükseltme ve süzgeç seçimi yok; elle parametrik EQ arayüzü ve
  crossfeed yolda. Profesyonel için VST, çoklu çıkış, cue yok ve hedef değil.
- **Dağıtım güveni:** imzasız exe, CI'yi beklemeyen yayın, lisans bildirimi yok (C1–C3). Teknik kullanıcı bunları
  depoda görür.

## 7. "Müthiş" için öncelik sırası

Doğrulama haftası kuralı: bunlar yeni özellik değil, bulunan kusurların düzeltmesi ve dağıtım güvenliğidir. Her madde
önce hatayı yakalayan testle gelir. Sıra, kullanıcıya etkisi ve işin küçüklüğüne göre:

1. **Veri güvenliği, bir günlük iş:** K1 (okunamayan klasörde silme yok), K6 (fsync + alan alan ayar çözme), A2 (yarım
   analiz saklanmaz), K4 (çıkarılan klasörün taraması durur).
2. **Çalma deneyimi, bir günlük iş:** U1 (bozuk dosya sırayı öldürmez, atlanır), S1 (MP3 ilk 47 örnek), S2 (bit-perfect
   yalnızca bit derinliği yetiyorsa; yoksa paylaşımlı ve nedeni yazılır), S3 (özel modda sona sessizlik), K5/A9 (kilit
   altında uzun iş yok).
3. **Görsel doğruluk, yarım gün:** A1 (bölen şarkı başına bir kez seçilir, %5–10 tolerans), A3 (boşluksuz geçişte şarkı
   seçimi görsel zamana göre), U3 (tek paylaşılan görsel veri kaynağı; çekmece kapalıyken `BeatIndicator` durur),
   U6 (sahne döngüsü hatası sınıra ulaşır).
4. **Dağıtım güvenliği, yarım gün:** C1 (yayın CI'ye bağlı), C2 (yazma izni yalnız yayın adımında; eylemler SHA'ya
   sabit; `persist-credentials: false`), C4 (iptal yok, önce yükle sonra etiket), C5 (haftalık advisories + npm audit),
   C7 (sürüm eşitliği testi), C10 (zaman aşımı, araç zinciri sabit, `--locked`), C8 (publisher, copyright).
5. **Lisans uyumu, yarım gün:** C3 (`cargo about` + npm bildirimi → `public/lisanslar/UCUNCU-TARAF.txt`, CI'da
   güncellik denetimi, Ayarlar'da "Lisanslar · Kaynak kod").
6. **Ölçülmesi gerekenler (sahibin cihazında):** K2 (büyük kütüphanede durum sorgusu süresi; kapsayan indeks), S4
   (MMCSS: önce `unsafe` için onay), U2 (küçültülmüş pencerede geçiş), A5/A7 (gerçek şarkılarda drop ve ölçü fazı;
   işaretleme aracıyla).
7. **Belge ve küçükler:** README ve hata formu güncellemesi, "Faz 1" sabiti, "1 – 4" kısayol metni, DOGRULAMA'ya yeni
   maddeler (üstüne kurulum, kaldırma, internet kapalı açılış, ayarların kalıcılığı), günlükte kullanıcı adının
   maskelenmesi (K12), NFD sadeleştirme (K10).

Bu liste bitince program "sağlam temel üstünde erken sürüm" olmaktan çıkıp "güvenilir çalar" olur; odyofil ve
profesyonelin denediği dört şey (bölüm 6) o zaman geçer.

## 8. Sınırlar ve yöntem notu

- Denetim Linux kapsayıcısında yapıldı; WASAPI yolu ve kurulum dosyası çalıştırılmadı. "Olası" damgalı maddeler
  Windows'ta ya da büyük kütüphanede ölçülmelidir.
- Kod beş paralel incelemeyle okundu, bulgular ikinci kez kodda doğrulandı; bu belgeye yalnızca doğrulanan ya da
  somut kod yeri gösterilen maddeler alındı. Satır numaraları `294bd80` içindir.
- Matematik denetimi formül düzeyindedir (bölüm 3); sayısal testlerin kendileri zaten depoda ve geçiyor. S1 ayrıca
  deneyle (ffmpeg karşılaştırması) doğrulandı.
