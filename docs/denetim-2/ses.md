# Ses motoru — Bağımsız denetim raporu

Denetçi: bağımsız denetçi ses motoru. Kaynak: main @ 25f83f8. Tarih: 16 Ekim 2026.

## 1. Kapsam ve yöntem

**İncelenen dosyalar (satır satır okundu, toplam ≈ 12 000 satır):**
`src-tauri/src/audio/` altında `mod.rs`, `decode.rs`, `gapless.rs`, `eq.rs`, `bass.rs`, `punch.rs`,
`biquad.rs`, `loudness.rs`, `normalize.rs`, `peq.rs`, `resample.rs`, `limiter.rs`, `render.rs`, `output.rs`,
`output/wasapi.rs`, `output/simulated.rs`, `abx.rs`, `player.rs`, `test_util.rs`; `src-tauri/tests/formats.rs`;
`src-tauri/examples/ses_denemesi.rs`; `src-tauri/Cargo.toml` ve `Cargo.lock` (ses bağımlılıkları: symphonia 0.6.1,
rubato 5.0.1, rtrb 0.4.0, wasapi 0.25.0, allocation-counter 0.8.1). Ses yolunun çağırdığı komşu kod da bağlam için
okundu: `commands.rs` (kilit kullanımı), `analysis/spectrogram.rs` (`measure_loudness`), `visual_bridge/mod.rs`,
`docs/MIMARI.md`, `docs/ROADMAP.md`, `README.md`.

**Bağımlılık kaynakları:** `~/.cargo/registry` içindeki symphonia-core 0.6.1 (`Packet.trim_start` anlamı),
symphonia-bundle-mp3 (dolgunun çözücüde kırpıldığı), symphonia-codec-aac (SBR desteği), wasapi 0.25
(`is_supported_exclusive_with_quirks`, `calculate_aligned_period_near`, `get_available_space_in_frames`,
`wait_for_event`), allocation-counter (kendi `#[global_allocator]`'ı var → bellek testi geçerli).

**Standartlar:** ITU-R BS.1770-4 / EBU R128 ve EBU Tech 3341, RBJ Audio EQ Cookbook, WASAPI özel/paylaşımlı mod
kuralları, Rust gerçek zamanlı ses kuralları (ayırma/kilit/panik yok), proje kuralları (`CLAUDE.md`).

**Çalıştırılanlar:** derleme/test yok (izin verilmedi). python3 ile sayısal doğrulama
(`scratchpad/denetim2/ses_check.py`, `limiter_check.py`):

- K-ağırlıklama katsayıları 48 kHz'te BS.1770-4 Tablo 1 ile 1e-10 içinde aynı (kod bilinear türetimle her hızda üretir).
- Ekolayzerin tepe filtresi: orta frekansta kazanç tam (12,000 dB), kenarlarda hedef biçimde.
- Gerçek tepe süzgeci (4×, faz başına 12 Blackman-sinc): 1–15 kHz'te |hata| ≤ 0,02 dB, 20 kHz'te −0,22 dB (EBU
  3341 toleransı +0,2/−0,4 dB içinde).
- Sınırlayıcı: 2 s'lik +12 dB'ye kadar taşan rastgele/darbe/kare sinyalde çıkış tepesi tam 1,000000000; en büyük
  kazanç adımı 0,0085 (tıksız). Kayan en küçük (delay+1) → kayan ortalama (delay) yapısı matematiksel olarak
  `çıkış ≤ CEILING` garantisini verir (tepe t'de girer; t..t+delay arasında `held ≤ wanted(t)`; t+delay'de uygulanan
  kazanç bu değerlerin ortalaması).
- Binom kuyruğu: P(K ≥ 12 | n = 16) = 2517/65536 doğru.

## 2. Genel değerlendirme

**Puan: 8/10.** Ses motoru, bu büyüklükte bir açık kaynak çalar için alışılmadık ölçüde disiplinli: gerçek zamanlı
yol gerçekten ayırma/kilit/panik içermiyor ve bu ölçülerek test ediliyor; DSP matematiği (RBJ, BS.1770-4, Orfanidis
tipi tepe filtresi, ileriye bakan sınırlayıcı) hesapla doğrulanabildi ve doğru; `unsafe` yok; ayar kanalları
atomik ve bellek sıralaması doğru (Relaxed değerler + Release sürüm / Acquire okuma); boşluksuz geçiş, yeniden
örnekleme gecikmesi ve şarkı sınırındaki kazanç geçişi özenle ele alınmış. **En güçlü yan:** gerçek zamanlı yolun
tasarımı ve bunu kanıtlayan test takımı (`ses_yolu_bellek_ayirmaz`, EBU 3341 durumları, bit bit aynılık testleri).
**En zayıf yan:** ses yolu dışındaki "altyapı" davranışı — aygıt değişimini izlememe, oynatıcı kilidi altında yapılan
bloklayan işler, ses iş parçacığına öncelik verilmemesi. **Bir numaralı risk:** kullanıcı kulaklık taktığında ya da
Bluetooth kulaklık bağlandığında sesin eski aygıtta (hoparlörde) devam etmesi; oynatıcı yeni varsayılan aygıta ancak
yeni bir oturum (sarma/şarkı değiştirme) açılınca geçer, boşluksuz zincirde ise hiç geçmez.

## 3. Bulgular (önem sırasıyla)

### [SES-01] Varsayılan ses aygıtı değişince akış eski aygıtta kalır; aygıt kopunca kurtarma kullanıcıya kalır

- Önem: Yüksek
- Güven: yüksek (Windows'ta çalıştırılamadı; kodda aygıt değişimini izleyen hiçbir yol yok)
- Yer: `src-tauri/src/audio/output/wasapi.rs:257` (`default_device()` yalnızca oturum açılırken), `:291-402`
  (döngüde aygıt değişimi denetimi yok), `:393-397` (yalnızca 2 s olay zaman aşımı); `player.rs:846-851`
  (hatadan sonra "çal" ile elle kurtarma)
- Kanıt: `let device = default_device()?;` bir kez çağrılır; döngüde yalnızca `shared.stop`, `get_available_space`,
  `wait_for_event` denetlenir. Depoda `IMMNotificationClient`/`OnDefaultDeviceChanged` ya da eşdeğeri yok
  (`grep -rn Notification src-tauri/src/audio` boş).
- Etki: Çalarken kulaklık takılınca (Windows yeni varsayılan aygıta geçer) ses hoparlörden çalmaya devam eder; şarkı
  değişene kadar (boşluksuz zincirde kuyruğun sonuna kadar!) böyle kalır. Varsayılan aygıt kaldırılınca akış hata
  verir, durum "Error" olur, kullanıcı "çal"a basana kadar ses kesik kalır (bu yol test edilmiş:
  `aygit_hatasindan_sonra_cal_kaldigi_yerden_surer`). Rakip çalarların hepsi bunu otomatik yapar; proje sahibi
  gerçek cihaz denemesinde ilk fark edeceklerden biri budur.
- Yeniden üretme: Şarkı çalarken 3,5 mm kulaklık tak (ya da Bluetooth kulaklık bağla) → ses hoparlörden sürer.
  Kulaklığı çıkar (aygıt kaybolur) → ses durur, arayüzde hata; "çal" gerekir.
- Önerilen düzeltme: En küçük doğru düzeltme, `wasapi::run` döngüsünde her periyotta (ya da 250 ms'de bir)
  `DeviceEnumerator::get_default_device` kimliğini (`get_id`) oturumun aygıt kimliğiyle karşılaştırıp değiştiyse
  `shared.fail` yerine ayrı bir `device_changed` atomik işareti kaldırmak; oynatıcı bu işareti `status()`/
  `visual_now()` içinde görünce `restart(path, resume, Some(position))` yapar (aynı yol `set_playback_options`'ta
  zaten var). Daha doğrusu `IMMNotificationClient` kaydı (wasapi crate'i `register_device_notifications`? yoksa
  yoklama yeterli). Aygıt kaybında da aynı işaretle otomatik yeniden açma (yeni varsayılan aygıtta) yapılmalı.

### [SES-02] Oynatıcı kilidi altında bloklayan işler: sarma/şarkı değiştirme sırasında arayüz ve görseller donar

- Önem: Orta
- Güven: kesin
- Yer: `src-tauri/src/commands.rs:152-157` (`open_track`), `:197-206` (`seek_playback`), `:209-211`
  (`visual_frame` her ekran karesinde aynı kilit); `player.rs:686-697` (`close_session` → `wait_for_fade_out`,
  ≤ 300 ms), `:290` (`Decoder::open`, disk), `:371` (`wait_for_prefill`, ≤ 3 s), `:373` (WASAPI açma),
  `:1355-1367` (beklemeler `std::thread::sleep` ile)
- Kanıt: `pub async fn seek_playback(...) { let mut player = player.lock()?; player.seek(seconds)...}` → `restart`
  → `close_session` (`wait_for_fade_out(&session.shared, FADE_OUT_TIMEOUT)`; tipik 110–130 ms, aygıt arabelleği
  100 ms + geçiş) → `Session::start` (dosya açma, 0,25 s ön dolum beklemesi, COM/WASAPI başlatma). Bu süre boyunca
  `visual_frame` ve `playback_status` kilidi bekler. Ayrıca `async fn` içinde `std::thread::sleep` ile bekleme
  Tauri'nin tokio çalışma zamanının bir iş parçacığını bloklar. Aynı sorun kör test için fark edilmiş ve #81'de
  hazırlık `spawn_blocking` ile kilit dışına alınmış (`commands.rs:626`); çalma yolunda alınmamış.
- Etki: Her sarma ve şarkı değiştirmede görseller ~150–400 ms donar (60 fps'de 10–25 kare); yavaş disk, uyuyan
  HDD ya da ağ sürücüsünde ön dolum 3 s'ye kadar sürer ve bu süre boyunca arayüzün bütün oynatıcı komutları
  (durum, görsel, ekolayzer) bekler. ROADMAP hedefi "1080p'de 60 fps" bu anlarda tutmaz.
- Yeniden üretme / önerilen test: Sanal çıkışla `Player::seek` çağrısının süresini ölç (Linux'ta
  `ses_denemesi` benzeri); ağ sürücüsündeki dosyada sar → arayüz donması.
- Önerilen düzeltme: (1) `restart`'ı iki aşamaya böl: kilit dışında `Session::prepare` (çözücü aç, ön dolum
  başlat, aygıt aç), kilit içinde yalnızca takas (`abx_install` deseni). (2) `wait_for_fade_out` ve
  `wait_for_prefill`'i `tauri::async_runtime::spawn_blocking` içinde çalıştır ya da `Condvar`/kanal ile uyandır.
  (3) `visual_frame` için `try_lock` kullanıp kilitliyse son kareyi döndür (donma yerine tekrar).

### [SES-03] Ses çıkış iş parçacığına öncelik verilmiyor; analiz iş parçacıklarıyla eşit yarışıyor

- Önem: Orta
- Güven: kesin (kodda yok; ROADMAP:228'de bilinçli ertelendiği yazılı)
- Yer: `output/wasapi.rs:137-139` (`std::thread::Builder::new().name(...).spawn`), `player.rs:364, 496, 504`
  (çözme, hızlı ölçüm ve tam analiz iş parçacıkları aynı öncelikte), `analysis/background.rs` (kütüphane analizi)
- Kanıt: `grep -rni "priority|AvSetMm|mmcss" src-tauri/src/audio` boş. Tek güvence 100 ms aygıt arabelleği
  (`BUFFER_DURATION_HNS = 1_000_000`).
- Etki: Dört–beş ağır iş parçacığı (FFT'li spektrum analizi, ton tespiti, arka plan kütüphane analizi, çözme) ve
  WebView çizimi aynı anda çalışırken 2–4 çekirdekli bir dizüstünde ses iş parçacığı 100 ms'den uzun bekleyebilir
  → takılma. "1 saat çalmada sıfır takılma" hedefi ağır yükte güvenceye alınmamış. Özel modda yoklama 5 ms'lik
  `sleep` ile yapıldığı için öncelik daha da önemlidir.
- Önerilen test: `ses_denemesi` çalışırken aynı anda 8 iş parçacıklı yapay yük (ör. büyük bir klasörün arka plan
  analizi) → `underruns` sayacı.
- Önerilen düzeltme: Ses iş parçacığının başında `AvSetMmThreadCharacteristicsW("Pro Audio")` (tek `unsafe`
  çağrı, `output/wasapi.rs` içinde `// GÜVENLİK:` yorumuyla; proje kuralı buna izin veriyor) ya da MIT lisanslı
  `thread-priority` crate'i ile `ThreadPriority::Max` (proje kodunda `unsafe` gerekmez). Ek olarak analiz iş
  parçacıklarına düşük öncelik (`THREAD_PRIORITY_BELOW_NORMAL`) verilmesi ucuz bir kazançtır.

### [SES-04] İlk ses için 0,6 s bekleme: "hızlı ölçüm genellikle yetişir" varsayımı tam uzunluktaki şarkılarda tutmaz

- Önem: Orta
- Güven: yüksek (süre makineye bağlı; mekanizma kesin)
- Yer: `render.rs:46-49` (`LOUDNESS_WAIT_SECONDS = 0.6`), `:139-154`; `analysis/spectrogram.rs:370-392`
  (`measure_loudness` dosyanın **tamamını** çözer); `normalize.rs:25` (`ASSUMED_LUFS = -10`), `:32`
  (`UPDATE_SECONDS = 1.5`)
- Kanıt: Bütünleşik ses yüksekliği (BS.1770 kapılı ortalama) tanım gereği bütün şarkıyı gerektirir; `measure_loudness`
  `decoder.next_chunk()` döngüsünü `Ok(None)`'a kadar sürdürür. 4 dakikalık bir MP3'ün çözülmesi tipik dizüstünde
  1–3 s'dir (symphonia ~100–200× gerçek zaman); 0,6 s'lik bekleme çoğu zaman dolar. Sonra şarkı −4 dB varsayımıyla
  başlar, ölçüm gelince 1,5 s'lik zaman sabitiyle kayar; bu sırada `headroom_db = 0` olduğu için ekolayzer koruması
  da tam kısar (`normalize.rs:228-236`).
- Etki: Önbellekte olmayan her şarkıda (ilk dinleme, önbellek silinmiş, kütüphane dışı dosya) "çal"a basınca
  0,6 s sessizlik; ardından duyulabilir bir seviye kayması. Eşitleme varsayılan **açık** olduğu için bu
  varsayılan deneyimdir. Hafifletici: kütüphanenin arka plan analizi zamanla önbelleği doldurur. Ayrıca analiz hiç
  başlatılamazsa (`Analysis::start` → `Decoder::open` başarısız, `player.rs:490`) `is_known(0)` hiç olmaz ve
  **her** açılış/sarmada 0,6 s beklenir.
- Önerilen test: 4 dakikalık sentetik WAV/MP3 ile `Player::load` → ilk `frames_consumed > 0` anına kadar geçen
  süre; `Analysis::start`'ın `None` döndüğü durumda `render` bekleme sayısı.
- Önerilen düzeltme: Bekleme süresini 0,6 s'den ~0,15 s'ye indirip hızlı ölçümden **erken tahmin** yayımlamak
  (ilk 3–5 s'nin kısa dönem yüksekliği; kaba ama −4 dB sabit varsayımından iyidir); ya da kütüphane taramasında
  (`library/scan.rs`) yalnızca ses yüksekliğini önceden ölçmek. Analiz başlatılamadığında `levels.set_loudness(0,
None)` benzeri bir "ölçülemez" işareti yazılmalı.

### [SES-05] Desteklenen uzantı listesi çözücü yeteneklerinden geniş: Opus çözücü yok, HE-AAC desteklenmiyor

- Önem: Orta
- Güven: yüksek (crate kaynakları incelendi; gerçek HE-AAC/Opus dosyasıyla davranış denenmedi)
- Yer: `decode.rs:25-28` (`SUPPORTED_EXTENSIONS` içinde `"webm"`, `"mka"`, `"aac"`, `"m4a"`); `Cargo.toml:22`
  (symphonia 0.6 `features = ["all"]`)
- Kanıt: `~/.cargo/registry` içinde `symphonia-codec-opus` yok (0.6.1 Opus içermez); WebM/MKA ses izleri pratikte
  çoğunlukla Opus'tur. `symphonia-codec-aac-0.6.1/src/aac/mod.rs:172` SBR yükünü yok sayar
  (`// no SBR payload or such`), `:96` "aac too complex". HE-AAC v1/v2 (düşük bit hızlı M4A, podcast, radyo
  kayıtları) bu çözücüde ya reddedilir ya da yalnızca çekirdek AAC-LC (yarı bant genişliği, PS'siz mono) çözülür.
- Etki: "Dosya aç" penceresi ve kütüphane taraması bu dosyaları kabul eder; kullanıcı "desteklenmiyor" hatası ya da
  sessizce donuk/mono/yanlış hızda ses alır (hangisi olduğu gerçek dosyayla doğrulanmalı — bkz. §5).
- Önerilen test: `tests/data`'ya ffmpeg ile üretilmiş sentetik HE-AAC (`-c:a libfdk_aac -profile:a aac_he`) ve
  WebM/Opus ekle; ilki için beklenen davranışı (açık hata) sabitle.
- Önerilen düzeltme: `webm`/`mka`'yı listeden çıkar ya da açılışta kodek adını denetleyip Türkçe açık hata ver
  ("Bu dosya Opus kodlanmış; henüz desteklenmiyor"); AAC için `codec_params` içindeki profil/`extra_data`
  (AudioSpecificConfig'te SBR işareti) okunup HE-AAC'de uyarı verilmesi.

### [SES-06] Çok kanallı dosyalar: kanal sayısı aygıta uyarlanmıyor; ses yüksekliği ağırlıkları yalnızca 1, 2 ve ≥ 6 kanalı tanıyor

- Önem: Orta
- Güven: olası (Windows'ta denenmedi; ağırlık kısmı kesin)
- Yer: `resample.rs:82-88` (`output_channels`: yalnızca mono → stereo), `player.rs:297`, `output/wasapi.rs:187-201`
  (paylaşımlı akış şarkının kanal sayısıyla, `autoconvert: true`); `loudness.rs:226-235` (ağırlıklar)
- Kanıt: 6 kanallı FLAC → `spec.channels = 6` → `WaveFormat::new(32, 32, Float, rate, 6)`; stereo aygıtta
  `AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM`'in kanal sayısı dönüşümü yaptığı belgelenmiş değildir (örnekleme hızı ve
  biçim için belgelidir); başarısızsa `AUDCLNT_E_UNSUPPORTED_FORMAT` → "Ses akışı başlatılamadı". Özel modda
  `exclusive_format(rate, 6)` stereo DAC'ta reddedilir → paylaşımlıya düşer (bu yol doğru). Ağırlıklar: 5 kanal
  (L R C Ls Rs) için Ls/Rs 1,0 (BS.1770: 1,41), 7.1'de Lb/Rb 1,0; kanal sırası SMPTE varsayılıyor, symphonia'nın
  kanal maskesine bakılmıyor.
- Etki: 5.1 FLAC/DTS-rip koleksiyonu olan kullanıcıda açılma hatası ya da (dönüşüm yapılırsa) kontrolsüz
  indirgeme; 5.0 dosyada eşitleme 1–1,5 LU hatalı.
- Önerilen test: Gerçek Windows'ta 6 kanallı sentetik WAV; `loudness.rs` testine 5 ve 8 kanallı ağırlık durumu.
- Önerilen düzeltme: `Converter`'a basit indirgeme matrisi (ITU-R BS.775: L' = L + 0,707 C + 0,707 Ls) ve
  `output_channels(n) = min(n, aygıt kanal sayısı)`; ağırlıkları kanal maskesinden türet.

### [SES-07] Duraklatınca konum tahmini aygıt arabelleği kadar (~100 ms) geride donar

- Önem: Düşük
- Güven: kesin
- Yer: `output/wasapi.rs:349-363` (`output_heard = consumed − padding − latency`, `fetch_max`), `:314-338`
  (duraklatmada aygıta sessizlik yazılır, `consumed` artmaz)
- Kanıt: Duraklatma anında arabellekte ~100 ms gerçek ses vardır (geçiş halka tamponun başında yapılır, hoparlöre
  100 ms sonra ulaşır). `consumed` C'de durur; `padding` sessizlikle dolu kalır → tahmin C − 100 ms'de kilitlenir;
  gerçekte duyulan son kare ≈ C. Yorum satırı (`:351-353`) bunun farkındadır ama yalnızca "geri gidiş yok" der.
- Etki: Duraklatılmışken gösterilen konum ~0,1 s geride; duraklatılmışken yapılan yeniden açmalar
  (bit-perfect değişimi `player.rs:955-958`, kör test dönüşü `:768-771`, hata sonrası çal) 0,1 s geriden başlar.
  `ses_denemesi` yalnızca konumun **sabit** kaldığını denetler, doğruluğunu değil.
- Önerilen düzeltme: Duraklatma sonrası sessizlik yazılan kareleri ayrı say (`silence_in_buffer`) ve
  `padding.saturating_sub(silence_in_buffer)` kullan; ya da `FadeWatch` bilgisiyle geçiş hoparlöre ulaşınca
  `output_heard = consumed − latency` yaz.

### [SES-08] Kör test: başlangıç 0,5 s'den küçükse eksi kaymalar aranamıyor (A gecikmeli dosyaysa test kurulamaz)

- Önem: Düşük
- Güven: kesin
- Yer: `abx.rs:368-392`
- Kanıt: `b_start = (start − MAX_LAG_SECONDS).max(0.0)`; `expected = (start − b_start)·rate`; `find_offset` yalnızca
  `k ≥ 0` (B'nin A'dan **sonra** gelmesi) arar; `start = 0` iken `expected = 0`, A'nın B'den geç başladığı durum
  (A = MP3, B = FLAC; MP3'ün ~1105 örneklik kodlayıcı gecikmesi) hiç aranmaz → benzerlik düşer → "aynı kayda
  benzemiyor". Varsayılan başlangıç 30 s (`src/lib/abx.ts:9`) olduğu için nadir.
- Önerilen düzeltme: `start < MAX_LAG` ise A'yı `start + MAX_LAG`'dan çöz (iki tarafta da pay kalsın) ya da
  roller simetrik olarak iki yönde ara. Ek not: `MIN_TRIALS = 4` ile en iyi sonuç p = 1/16 = %6,25'tir; arayüz
  "anlamlı" diyemez — alt sınırın 5–6 olması ya da arayüzde açıklanması önerilir.

### [SES-09] Çözme hata yolu: `DecodeError` dışındaki hatalar (ör. `ResetRequired`) şarkıyı durdurur

- Önem: Düşük
- Güven: kesin
- Yer: `decode.rs:200-271`
- Kanıt: `Err(SymphoniaError::DecodeError(_)) => continue, Err(e) => return Err(e.into())`. symphonia,
  akış parametreleri değişince (zincirlenmiş OGG, bazı MKV'ler) `ResetRequired` döndürür ve çözücünün `reset()`
  ile sürmesini bekler; burada şarkı "Şarkı çözülürken hata oluştu" ile biter ve `decode_done` ile oturum kapanır.
- Önerilen düzeltme: `Err(SymphoniaError::ResetRequired) => { self.decoder.reset(); continue; }` (gerekirse
  parametreler değiştiyse çözücüyü yeniden kur).

### [SES-10] Bit-perfect oturumda sınırlayıcı ve kırpma yolda kalıyor; kayan noktalı kaynaklar da "bit-perfect" sayılıyor

- Önem: Düşük
- Güven: kesin
- Yer: `render.rs:186-190, 224-241` (bit-perfect dalda `write_frame` → `Limiter::process_frame` → `to_device` kırpma);
  `decode.rs:102-106` (`F32/F64 → source_bits = Some(32)`); `output.rs:80-87` (yuvarlama)
- Kanıt: Tam sayı kaynaklarda örnekler (−1, 1) içinde kaldığı için sınırlayıcı devreye girmez ve zincir bit bit
  aynıdır (test `bit_perfect_zinciri_…` bunu kanıtlıyor). Kayan noktalı WAV/CAF kaynakta |x| > 1 olabilir:
  sınırlayıcı kazancı düşürür, `encode` yuvarlar; arayüz yine "bit-perfect, 32 bit" der.
- Etki: Yalnızca kayan noktalı kaynaklar; nadir. Ancak "ses hiç işlenmez" vaadi bu durumda doğru değil.
- Önerilen düzeltme: `source_bits` kayan nokta için `None` döndürüp özel modu "bit-perfect" yerine "özel mod"
  olarak etiketlemek; ya da bit-perfect dalda sınırlayıcıyı atlamak (gecikme hattı yine ilerletilerek).

### [SES-11] Belge–kod uyumsuzluğu: README ve CLAUDE.md "crossfeed" var diyor, kodda yok

- Önem: Düşük
- Güven: kesin
- Yer: `README.md:15, 26-27`, `CLAUDE.md` ("Profesyonel ses: … crossfeed, bit-perfect mod"); `docs/ROADMAP.md:132`
  `- [ ] Crossfeed`
- Kanıt: `grep -rni crossfeed src-tauri/src src` boş.
- Önerilen düzeltme: README/CLAUDE.md'de "planlanan" olarak işaretle ya da kaldır.

### [SES-12] Test boşlukları

- Önem: Düşük
- Güven: kesin
- Yer: `tests/formats.rs` (yalnızca 22,05 kHz **mono** MP3/M4A/OGG/FLAC), `limiter.rs` testleri, `render.rs:386-512`
- Kanıt ve boşluklar:
  1. Gerçek kodeklerle **stereo** ve 44,1/48 kHz boşluksuzluk sınanmıyor; `aif/aiff/aifc/caf/mka/webm/aac(ADTS)`
     uzantıları ve 24 bit FLAC/ALAC için hiç dosya testi yok (`SUPPORTED_EXTENSIONS` 16 uzantı, test 4 biçim).
  2. Sınırlayıcı testleri yalnızca tavan ve kazanç adımını ölçer; **sınırlarken** eklenen bozulma (ör. 50 Hz'lik
     +3 dB taşan sinüste THD) ölçülmüyor — 1,5 ms'lik hızlı iniş + 80 ms bırakma, bas ağırlıklı yüksek basılmış
     kayıtlarda duyulabilir modülasyon üretebilir; bir sayı olmadan bilinmiyor.
  3. `ses_yolu_bellek_ayirmaz` kapsamı ses yolu için **tamdır** (EQ yeniden tasarım, PEQ çapraz geçiş, bas/derinlik/
     vuruş/küçük hoparlör, eşitleme bekleme + ölçüm gelişi + şarkı sınırı, duraklatma, veri yetişmeme, bit-perfect);
     allocation-counter kendi `#[global_allocator]`'ını kurduğu için ölçüm gerçektir. Kapsam dışı kalan yalnızca
     Windows'a özel döngü (`wasapi::run`: f32/tamsayı dönüşüm döngüleri, `fail(...)` kapanışları yalnızca hatada
     ayırır — okumayla doğrulandı) ve `simulated.rs`. Not: test 48 kHz'e sabit; tasarım hızdan bağımsız olduğu
     için kabul edilebilir.
  4. `her_bicimde_sarma_dogru_yere_gider` frekansı %6 toleransla ölçer; sarma hassasiyeti (±1 örnek) yalnızca WAV
     birim testinde (`sarma_ornek_hassasiyetinde_calisir`) sınanıyor; MP3/AAC'de `skip_until` yolu örnek düzeyinde
     doğrulanmıyor.
- Önerilen test: Stereo 44,1 kHz üç-ton fixture seti; sınırlayıcı için `fit()` ile THD+N ölçümü (eşik önerisi −40 dB);
  MP3'te sarma sonrası ilk örneğin fazını `alignment()` ile denetleme.

### [SES-13] `TrackLevels::count` okuma–yazma atomik değil; `set_loudness` erken gelirse sessizce düşer

- Önem: Düşük
- Güven: kesin
- Yer: `normalize.rs:125-132` (`load` → … → `store(index + 1)`), `:179-182`, `player.rs:1344-1353`
- Kanıt: Tek yazarlı olduğu için (çözme iş parçacığı) bugün doğru; ama bu varsayım tipte yazılı değil ve
  `begin_track` `pub`. `attach_next` önce `push_segment` sonra `begin_track` yapar; aradaki anda `sync_levels`
  `set_loudness(N)` çağırırsa `slot(N)` `None` döner ve ölçüm düşer (bir sonraki sorguda yeniden yazıldığı için
  kendini düzeltir).
- Önerilen düzeltme: `count.fetch_add(1, AcqRel)` ve belgeye "tek yazar" notu; `attach_next`'te sırayı
  `begin_track` → `push_segment` yap.

### [SES-14] Küçük tutarsızlıklar

- Önem: Düşük
- Güven: kesin
- (a) `eq.rs:67, 225-237`: arayüzdeki ekolayzer eğrisi her zaman 48 kHz için çizilir; 44,1/96 kHz'te 16 kHz bandı
  gerçekte farklı davranır (±0,5 dB). (b) `player.rs:290, 490, 494`: her yeni şarkı için dosya **üç kez** açılıp
  üç çözücü kurulur (oturum, hızlı ölçüm, tam analiz); SSD'de sorun değil, ağ sürücüsünde üç kat G/Ç.
  (c) `output/wasapi.rs:393` + `player.rs:407-417`: aygıt olayı hiç gelmezse `Session::drop` 2 s'ye kadar
  `wait_for_event`'i bekler — bu da oynatıcı kilidi altında (SES-02 ile birleşir). (d) `loudness.rs:281-288`:
  400 ms'den kısa ses tek blok sayılır — BS.1770 böyle bir durumu tanımlamaz; zararsız ama belgelenmeli.

### Şüpheler (kanıtlanamadı; güven: olası)

- **S-1 HE-AAC davranışı** (SES-05'in uzantısı): symphonia AAC-LC çözücüsünün HE-AAC dosyada açık hata mı verdiği
  yoksa çekirdeği 22,05 kHz olarak çözüp konteynerin 44,1 kHz zaman tabanıyla uyumsuz (iki kat hızlı/kısa) mı
  çaldığı gerçek dosyayla doğrulanmalı.
- **S-2 Paylaşımlı modda kanal dönüşümü** (SES-06): `AUTOCONVERTPCM` ile 6 kanallı akışın stereo aygıtta açılıp
  açılmadığı.
- **S-3 Bluetooth + özel mod**: Bluetooth A2DP aygıtları özel modu genellikle reddeder; `exclusive_reason`
  mesajı anlaşılır (`wasapi.rs:58-70`), ama bazı sürücüler `Initialize`'ı kabul edip ses vermez — gerçek cihazda
  denenmeli.
- **S-4 Olay zaman aşımı ve uyku**: bilgisayar uyuyup uyanınca paylaşımlı akışın olayı gelmeyebilir → 2 s sonra
  "aygıt yanıt vermiyor" hatası; SES-01'deki otomatik yeniden açma bunu da kapatır.

## 4. Güçlü yanlar

- **Gerçek zamanlı disiplin kanıtlı:** `render.rs` ve çağırdığı her modül (`eq`, `peq`, `bass`, `punch`,
  `normalize::Leveler`, `limiter`) belleği yalnızca `new` içinde ayırır; ayar kanalları atomik (`EqControl`,
  `PeqControl`, `LoudnessControl`, `TrackLevels`) ve bellek sıralaması doğru (değerler Relaxed, sürüm Release;
  okuyucu sürümü Acquire). `ses_yolu_bellek_ayirmaz` bunu 250 blok boyunca ayar değişimleri, ölçüm gelişi, şarkı
  sınırı, duraklatma ve veri yetişmeme dahil ölçer (`render.rs:386-512`). Panik yolu yok: bütün indeksler
  `get_mut`/`zip`/`take` ile sınırlı; `to_device` NaN'ı sessizliğe çevirir (`render.rs:235-241`); çözücü
  NaN/sonsuz örnekleri girişte temizler (`decode.rs:232-238`).
- **DSP matematiği doğru:** RBJ tepe/raf/alçak/yüksek geçiren katsayıları Cookbook ile bire bir (`biquad.rs:57-131`);
  BS.1770-4 K-ağırlıklama her hız için bilinear türetimle üretilir ve 48 kHz'te tabloyla 1e-10 içinde aynıdır
  (hesapla doğrulandı); kapılama (−70 LUFS mutlak, −10 LU göreli, 400 ms/%75 örtüşme) standarda uygun; gerçek
  tepe 4× (96 kHz üstünde 2×) çok fazlı pencereli sinc ile ölçülür ve 20 kHz'e kadar −0,22 dB içinde doğru;
  EBU 3341 durum 1–5 testleri ±0,1 LU ile var (`loudness.rs:353-393`). Sınırlayıcının tavan garantisi matematiksel
  olarak sağlam (yukarıda kanıt) ve 2 s'lik kötü niyetli sinyalde tam 1,0 ölçüldü.
- **Ekolayzer tasarımı:** Välimäki–Liski etkileşim matrisi + iki geçişli en küçük kareler, sabit boyutlu dizilerle
  ses iş parçacığında yeniden tasarlanabilir (`eq.rs:536-594`); 7 örnekleme hızı × 308 ayar için sapma < 1,2 dB
  testle sabitlenmiş; "bütün sürgüler +6 → ses +6" vaadi test ediliyor.
- **Boşluksuz geçiş gerçekten boşluksuz:** MP4 dolgusu dosyadan okunur (`iTunSMPB` ve `elst`, 16 MB kutu sınırı,
  derinlik sınırı — `gapless.rs`), MP3/Vorbis dolgusunu symphonia kırpar ve kod `pts + trim_start` ile buna uyar
  (symphonia-core 0.6.1 `packet.rs:40-72` ile doğrulandı); aynı hızdaki şarkılar **tek** yeniden örnekleyiciden
  geçer (sınırda dalga kopmaz), yeniden örnekleyici gecikmesi başta atılır ve toplam uzunluk giriş süresine eşitlenir
  (`resample.rs:131-187`); eşitleme kazancı tam sınırda, 10 ms'lik doğrusal geçişle değişir (`normalize.rs:378-430`).
- **Çıkış kipi kararları doğru:** paylaşımlı modda aygıtın kendi hızına kendi yüksek kaliteli dönüştürücüsüyle
  çevirir (Windows SRC'ye bırakmaz); özel modda 128 bayt hizalı periyot, en az 100 ms arabellek, en yüksek tamsayı
  biçimi önce denenir, şarkıdan dar bit derinliği kabul edilmez (#79), kabul edilen biçim (kanal maskesi dahil)
  kullanılır; tamsayı kodlama 16/24 bit örnekleri bit bit korur (`output.rs:80-87`, testler). Duraklatma/kapatma
  "tık" üretmez: geçişin hoparlöre ulaştığı `FadeWatch` ile izlenir.
- **Kaynak yönetimi:** `Session`, `Analysis` ve ABX `Playback` `Drop`'ta iş parçacıklarını durdurup birleştirir;
  çıkış açılamazsa çözme iş parçacığı geri alınır; `unsafe` yok (wasapi crate'inin içinde 100 `unsafe` blok var,
  projenin kodu temiz).
- **Hata iletileri Türkçe ve eyleme dönük** (ör. özel mod izni için Windows ayar yolu, `wasapi.rs:58-70`).

## 5. Denetlenemeyenler ve sınırlar

- Windows'a özel kod (`output/wasapi.rs`) Linux'ta çalıştırılamadı; aygıt biçimi pazarlığı, olay zamanlaması,
  özel modun gerçek sürücülerdeki davranışı, kanal dönüşümü (S-2) ve aygıt değişimi (SES-01) yalnızca kod okuması
  ve wasapi crate kaynağıyla değerlendirildi. `examples/ses_denemesi.rs` Windows CI'da çalışıyor ama aygıt değişimi,
  yük altında takılma ve özel modu denemiyor.
- Derleme/test çalıştırma izni olmadığı için testlerin geçtiği varsayıldı (CI yeşil varsayımı); sayısal doğrulamalar
  python3 ile formüllerin yeniden hesaplanmasıdır, Rust çıktısıyla karşılaştırma değildir.
- Gerçek HE-AAC/Opus/5.1 dosyalarıyla davranış (S-1, S-2) doğrulanamadı.
- Zamanlama iddiaları (SES-02'deki 150–400 ms, SES-04'teki 1–3 s çözme) kod yoluna ve tipik donanım hızlarına
  dayanır; makineye göre değişir.

## 6. Önerilen öncelik sırası

1. **SES-01** — aygıt değişimini izle, kayıpta otomatik yeniden aç (kullanıcının gerçek cihazda ilk göreceği sorun).
2. **SES-02** — sarma/açma yolunu kilit dışına taşı (görsellerdeki donma; SES-14c'yi de kapatır).
3. **SES-04** — ilk ses beklemesini kısalt / erken tahmin yayımla (varsayılan deneyimi etkiler).
4. **SES-03** — ses iş parçacığına öncelik, analiz iş parçacıklarına düşük öncelik ("sıfır takılma" hedefi).
5. **SES-05** — uzantı listesini kodek yeteneğiyle hizala, HE-AAC/Opus için açık hata (gerçek dosyayla doğrula, S-1).
6. **SES-06** — çok kanallı indirgeme ve BS.1770 ağırlıkları (S-2 ile birlikte).
7. **SES-07, SES-09, SES-13** — küçük doğruluk düzeltmeleri (konum, `ResetRequired`, atomik sayaç).
8. **SES-12** — test boşlukları (stereo gerçek kodek fixture'ları, sınırlayıcı THD).
9. **SES-08, SES-10, SES-11, SES-14** — belge ve etiket düzeltmeleri.
