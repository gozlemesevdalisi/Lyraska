# Şarkı analizi, Görsel Yönetmen ve görsel köprü — Bağımsız denetim raporu

Denetçi: bağımsız denetçi analiz. Kaynak: main @ 25f83f8. Tarih: 16 Ekim 2026.

## 1. Kapsam ve yöntem

İncelenen dosyalar (hepsi `(depo kökü)/src-tauri/src/` altında, satır satır okundu):

| Dosya                   | Satır | Dosya                        | Satır |
| ----------------------- | ----: | ---------------------------- | ----: |
| analysis/spectrogram.rs |  1183 | analysis/key.rs              |   800 |
| analysis/beats.rs       |   674 | analysis/cache.rs            |   748 |
| analysis/structure.rs   |   949 | analysis/background.rs       |   460 |
| analysis/meter.rs       |   459 | analysis/evaluate.rs         |   282 |
| analysis/levels.rs      |   206 | analysis/annotation.rs       |   325 |
| analysis/mod.rs         |    48 | director/mod.rs              |   516 |
| visual_bridge/mod.rs    |   188 | visual_bridge/calibration.rs |   122 |

Toplam ≈ 6 960 satır; ayrıca `audio/player.rs` içinden yalnızca `Analysis::start/refresh/drop`,
`adopt_analysis`, `visual_now`, `now_playing` ve `VisualData`/`StructureNow` (≈ 250 satır); arayüz tarafından
yalnızca etkileşim noktaları (`src/lib/sync.ts`, `src/hooks/useVisualFeed.ts`, `src/lib/flash.ts`,
`src/lib/sky.ts` ve `highway.ts` içindeki `pulse/accent/barPhase/safe` kullanımları), `commands.rs`'te
`visual_frame`, `annotation_*`; `storage.rs`'te `analyses` tablosu göçleri. Okunan belgeler: `CLAUDE.md`,
`docs/MIMARI.md`, `docs/ROADMAP.md`, `src-tauri/tests/data/README.md`.

Karşılaştırılan standart ve yayınlar: Ellis 2007 (dinamik programlamayla beat takibi: arama aralığı, sıkılık,
tempo önseli), Foote 2000 (dama tahtası yenilik çekirdeği), Krumhansl–Kessler 1982 ton profilleri ve
Krumhansl–Schmuckler algoritması, Camelot çarkı, MIREX beat F-ölçüsü (±70 ms), WCAG 2.3.1 / 2.3.2 (saniyede en
fazla 3 parlama) ve ISO 9241-391 ışık hassasiyeti ilkeleri, SQLite WAL/busy_timeout davranışı, Rust eşzamanlılık
kuralları.

Çalıştırılanlar: derleme/test yok (izin verilmedi). `python3 -I` ile sayısal doğrulama betiği
(`scratchpad/denetim2/anl_check.py`): her örnekleme hızında bant→FFT kutusu eşlemesi ve pencere kapsaması,
başlangıç gücü bantları, `onset_latency` değerleri, spektral kare merkezinin ekrana göre gecikmesi, Camelot
formülünün 24 ton için doğrulaması, Foote çekirdeğinin sıfır toplamı, Yönetmen'in nabız + vurgu birleşik olay
sayısının 60–200 BPM × 3/4, 4/4 × her faz ve bütün aksak gruplamalar için 1 sn'lik en kötü penceresi.

## 2. Genel değerlendirme

Puan: **7/10.** Analiz katmanı, bu büyüklükte bir açık kaynak projede nadir görülen bir özenle yazılmış: bütün
hesaplar tek geçişte (çözme bir kez), sonuçlar parça sürümlü bir önbellekte, vuruşlar örnekleme hızına bağlı
gecikme düşülerek ses ile hizalı, drop/ölçü/ton için güven değerleri tek eşikle her yerde aynı, nabız olayları
seyrekleştirilerek parlama sınırı matematiksel olarak tutuluyor, yarıda kesilen çözme saklanmıyor, iptal ve
arka plan hız ayarı düşünülmüş. Krumhansl–Kessler profilleri ve Camelot formülü yayınla birebir doğru. En zayıf
yan, bütün FFT boyutlarının **örnek sayısıyla sabit** olması: hi-res (176,4/192 kHz) dosyalarda analiz
penceresi kare adımından kısa kalıyor (sesin üçte biri hiç analiz edilmiyor) ve bas bantları tek kutuya
çöküyor; 44,1/48 kHz'te bile en alt bantlar birbirinin kopyası. Bir numaralı risk, bütün doğruluk kanıtının
**sentetik** kayıtlara dayanması: tempo değişimi, aksak ölçünün gerçek icrası, drop çeşitliliği hiçbir testte
yok; ürünün "şarkıyı önceden bilen görseller" vaadi gerçek müzikte henüz ölçülmemiş durumda (yol haritası da
bunu "işaretleme aracıyla ölçülecek" diye kabul ediyor).

## 3. Bulgular (önem sırasıyla)

### [ANL-01] 176,4/192 kHz'te FFT penceresi kare adımından kısa: sesin %30–36'sı hiç analiz edilmiyor

- Önem: Orta
- Güven: kesin (hesapla kanıtlandı)
- Yer: analysis/spectrogram.rs:31 (`FFT_SIZE = 2048`), :430–494 (dairesel pencere, her `frame_end`'de bir FFT),
  :640–643 (`frame_end`); analysis/beats.rs:46–53 (`onset_latency`), :639–665 (test yalnızca 8–96 kHz)
- Kanıt: kare adımı `sample_rate / 60` örnek, pencere sabit 2048 örnek. 192 kHz'te adım 3 200, pencere 2 048 →
  her karede 1 152 örnek (6 ms) hiçbir pencereye girmiyor (kapsama %64; 176,4 kHz'te %70). Hann ağırlığı pencere
  uçlarında sıfıra indiği için etkin kapsama daha da az. Aynı hızda FFT kutusu 93,75 Hz: 32 bandın ilk **8'i**
  aynı kutuya (1 numaralı, 94 Hz) eşleniyor (`ranges` 44,1 kHz'te 30/32 farklı, 192 kHz'te 22/32). Başlangıç
  gücünün 48 bandının ilk 39'u nominal frekansının üstüne itiliyor (en alt bant 94 Hz). `onset_latency` doğrusu
  8–96 kHz ölçümlerinden çıkarılmış; 192 kHz'teki −4,5 ms değer dışkestirim, testi yok.
- Etki: Bit-perfect/hi-res kullanıcısında (ürünün hedef kitlesi) kısa davul vuruşları aradaki boşluğa düşerse
  başlangıç gücünde görünmez: vuruş takibi, drop ve ölçü tespiti aynı şarkının 44,1 kHz kopyasından daha kötü;
  bas görselleri (ilk 8 bant) tek bir değer gösterir; ses yüksekliği ölçümü etkilenmez.
- Yeniden üretme / önerilen test: `her_orneklemede_vuruslar_sesle_senkron` listesine 192 000 ekleyin ve
  `drum_track`'i 192 kHz'te üretip F-ölçüsü ile ortalama hatayı 44,1 kHz sonucuyla karşılaştırın; ayrıca
  `baslangic_bantlari_her_orneklemede_gecerli` testine "ardışık bantların en az %80'i farklı kutu aralığı" koşulu.
- Önerilen düzeltme: FFT boyutunu süreye göre seçin (ör. `2^round(log2(0.046 · sample_rate))`, 44,1 kHz'te
  yine 2048) ya da analizden önce 48 kHz'e indirgeyin (`resample.rs` var). Spektrum sürümü (`VERSIONS.spectrum`)
  artırılmalı; `onset_latency` doğrusu süreye bağlı olduğu için pencere süresi sabitlenince tek sabite iner.

### [ANL-02] 44,1/48/96 kHz'te en alt spektrum bantları birbirinin kopyası, 0. bant 30 Hz'in altını gösteriyor

- Önem: Orta
- Güven: kesin
- Yer: analysis/spectrogram.rs:667–677 (`ranges`: `round` + `clamp(start + 1, …)`), :704–709 (bant = aralıktaki
  en güçlü kutu)
- Kanıt: 44,1 kHz'te kutu 21,53 Hz. Bant 0 (30–36,5 Hz) → `(1, 2)` yani 21,5 Hz'lik kutu (bandın dışı); bant 1 ve
  2 → ikisi de `(2, 3)`; bant 3 ve 4 → ikisi de `(3, 4)`. 48 kHz'te bant 1–3 aynı (29/32 farklı), 96 kHz'te bant
  0–4 aynı (26/32 farklı). Hesap betikte; `sinus_dogru_bantta_en_yuksek_cikar` testi 1 kHz'i sınadığı için bunu
  yakalamıyor.
- Etki: Spektrum sahnesinde en soldaki 3–5 çubuk aynı hareketi yapar; `structure.rs` ve `meter.rs` "bas" ölçümünü
  ilk 8 bandın ortalamasından aldığı için (`BASS_BANDS = 8`) bas vuruşu ağırlığı fiilen 4–5 kutudan geliyor.
  Görsel kalite kaybı; 32 bantlık logaritmik tasarımın vaadi alt oktavda tutmuyor.
- Önerilen test: `band_edges`/`ranges` için "her bandın kutu aralığı bir öncekiyle aynı olmamalı; aralığın merkezi
  bandın sınırları içinde olmalı" birim testi (44 100, 48 000, 96 000).
- Önerilen düzeltme: Alt bantlar için kutu enterpolasyonu (bandın sınırları arasında Goertzel ya da sıfır
  doldurmalı daha uzun FFT) ya da en alt bantları birleştirip bant sayısını koruyarak 30 Hz'in altını sabit kabul
  etmek; en azından `start`'ı `floor`, `end`'i `ceil` ile alıp bandı kendi sınırlarının içinde tutmak.

### [ANL-03] Spektral karenin ses içeriği ekrana göre örnekleme hızına bağlı gecikmeli; vuruşlar düzeltiliyor, kareler düzeltilmiyor

- Önem: Orta
- Güven: kesin (hesap)
- Yer: analysis/spectrogram.rs:171–186 (`frame_at`: `floor(seconds · 60)`), :476–478 (yorum: "ortası k/60'ın
  ~6 ms gerisinde"); visual_bridge/mod.rs:26, :38–41 (`visual_time`: +1/60 ekran öncülü); audio/player.rs:1049–1055
- Kanıt: k. karenin penceresi (k+1)/60'ta biter; merkezi 44,1 kHz'te (k+1)/60 − 23,2 ms. Arayüz `heard + 16,7 ms`
  anının karesini okur ve bir kare sonra gösterir; gösterim anına göre içerik gecikmesi 44,1 kHz'te 6,6–23,2 ms
  (ortalama **14,9 ms**), 48 kHz'te 13,0, 22,05 kHz'te 38,1, 8 kHz'te 120 ms; 96 kHz'te 2,4, 192 kHz'te −3,0 ms
  (görsel sesin önünde). Vuruşlar için aynı sorun `onset_latency` ile düşülmüş (beats.rs:179), kareler için
  değil. VU seviyeleri (`meters_at`) ise karenin kendi aralığından geldiği için ~8 ms **önde**.
- Etki: Yol haritasındaki ±20 ms hedefinin dörtte üçü daha arayüz ve ekran gecikmesi eklenmeden spektrumda
  harcanıyor; 44,1 kHz ile 192 kHz şarkı arasında 18 ms fark var; spektrum ve VU zıt yönde kayıyor.
- Önerilen test: `kare_zamani_sesle_hizali` testini keskin bir geçişle (sessizlik → 1 kHz) ve 1 ms çözünürlükte
  yazın: geçişin karede göründüğü an ile gerçek an farkı her hızda |Δ| ≤ 8 ms olsun.
- Önerilen düzeltme: `frame_at`/`meters_at` okurken `seconds + FFT_SIZE / (2 · sample_rate) − 1/(2·60)` kaydırması
  (spektrogram örnekleme hızını zaten saklıyor) ya da karenin zaman damgasını pencere merkezi olarak tanımlayıp
  `frame_end`'i buna göre kaydırmak (spektrum sürümü artar).

### [ANL-04] Tek küresel tempo: tempo değişen şarkıda BPM yanlış, vuruşlar sabit periyoda zorlanıyor; testi yok

- Önem: Orta
- Güven: yüksek
- Yer: analysis/beats.rs:164–184 (`track`: tek `period`), :224–236 (`mean_interval`: bütün vuruşlara tek doğru),
  :326–365 (`dynamic_beats`: sabit `period`, `TIGHTNESS = 100`), :285–289 (tempo önseli)
- Kanıt: `estimate_period` bütün şarkının öz-ilintisinden tek bir periyot seçer; DP her aralığı bu periyota göre
  cezalandırır (`−100 · ln(gap)²`; %10 sapma = 0,9 puan, %30 sapma = 6,9 puan — z-puanlı başlangıç tepeleri
  3–6 düzeyinde, yani %30 üstü sapmada vuruş atlanır). `bpm` en küçük kareler doğrusunun eğimi: 120 → 140 BPM'e
  geçen bir şarkıda ~130 çıkar; kütüphane bu değeri gösterir. Ellis 2007 de tek tempo kullanır ama bunu açıkça
  sınırlama olarak verir; burada belge/yol haritası sınırlamayı anmıyor. Hiçbir test tempo kayması, ritardando ya
  da iki tempolu kayıt içermiyor. Önsel genişliği 1,0 oktav ve sıkılık 100, Ellis'in 0,9–1,4 oktav / 400–680
  değerlerinden farklı; normalizasyon farklı olduğundan karşılaştırma kesin değil, ama seçimin gerekçesi ve
  gerçek müzikte ölçümü yok.
- Etki: Canlı kayıt, rock, klasik ve çoğu akustik şarkıda vuruş ızgarası şarkının bir yarısında kayar; Yönetmen'in
  nabzı ve otoyol lambaları müziğin yanına düşer; kütüphanedeki BPM yanıltıcı olur.
- Önerilen test: `drum_track`'i iki parçadan (ör. 16 sn 112 BPM + 16 sn 132 BPM, ayrıca doğrusal ritardando)
  üretip her parçada ayrı F-ölçüsü ≥ 0,9 ve `bpm` yerine parça başına tempo bekleyin.
- Önerilen düzeltme: En küçük değişiklik: `mean_interval` yerine ortanca aralık + arayüze "tempo değişken"
  bayrağı (ortanca mutlak sapma eşiği). Asıl çözüm: periyodu zaman penceresine göre (ör. 8 sn'lik pencereler)
  tahmin edip DP'de yerel periyot kullanmak (Ellis'in "tempo değişimine izin veren" varyantı).

### [ANL-05] Aksak ölçü tespiti ızgaradaki tek bir kaymaya dayanıksız; eşikler yalnızca sentetik kayıtla ayarlı

- Önem: Orta
- Güven: olası
- Yer: analysis/meter.rs:167–185 (`fit`: kalıp şarkı boyunca tek fazla döşeniyor), :89–130 (`detect`,
  `MIN_CORRELATION 0,35`, `MIN_MARGIN 0,12`), :102 (`tatum` yalnızca `beat/2` ve `beat`)
- Kanıt: `pattern[k] = template[(k − phase) mod n]` bütün sekizlik dizisi için tek faz; `track_period`'ın DP'si
  bir sekizliği atlar ya da ekler (periyodun yarısıyla iki katı arasında aralığa izin veriyor, beats.rs:328–329)
  ise o noktadan sonra kalıp ters döner ve ilinti şarkı ortalamasında sıfıra iner; aksak bulunmaz (yanlış
  negatif). Tersine, `MIN_MARGIN` ile düzenli kalıplardan açık ara üstünlük isteniyor, bu yanlış pozitifi
  bastırıyor; ama REGULAR listesinde 12/8 (3+3+3+3) ve 2+2 yok, senkoplu 4/4 (3+3+2 tresillo) için düzenli kalıp
  zaten kötü uyar. Testler (`dokuz_sekizlik_bulunur`, `yedi_ve_bes_sekizlik_bulunur`) kusursuz metronomik
  sentetik davul kullanıyor; tek bir gerçek icra yok.
- Etki: Gerçek 9/8 (zeybek, karşılama) kayıtlarında büyük olasılıkla aksak bulunmaz ve takipçinin eşit aralıklı
  (yanlış) vuruşları kalır: ürünün Türk müziği için öne çıkardığı özellik gerçek dosyada sessizce devre dışı
  kalabilir.
- Önerilen test: `aksak_drums`'a rastgele bir sekizlik düşürme/ekleme (%2) ve ±%3 tempo dalgası ekleyip tespitin
  sürdüğünü sınayın; ayrıca 12/8 ve tresillo 4/4 kayıtlarında `None` beklendiğini.
- Önerilen düzeltme: İlintiyi 2–4 ölçülük pencerelerde yerel olarak hesaplayıp faz oylaması yapın (kayma
  sonrası pencereler kendi fazını bulur); REGULAR'a `[3,3,3,3]` ve `[2,2]` ekleyin.

### [ANL-06] Önbellek kilidi ile Player kilidi iç içe: arayüz komutu 5 sn'ye kadar bekleyebilir

- Önem: Orta
- Güven: olası
- Yer: audio/player.rs:468–489 (`Analysis::start` → `cache.load` Player kilidi altında), :572–579
  (`Analysis::drop` → `thread.join()`), :1044–1051 (`visual_now` her karede `adopt_analysis`); analysis/cache.rs:172
  (`busy_timeout 5 s`), :181–183 (tek `Mutex<Connection>`), :301 (`store`); analysis/background.rs:236
- Kanıt: `visual_frame` komutu `player.lock()?.visual_now()` (commands.rs:209–211). Boşluksuz geçişte
  `adopt_analysis` eski `Analysis`'i düşürür → `join`; analiz iş parçacığı o anda `cache.store()` içinde ise
  önbelleğin `Mutex`'ini bekler; `Mutex` arka plan analizinin `store()`'unda olabilir ve o da kütüphane taraması
  yazarken SQLite `busy_timeout` ile **5 sn'ye kadar** bekler. Aynı zincir `open_track` → `Analysis::start` →
  `cache.load` için de geçerli (WAL'da okuma yazarı beklemez, ama Rust `Mutex` bekler).
- Etki: Tarama + arka plan analizi + şarkı geçişi çakıştığında görseller donar ve komutlar yanıt vermez
  (ses akışı etkilenmez: gerçek zamanlı yol ayrı). Nadir ama kullanıcı için "program takıldı" izlenimi.
- Önerilen test: Arka plan iş parçacığında `BEGIN IMMEDIATE` tutarken `visual_now` süresini ölçen bir test
  (`onbellek_yazarken_kutuphane_taramasi_bekler_hata_vermez` benzeri) — hedef < 50 ms.
- Önerilen düzeltme: Önbellek yazımlarını ayrı bir kuyruk iş parçacığına verin (ya da arka plan ve oynatıcı için
  ayrı `Connection`); `Analysis::drop`'ta `join` yerine iş parçacığını ayrık bırakıp `cancelled` bayrağına güvenin
  (`saved()` iptalde zaten `None`).

### [ANL-07] Doğruluk kanıtı yalnızca sentetik sinyallerden: testler neyi ölçmüyor

- Önem: Orta
- Güven: kesin (test kodu)
- Yer: analysis/beats.rs:491–575 (`drum_track`), analysis/meter.rs:277–339, analysis/structure.rs:714–789
  (`edm_track`), analysis/key.rs:521–565 (`music`: tam akortlu 6 harmonikli üçlüler), tests/data/README.md
- Kanıt: Bütün ritim/yapı/ton testleri programın kendi ürettiği, metronomik, gürültüsüz (ya da beyaz gürültülü)
  sinyaller. Ölçülmeyenler: tempo değişimi (ANL-04), ızgara kayması (ANL-05), 192 kHz (ANL-01), vokal/hi-hat
  yoğun müzikte başlangıç gücü, bas sürekli olan şarkıda drop (bkz. ANL-12), ton değişimi dışında modülasyon
  (örn. armonik minör/dorik), kaydırılmış vurgulu (swing) 4/4, "nabız + vurgu" birleşik parlama sayısı (ANL-09),
  `mood`/`warmth` eğrilerinin parlama sayacı. Depoya ses girmeme kuralı (telif) doğru, ama bu açığı bir
  "işaret dosyası + yerel şarkı" ile çalışan, CI dışı ölçüm betiği kapatabilirdi; yol haritası bunu proje
  sahibinin işaretlemesine bırakıyor (henüz sonuç yok).
- Etki: F ≥ 0,80, ton isabeti ≥ %80 hedefleri bugün yalnızca sentetik verideki başarıyla "yeşil"; gerçek müzikte
  gerileme fark edilmez.
- Önerilen test: `evaluate_file` üzerinden çalışan bir `examples/dogruluk.rs` (yerel klasördeki şarkılar + işaret
  dosyaları → F, drop isabeti, ton isabeti tablosu) ve bu raporda önerilen sentetik bozulma testleri.
- Önerilen düzeltme: Test verisine lisansı temiz (CC0/kendi üretimi) 3–4 gerçek icra (davul + bas kaydı; 9/8
  darbuka) eklenmesi; `check:audio` bunları beyaz listeye alır.

### [ANL-08] Görsel Yönetmen'in nabız garantisi kanal başına; birleşik olaylar tam WCAG sınırında ve testsiz

- Önem: Düşük
- Güven: kesin (hesap)
- Yer: director/mod.rs:19–23 (belge: "nabız olayları (vuruş, ölçü başı, boşalma) arasında en az
  MIN_PULSE_INTERVAL"), :125, :130–139 (`thin` ayrı ayrı: `pulses`, `safe_pulses`, `accents`), :205–211,
  :397–417 (test yalnızca `pulses` dizisini ve `pulse` yükselişlerini sayıyor)
- Kanıt: `accents` ölçü başlarından ayrıca seyrekleştiriliyor; 176–200 BPM'de `pulses` çift vuruşları alır,
  ölçü başı tek vuruşa düşerse (`downbeat_phase` tek) vurgu nabzın arasına gelir. Betikle 60–200 BPM × {3/4, 4/4}
  × her faz ve bütün aksak gruplamalar için birleşik olay sayısı tarandı: **en kötü 3 olay / 1 sn** (ör. 120 BPM
  3/4; 200 BPM: 0, 0,3, 0,6). WCAG 2.3.1 "üçten fazla" dediği için sınır aşılmıyor, ama garanti kodda değil
  rastlantıda: `thin` aralığı 0,34'ten 0,30'a inse ya da `MAX_BPM` artsa 4'e çıkar. Güvenli modda ayrıca `release`
  (drop) seyrekleştirilmiş bir ölçü başına düşerse o saniyede 2 olay olur (≤ 1/sn vaadi drop anında bozulur;
  `release` yavaş söndüğü için parlama sayılmaz, yine de belgeyle uyumsuz).
- Etki: Bugün güvenli; parametre değişiminde sessizce bozulur. Belge "bütün nabız olayları" derken kod kanal başına
  tutuyor.
- Önerilen test: `nabiz_saniyede_ucten_fazla_olmaz` testine `pulses ∪ accents ∪ drops` birleşiminin herhangi bir
  1 sn penceresinde ≤ 3 koşulu (60–200 BPM, 3/4 ve 4/4, her faz).
- Önerilen düzeltme: `accents`'i `pulses`'ın alt kümesi yapın (ölçü başı nabız dizisine giremediyse vurgu da
  olmasın) ya da `thin`'i birleşik diziye uygulayın.

### [ANL-09] `Rhythm.beat_phase` güvenli modda da vuruş hızında süreksiz; parlaklığa bağlanırsa sınır aşılır

- Önem: Düşük
- Güven: yüksek
- Yer: director/mod.rs:75–77, :213–228; src/lib/highway.ts:162 (yalnızca `barPhase`, kilitli ve hız sınırlı);
  src/lib/sky.ts:183–186 (`pulse` sahnede **spektrum basından** türetiliyor, Yönetmen'den değil)
- Kanıt: `beat_phase` her vuruşta 1 → 0 testere dişi; `safe` parametresi onu değiştirmiyor. Bugün sahneler
  parlaklığı buna bağlamıyor; ancak gece göğünün `pulse`'ı spektrum bantlarından geliyor (sky.ts:183) ve
  Yönetmen'in seyrekleştirmesinin dışında — sahne bunu kendi parlama testiyle kapatıyor (flash.ts), Yönetmen
  katmanının garantisi bu yolu kapsamıyor.
- Etki: API tuzağı: yeni bir sahne `beatPhase`'i parlaklığa bağlarsa 200 BPM'de 3,3 Hz parlama, güvenli modda da.
- Önerilen düzeltme: `DirectorFrame` belgesine "parlaklık yalnızca `pulse`, `accent`, `release` ve yumuşak
  değerlerden; `beat_phase`/`bar_phase` yalnızca konum için" kuralını yazın ve `check:scenes`/`flash.test`'te
  sahne girdisi olarak `beatPhase`'i sentetik 200 BPM ile besleyen bir parlama testi ekleyin.

### [ANL-10] Ekran öncülü 60 Hz'e sabit; kalibrasyon insan erken basmasını ve klavye gecikmesini ayırmıyor

- Önem: Düşük
- Güven: yüksek
- Yer: visual_bridge/mod.rs:26 (`DISPLAY_LEAD_SECONDS = 1/60`); src/lib/sync.ts:39–60 (`estimateAudioDelay`:
  ortanca + ortanca mutlak sapma, en az 8 basış, 40 ms dağınıklık eşiği)
- Kanıt: İşaret ve büyüklük doğru: `heard + 1/60 − delay` (aygıt geç çalıyorsa görsel geriden okunur).
  Ancak 120/144 Hz ekranda öncül 8–10 ms fazla, 30 Hz'de eksik. Tıklama kaydıyla ölçüm, basışın tıklamanın
  şarkıdaki zamanından farkını alır: bu fark aygıt gecikmesi + klavye/olay gecikmesi (10–30 ms) + öngörülebilir
  metronoma insanın **erken** basması (literatürde −20…−50 ms "negative mean asynchrony") toplamıdır; bu
  bileşenler çıkarılmıyor, sıfır noktası yok. İstatistik (ortanca, MAD, en az 8 basış) seçimleri yerinde.
- Etki: Kalibrasyon sonucu sistematik olarak 10–40 ms sapabilir; hedef ±20 ms'nin kendisi kadar.
- Önerilen düzeltme: Öncülü `requestAnimationFrame` aralığından ölçün (arayüz zaten `dt` biliyor); kalibrasyon
  ekranında "kablolu kulaklıkla bir kez ölçüp sıfır kabul et, Bluetooth'ta farkı al" akışı (fark alma
  insan/klavye payını düşürür).

### [ANL-11] Ton: akort tahmini harmonikleri de kullanıyor; örtüşmesiz Hann; 192 kHz'te kutu yarım tondan geniş

- Önem: Düşük
- Güven: yüksek
- Yer: analysis/key.rs:344 (`size` 4096–32768'e kırpılıyor), :359 (`hop = size`), :447–451 (akort: ≥ 200 Hz bütün
  tepeler), :441–443
- Kanıt: 192 kHz'te `size` = 32 768 → kutu 5,86 Hz; C2'de yarım ton 3,9 Hz (yorum :40 hedefi 2,7 Hz). Parabol
  inceltmesi kısmen telafi eder ama komşu tepeler karışır. Akort: 5. harmonik −14 sent, 7. −31 sent
  eşit tamperamandan sapar; `weight = √magnitude` ile toplamda ortalamayı aşağı çeker (test 6 harmonikle 5 sent
  içinde geçiyor; gerçek çalgıda 7.–9. harmonik de var). Örtüşmesiz Hann'da pencere uçlarındaki notalar yarı
  ağırlıkta; kısa akorlar (< 0,37 sn) rastgele kaçabilir. Krumhansl–Kessler sayıları (6,35 … 2,88 / 6,33 … 3,17)
  ve Camelot formülü (`((7·majör_tonik mod 12) + 7) mod 12 + 1`, minörde +3) 24 ton için doğrulandı: doğru.
- Etki: Hi-res ve kısa akorlu müzikte güven düşer (gösterilmez), yanlış ton çıkma olasılığı küçük artar.
- Önerilen düzeltme: `size` kırpmasını 65 536'ya çıkarın ya da kromayı 48 kHz'e indirgenmiş sinyalden alın;
  akort için yalnızca 2.–4. harmonik aralığı ya da tepe/temel oranı kontrolü; `hop = size / 2`.

### [ANL-12] Drop tespiti mutlak eşiklere bağlı; güven hesabında aynı kanıt iki kez sayılıyor

- Önem: Düşük
- Güven: olası
- Yer: analysis/structure.rs:521–546 (`find_drops`: `jump ≥ 0,12`, `after_energy ≥ 0,65`, `before_energy > 0,05`),
  :553–570 (`drop_confidence`: `boundary` payı %20), :288 (`section_boundaries` girdisi `bar_features` içinde
  `bar_energy` ve `bar_bass` 6'şar kez)
- Kanıt: Bas sürekli olan (yükselişte de bas çalan) şarkıda `jump` 7 dB'yi bulmaz → drop yok (yanlış negatif);
  rock'ta bassız nakarat öncesi → her nakarat aday, güven eşiği (0,6) eler ama `boundary` terimi bölüm sınırını
  **aynı** bas/enerji sıçramasından türetir (bağımsız kanıt değil). Sentetik `edm_track` tek desen.
- Etki: Drop sayacı ve Yönetmen'in beklentisi bazı türlerde hiç çalışmaz; bazılarında güven yapay yüksek.
- Önerilen test: `edm_track`'e "bas yükselişte de var, yalnızca davul ve genişlik dönüyor" varyantı; ve 16 ölçüde
  bir tekrarlayan rock nakaratı → en fazla 1 güvenli drop.
- Önerilen düzeltme: Bas sıçramasını mutlak yerine şarkının bas dağılımına göre (z-puanı) ölçün; `boundary`
  terimini yalnızca tını (bant) sütunlarından hesaplanan ayrı bir yenilikle besleyin.

### [ANL-13] Arka plan analizi "düşük öncelikli" ama işletim sistemi önceliği değiştirilmiyor; Pacer tek çekirdeğin yarısını varsayıyor

- Önem: Düşük
- Güven: kesin
- Yer: analysis/background.rs:6–11 (belge), :78–82 (`thread::Builder` yalnızca ad), :246–270 (`Pacer`),
  :172–176 (`wait_for_foreground` 100 ms yoklama)
- Kanıt: Hiçbir yerde `SetThreadPriority`/`thread_priority` çağrısı yok; "öncelik" yalnızca %50 görev döngüsü
  (20 ms iş / 20 ms uyku). Oynatıcının kendi analiz iş parçacığı (`lyraska-analiz`) tam hızda normal öncelikte.
  Dizüstünde pil/termal etkisi ve arayüz iş parçacığıyla çekirdek paylaşımı söz konusu. Oynatıcı ile arka plan
  aynı şarkıyı aynı anda analiz edebilir (ikisi de `INSERT OR REPLACE`, içerik aynı; yalnızca boşa iş).
- Etki: Zayıf makinede tarama + arka plan analizi sırasında arayüz akıcılığı düşebilir; veri bozulmaz.
- Önerilen düzeltme: Windows'ta `THREAD_PRIORITY_BELOW_NORMAL` + `THREAD_MODE_BACKGROUND_BEGIN`
  (`windows` kasası zaten bağımlılık), `pending_library_tracks`'ten çalan/sıradaki yolu dışlamak.

### [ANL-14] F-ölçüsü açgözlü eşleştirme; MIREX'in ilk 5 sn atlaması yok; `tap_offset` 100 ms kesmesi

- Önem: Düşük
- Güven: kesin
- Yer: analysis/evaluate.rs:121–153 (`f_measure`), :157–176 (`tap_offset`), :56 (`DROP_TOLERANCE = 1 s`)
- Kanıt: Her gerçek vuruşa "henüz kullanılmamış en yakın tahmin" sırayla atanıyor; `mir_eval` en büyük ikili
  eşleme kullanır — iki tahminin bir gerçek vuruşa eşit uzaklıkta olduğu durumlarda açgözlü yöntem bir isabet
  eksik sayabilir (küçük, kötümser sapma). Çift sayma yok (`used`). MIREX ilk 5 sn'yi değerlendirme dışı tutar;
  burada tutulmuyor (giriş sessizliğinde `trim_quiet_ends` zaten vuruş vermez, etkisi küçük). `tap_offset` ±100 ms
  dışını atar: parmak gecikmesi 100 ms'yi aşan kullanıcıda (yaşlı, dokunmatik) kayma 0 ölçülür ve
  `f_measure_aligned` düzelmez.
- Etki: Rapor edilen F birkaç yüzde kötümser olabilir; büyük parmak gecikmesinde "hizalı F" yanıltıcı.
- Önerilen düzeltme: Eşleştirmeyi `mir_eval` ile birebir (bipartite) yapmak ya da belgede "kötümser" demek;
  `tap_offset` penceresini 250 ms'ye çıkarıp ortanca sonrası ikinci geçiş.

### [ANL-15] İşaret dosyası adı: Windows ayrılmış aygıt adları (CON, AUX, NUL, COM1…) süzülmüyor

- Önem: Düşük
- Güven: yüksek (Windows kuralı; Linux'ta sınanamadı)
- Yer: analysis/annotation.rs:190–205 (`file_name`), :121–139 (`save`)
- Kanıt: Yalnızca denetim karakterleri ve `<>:"/\|?*` değiştiriliyor, uzunluk 80 karaktere kırpılıyor, sondaki
  nokta/boşluk temizleniyor. Etiketi olmayan `aux.mp3` adlı şarkı → etiket `aux.mp3` → dosya
  `aux.mp3.1a2b3c4d.json`; Windows ilk noktaya kadar olan adı aygıt sayar → `std::fs::write` başarısız → "İşaretler
  kaydedilemedi". Yol geçişi (`/`, `\`) ve özet çakışması doğru ele alınmış; `with_extension("json.tmp")` doğru.
- Etki: Çok nadir; işaretleme kaydı o şarkı için yapılamaz.
- Önerilen düzeltme: Ad, (büyük/küçük harf duyarsız) `CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9]` ile başlıyorsa başına
  `_` ekleyin; testi `dosya_adi_gecerli_ve_benzersiz`'e.

### [ANL-16] Yarıda kesilen çözme hiç önbelleğe yazılmıyor: bozuk kuyruklu dosya her açılışta baştan analiz

- Önem: Düşük
- Güven: kesin
- Yer: analysis/spectrogram.rs:142–146 (`saved`: `is_truncated` → `None`), :118–124; analysis/background.rs:225–231
  (`Failed` → yalnızca oturumluk `skip`)
- Kanıt: Tasarım gerekçesi (geçici ağ hatası kalıcı kısaltmasın) doğru; ama kalıcı bozuk dosyada (yaygın: kesik
  indirme, hatalı MP3 sonu) her açılış tam analiz (çözme + FFT, birkaç saniye CPU) ve her program başlangıcında
  arka plan bir kez daha dener.
- Etki: Gereksiz işlemci ve pil; görseller ilk saniyelerde eksik gelebilir (kareler hazır olana kadar).
- Önerilen düzeltme: Kesilmiş sonucu `truncated_at` damgasıyla saklayıp aynı damga ve aynı kesilme noktasında iki
  kez görülürse "kalıcı" sayarak kullanmak.

### [ANL-17] Görsel bant değerinin tanımı: aralıktaki en güçlü kutu + sabit eğim; bas 15 dB bastırılıyor

- Önem: Düşük
- Güven: kesin
- Yer: analysis/spectrogram.rs:40 (`TILT_DB_PER_OCTAVE = 3`), :678–681, :704–711
- Kanıt: Bant 0'ın merkezi ~33 Hz → eğim düzeltmesi 3·log2(0,033) = **−14,8 dB**; bant 31'de +11,6 dB. Üst
  bantlar 40–50 kutu geniş: gürültü benzeri içerikte N kutunun en büyüğü tek kutulu alt banda göre istatistiksel
  olarak ~5 dB yüksek çıkar; eğimle birlikte tizler görselde bas karşısında ~20 dB kayırılıyor. Yorum ("görsel
  denge") bunun bilinçli olduğunu söylüyor; ama `structure`/`meter` bas ölçümleri de aynı eğimli değerlerden
  (fark temelli oldukları için eşiklere etkisi sınırlı).
- Etki: Bas ağırlıklı türlerde spektrum çubukları solda cılız; tasarım tercihi olarak kabul edilebilir, belgede
  görünmüyor.
- Önerilen düzeltme: Bant gücünü ortalama güç (RMS) ile alıp eğimi 1,5 dB/oktava indirmek ya da en azından
  MIMARI.md'de belgelemek; değişiklik spektrum sürümünü artırır.

### [ANL-18] Tek kanala indirme (L+R)/2 ile FFT ve kroma: zıt fazlı stereo içerik kayboluyor

- Önem: Düşük
- Güven: kesin
- Yer: analysis/spectrogram.rs:467–468, :604
- Kanıt: Geniş stereo synth/gitar, Haas efektli vokal ve bazı MS kodlamalı kayıtlarda L ve R zıt fazlı bileşenler
  ortalamada sönümlenir; spektrum ve kroma o enstrümanı görmez. Seviyeler ve loudness (kanal başına) etkilenmez.
- Önerilen düzeltme: FFT'yi `max(|L|,|R|)` yerine L ve R güç spektrumlarının toplamından almak (iki FFT ya da
  gerçek/sanal paketleme ile tek FFT).

### [ANL-19] Belge/yorum ile kod uyumsuzlukları

- Önem: Düşük
- Güven: kesin
- Yer ve kanıt:
  - director/mod.rs:40–41: `TREBLE_FROM_BAND = 22` için "~3 kHz üstü" — bant 22'nin alt kenarı **2 249 Hz**.
  - visual_bridge/mod.rs:82–83 ve player.rs:258: `meter` "3 ya da 4" — aksak 5/8 (`[2,3]`) için **2**, 7/8 için
    3, 9/8 için 4; arayüz `meter` ile ölçü çizerse 5/8'de 2/4 gibi görünür.
  - analysis/beats.rs:43–44: 48 kHz için "0,0 ms" yazıyor, formül **1,1 ms**; 96 kHz "−5,6", formül −2,7 (doğru
    ±3 ms deniyor, tutuyor; ama tablo doğrunun değil ham ölçümün).
  - analysis/structure.rs:464: `peaks.dedup()` etkisiz (süzgeçten tekrar çıkmaz).
  - analysis/spectrogram.rs:216–217: `bass_peaks` belgesi `loudness` belgesiyle karışmış.
  - analysis/key.rs:125–127, 202–203: aynı Camelot sayısı için "paralel majör/minör" — Türkçe/Alman gelenekte
    doğru, İngilizce `KeyRelation::Relative/Parallel` adlarıyla ters okunuyor; bir cümlelik açıklama yeterli.
- Önerilen düzeltme: Yorumları düzeltmek; `BeatFrame.meter` için "ölçüdeki vuruş (grup) sayısı: 2–4".

### Şüpheler (kanıtlanamadı; güven: olası)

- **ANL-Ş1 — `dynamic_beats`'teki `value > 0.0` yeniden başlatma kuralı** (beats.rs:344–350) Ellis'te yoktur;
  uzun sessiz araları olan şarkıda zincir kopup yeniden başlar; `trim_quiet_ends` yalnızca uçları kırptığı için
  ortadaki boşlukta tek tük "hayalet" vuruş kalabilir. Sentetik testlerde ara sessizlik yok.
- **ANL-Ş2 — `choose_meter` 3/4 kararı** (structure.rs:379–386): `contrast3 > 1,5·contrast4 && > 0,5` eşiği 6/8
  ve 12/8'i 4/4'e, 3/4'ü de kick 1 ve 3'te olmadığı için 4/4'e kaydırabilir; vals kaydı testi yok.
- **ANL-Ş3 — `energy` eğrisinin %5–%95 normalizasyonu** (structure.rs:234–235) giriş/çıkış sessizliklerini de
  sıraladığı için uzun fade-out'lu şarkıda `low` −60 dB'ye iner ve bütün şarkı 0,7–1,0 bandına sıkışır;
  `after_energy ≥ 0,65` drop koşulu bu durumda neredeyse her ölçüde sağlanır.

## 4. Güçlü yanlar

- **Tek geçiş, çok ölçüm:** spektrum, seviyeler, başlangıç gücü, LUFS, bas tepeleri ve kroma aynı çözme
  döngüsünde (spectrogram.rs:463–494); ritim karelerden şarkı çözülmeden yenileniyor
  (`rhythm_from_frames`, test `eskiyen_ritim_sarki_cozulmeden_karelerden_ayni_hesaplanir`).
- **Parça sürümlü önbellek** (cache.rs:50–66): bir hesabın değişmesi yalnızca o parçayı eskitiyor; bozuk kayıt
  (`is_consistent`, inflate hatası) sessizce yok sayılıyor; `pending_library_tracks` ucuz yenilemeleri öne alıyor.
  Eski program sürümleriyle birlikte yaşama (`LEGACY_VERSION`) düşünülmüş.
- **Örnekleme hızına bağlı vuruş gecikmesi ölçülüp düşülmüş** (beats.rs:39–53) ve 8–96 kHz'te ortalama hata
  < 5 ms testle korunuyor; `frame_end` kesirli kare aralığında kaymayı önlüyor (test `her_ornekleme_hizinda…`).
- **Güven tek eşik, her yerde aynı:** `DROP_CONFIDENCE_MIN` arayüz sabitiyle testle eşitleniyor
  (structure.rs:922–939); ölçü güveni düşükse vurgu kapanıyor (`bars_confident`), ton güveni düşükse gösterilmiyor.
- **Parlama sınırı Yönetmen'de yapısal:** `thin` ile olaylar arası ≥ 0,34 s, güvenli modda ≥ 1 s; bu denetimin
  taraması birleşik kanalda bile 3/sn'yi aşmadığını gösterdi (ANL-08 yalnızca garantinin yazılı olmasını istiyor).
- **Krumhansl–Kessler profilleri ve Camelot eşlemesi yayınla birebir doğru** (24 ton için hesapla doğrulandı);
  akort kayması dairesel ortalamayla; ton oylaması ton değişen şarkıda "ortadaki komşu ton" hatasını önlüyor.
- **Yarıda kesilen çözme** çalma için kullanılıyor ama saklanmıyor; iptal her çözme parçasında denetleniyor;
  `from_saved` ile önbellekten kurulan spektrogramın tam analizle birebir aynı olduğu testle sabit.
- **İşaret dosyaları** ses içermiyor, atomik yazılıyor (`tmp` + `rename`), etiket değişse de özetle bulunuyor,
  yol geçişi karakterleri süzülüyor.
- **Foote çekirdeği** simetrik (sabit benzerlik için toplam 0; hesapla doğrulandı), boyutlar z-puanlanmış,
  en kısa bölüm ve eşik makul.

## 5. Denetlenemeyenler ve sınırlar

- Derleme ve test çalıştırma izni olmadığı için testlerin gerçekten geçtiği ve `eprintln!` ile yazılan senkron
  hataları (ms) görülmedi; bulgular kod okuması ve bağımsız hesaba dayanıyor.
- Gerçek müzikle hiçbir ölçüm yapılamadı (depoda ses yok, kural gereği); ANL-04, 05, 12 ve şüpheler bu yüzden
  "olası"/"yüksek" güvenle verildi.
- Windows'a özel davranış (ayrılmış dosya adları, iş parçacığı önceliği, 120 Hz ekran) Linux'ta sınanamadı.
- Sahnelerin parlaklık eğrileri (`sky.ts`, `highway.ts`, `stageLight.ts`) bu denetimin kapsamı dışında; Yönetmen'in
  garantisinin sahneye nasıl yansıdığı yalnızca girdi tüketimi düzeyinde incelendi (ANL-09).
- `audio::loudness`, `audio::bass` (BassPeakMeter) ve `storage.rs` göçleri yalnızca arayüzleri kadar okundu.

## 6. Önerilen öncelik sırası

1. **ANL-01** — hi-res dosyalarda FFT penceresini süreye bağlamak (spektrum sürümü artar; ANL-02 ve ANL-11'in
   192 kHz kısmını da düzeltir).
2. **ANL-03** — spektral kare okumasını pencere merkezine göre kaydırmak (küçük, ölçülebilir, ±20 ms bütçesini
   geri kazandırır).
3. **ANL-07** — gerçek müzikle çalışan yerel doğruluk betiği ve sentetik bozulma testleri (tempo değişimi, ızgara
   kayması, 192 kHz, birleşik parlama); diğer bulguların gerçek etkisini bu ölçüm gösterecek.
4. **ANL-04** — tempo değişimi: en azından "değişken tempo" bayrağı ve ortanca BPM; sonra yerel periyot.
5. **ANL-06** — önbellek kilidini Player kilidinden ayırmak (`join` ve `store`'u komut yolundan çıkarmak).
6. **ANL-05**, **ANL-12** — aksak ve drop tespitinde yerel/pencereli ölçütler.
7. **ANL-08**, **ANL-09** — Yönetmen garantisini birleşik kanal için teste ve belgeye yazmak.
8. **ANL-02**, **ANL-17**, **ANL-18** — spektrum bant tanımı (aynı spektrum sürümü artışında birlikte).
9. **ANL-10**, **ANL-11**, **ANL-13**, **ANL-14**, **ANL-15**, **ANL-16**, **ANL-19** — küçük düzeltmeler ve belge
   uyumu.
