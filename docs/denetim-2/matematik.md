# Matematik ve sinyal işleme — Bağımsız denetim raporu

Denetçi: bağımsız denetçi matematik/sinyal işleme. Kaynak: main @ 25f83f8. Tarih: 16 Ekim 2026.

## 1. Kapsam ve yöntem

İncelenen dosyalar (`(depo kökü)/src-tauri/src`, toplam ≈ 9 800 satır):

| Konu | Dosya                                                                                                        | Satır       |
| ---- | ------------------------------------------------------------------------------------------------------------ | ----------- |
| 1    | `audio/biquad.rs`, `audio/peq.rs`                                                                            | 222 + 757   |
| 2    | `audio/eq.rs`                                                                                                | 1 129       |
| 3    | `audio/loudness.rs`                                                                                          | 467         |
| 4    | `audio/normalize.rs`                                                                                         | 748         |
| 5    | `audio/limiter.rs`                                                                                           | 223         |
| 6    | `audio/resample.rs` (+ rubato 5.0.1 kaynağı: `synchro.rs`, `sinc.rs`, `windows.rs`)                          | 318         |
| 7    | `audio/bass.rs`, `audio/punch.rs`                                                                            | 1 051 + 347 |
| 8    | `analysis/beats.rs`, `analysis/evaluate.rs`                                                                  | 674 + 282   |
| 9    | `analysis/structure.rs`                                                                                      | 949         |
| 10   | `analysis/key.rs`                                                                                            | 800         |
| 11   | `director/mod.rs`                                                                                            | 516         |
| 12   | `visual_bridge/mod.rs` (+ `audio/output/wasapi.rs` 340–400, `src/hooks/useVisualFeed.ts`, `src/lib/sync.ts`) | 188         |
| 13   | `audio/abx.rs`                                                                                               | 1 015       |

Karşılaştırılan standart ve yayınlar: RBJ Audio EQ Cookbook; Equalizer APO/AutoEq `ParametricEQ.txt` biçimi;
Välimäki & Liski 2017 (Accurate Cascade Graphic Equalizer) ve Orfanidis tepe süzgeci; ITU‑R BS.1770‑4 (Tablo 1–2,
kapılama, Ek 2 gerçek tepe); EBU R128 / Tech 3341; Ellis 2007 (Beat Tracking by Dynamic Programming) ve librosa'nın
aynı algoritmaya dayalı uygulaması; MIREX/mir_eval F‑ölçüsü (±70 ms); Foote 2000 (yenilik eğrisi);
Krumhansl–Kessler 1982 ton profilleri; Camelot çarkı; binom sınaması.

Yöntem: her konuda önce standardın formülü yazıldı, sonra kod satır satır okundu ve fark **sayısal olarak**
gösterildi. Bütün hesaplar depo dışında, `python3 -I` ile (numpy 2.5.3; scipy yok) şu klasörde çalıştırıldı:
`docs/denetim-2/hesap/`
(`rbj.py`, `rbj2.py`, `camelot_binom.py`, `eq10.py`, `eq10b.py`, `peq_rate.py`, `loud.py`, `limiter.py`,
`resample.py`, `bass.py`, `beats_dir.py`). Her betik ilgili Rust fonksiyonunu bire bir Python'a taşır ve bağımsız bir
referansla (ör. doğrudan `|H(z)|`, `math.comb`, FFT) karşılaştırır. Depoda derleme/test çalıştırılmadı (izin yok);
testlerin toleransları okunarak değerlendirildi.

## 2. Genel değerlendirme

**Puan: 8/10.** Ses yolundaki matematik beklenenden iyi: RBJ katsayıları, BS.1770‑4 K‑süzgeci (48 kHz tablosuyla
1e‑16 düzeyinde aynı) ve kapılama, rubato yeniden örnekleyici (geçirme bandı ±0,0000 dB, durdurma −142 dB), Camelot
çarkı (24/24), Krumhansl–Kessler profilleri (24/24 sayı) ve binom p‑değeri (12/16 → 0,0384) hatasız. 10 bantlı
ekolayzerin etkileşim düzeltmesi gerçekten çalışıyor (düzeltmesiz +9,7 dB yerine +6,0 dB). Bulgular çoğunlukla
"tasarım sınırı ile belge/beklenti arasındaki fark" türünden: sınırlayıcı yalnızca örnek tepesine bakıyor (örnekler
arası +2,9 dBTP geçebiliyor), ekolayzerin uç bantları raf değil tepe olduğu için 20 Hz'de sürgünün yarısı
gerçekleşiyor, alt oktav üreticisinin seviyesi giriş frekansına göre ±5 dB oynuyor, kulaklık ön kazancı sabit 96 kHz
varsayımıyla hesaplanıyor.

En güçlü yan: standartlara dayalı her hesap (BS.1770, RBJ, Camelot, binom) birebir doğru ve testleri gerçek
standardın değerlerine bağlı. En zayıf yan: "sürgü = duyulan eğri" ve "0 dBFS aşılmaz" gibi ürün vaatlerinin
sınırları (bant kenarları, örnekler arası tepe) belgede açıkça yazmıyor. Bir numaralı risk: ekolayzer/bas yükseltmesi
sonrasında sınırlayıcıdan geçen sinyal örnek düzeyinde ≤ 1,0 kalsa da dönüştürücü çıkışında (DAC) 0 dBTP'yi
aşabilir; ucuz kulaklık çıkışlarında bu duyulabilir kırpılma demektir.

### Özet tablo

| #   | Konu                 | Standart/yayın → Kod → Fark                                                                                                                                                                                                                                                                              | Sonuç                                                                          |
| --- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| 1   | biquad/peq           | RBJ Cookbook → aynı formüller, TDF2, f64 → fark yok; fc'de G ve G/2 doğrulandı; kutuplar 10 Hz/Q 30/±30 dB'de bile birim çember içinde; f64 ile 20 Hz @192 kHz'te katsayı hatasının yanıta etkisi 4e‑12 dB                                                                                               | **doğru** (ön kazanç tahmini için bkz. MAT‑03, LS/HS varsayılan Q için MAT‑11) |
| 2   | eq.rs                | Välimäki–Liski 2017 (etkileşim matrisi, 2 geçiş) + Orfanidis tepe → aynı yapı, ayarlanmış sabitler (bant genişliği 1,0·w0, kenar 0,4·G, prototip 12 dB) → tasarım noktalarında en kötü 1,11 dB (belge "~1 dB", test 1,2 dB); aralarda 1,2 dB; 20 Hz'de +12 yerine +5,2 dB                                | **sapma var** (MAT‑02)                                                         |
| 3   | loudness.rs          | BS.1770‑4 → K‑süzgeci katsayıları 48 kHz'te tabloyla 2e‑16 içinde aynı; diğer hızlarda ±0,03 dB; 400 ms/100 ms blok, −70 LUFS mutlak, −10 LU göreli kapı; 1 kHz −23 dBFS → −22,993 LUFS; gerçek tepe 4× Blackman‑sinc en kötü −0,17 dB                                                                   | **doğru** (mono ağırlığı 2 = bilinçli +3 dB sapma, MAT‑07)                     |
| 4   | normalize.rs         | Hedef −14 LUFS, yükseltmede −1 dBTP tavanı, boşluk = −(tepe+kazanç) → formüller tutarlı; koruma = yükseltme − boşluk                                                                                                                                                                                     | **doğru**                                                                      |
| 5   | limiter.rs           | İleriye bakan tepe sınırlayıcı → 1,5 ms bakış, kayan en küçük + anlık iniş/80 ms bırakma + kayan ortalama → örnek tepesi ≤ 1,0 **ispatlandı** (sim. 1,000000000); kazanç adımı ≤ 0,5/72; ama örnekler arası tepe ölçülmüyor: fs/4 sinüs +2,92 dBTP dokunulmadan geçiyor                                  | **sapma var** (MAT‑01)                                                         |
| 6   | resample.rs          | rubato Fft, BlackmanHarris², FixedSync::Input, 2048 → 44,1→48: sinc 2058, kesme 21 903 Hz, geçiş 200 Hz, geçirme bandı ±0,0000 dB, görüntü/alias ≤ −142 dB; gecikme (1120 kare) baştan atılıyor; aynı dönüştürücü boşluksuz devam ediyor                                                                 | **doğru**                                                                      |
| 7   | bass.rs/punch.rs     | RBJ raf 100 Hz Q 0,707; Chebyshev T2–T4 (hata 1e‑15); oktav bölücü = kare dalga × zarf + 4. derece 60 Hz LP → alt oktav seviyesi girişe göre −2,6…−12,1 dB (40–120 Hz), 40 Hz girişte 3. harmonik −13 dB; iki tonda ikinci nota kayboluyor; vuruş: düz basta 0,00 dB sızıntı, ±3 dB tremoloda 1,4 dB     | **sapma var** (MAT‑04; vuruş doğru)                                            |
| 8   | beats.rs/evaluate.rs | Ellis 2007: spektral akı, ACF × log‑Gauss önsel (σ 1,4 okt), TPS2, DP (log‑kare ceza) → kodda σ = 1,0, önsel 2τ terimlerine W(τ) ile uygulanıyor, α = 100 (librosa), yerel puan σ = T/32 (librosa) → küçük, bilinçli sapmalar; F‑ölçüsü ±70 ms doğru ama "en yakın" açgözlü eşleme en büyük eşleme değil | **sapma var** (MAT‑05, MAT‑06)                                                 |
| 9   | structure.rs         | Foote 2000 dama tahtası, Gauss eğim (σ = w/2) → aynı; benzerlik = −Öklid (z‑puanlı), kosinüs değil (geçerli); drop eşiği 0,12 bant ölçeği = 7,2 dB (ölçek dB‑doğrusal, 60 dB/1,0 doğrulandı)                                                                                                             | **doğru**                                                                      |
| 10  | key.rs               | KK‑1982 majör/minör profilleri sayı sayı aynı; Pearson ilinti; Camelot 24/24 doğru; akort = dairesel ortalama, kroma = Hann 16384 + parabol tepe + cos² katlama                                                                                                                                          | **doğru**                                                                      |
| 11  | director             | thin(0,34 s) → herhangi 1 s penceresinde ≤ 3 nabız (60–240 BPM, ölçü 3/4/5/7 taraması: en kötü 3); sönme e^(−t/τ), smoothstep sürekli; warmth drop anında 0,15 sıçrıyor                                                                                                                                  | **doğru** (MAT‑10 küçük)                                                       |
| 12  | visual_bridge        | görsel zaman = duyulan + 1/60 − ses gecikmesi; +200 ms gecikme → görseller 200 ms geriden: işaret doğru; duyulan kare = tüketilen − aygıt dolgusu − sınırlayıcı gecikmesi: doğru; ekran gecikmesi sabit 1/60 s                                                                                           | **doğru** (MAT‑09 küçük)                                                       |
| 13  | abx.rs               | FFT çapraz ilinti + normalize benzerlik; LUFS farkıyla yalnız kısma; binom kuyruğu `math.comb` ile 1e‑31 içinde aynı, 12/16 → 0,038406                                                                                                                                                                   | **doğru**                                                                      |

## 3. Bulgular (önem sırasıyla)

### [MAT-01] Sınırlayıcı örnek tepesine bakıyor; örnekler arası tepe (dBTP) 0 dB'yi aşabiliyor

- Önem: Orta
- Güven: kesin (simülasyonla kanıtlandı)
- Yer: `audio/limiter.rs:80-81`, `:17-20`
- Kanıt: `let peak = frame.iter().fold(0.0, |m, &x| m.max(x.abs())); let wanted = if peak > CEILING { CEILING / peak } else { 1.0 };`
  — kazanç yalnızca örneğin mutlak değerine göre. `python3 -I hesap/limiter.py`:
  ```
  düşmanca sinyal: giriş maks 5.33, çıkış maks 1.000000000 (≤1.0 → örnek tepe ispatı tutuyor)
  tek darbe 2.0: min kazanç 0.5000, en büyük kazanç adımı 0.00694 (= 0.5/72)
  fs/4 sinüs, örnek tepe 0.990 (−0.09 dBFS), gerçek tepe +2.92 dBTP → sınırlayıcı kazancı 1.000; çıkış gerçek tepe +2.92 dBTP
  kırpılmış 9 kHz: giriş gerçek tepe +1.74 dBTP; çıkış örnek tepe 1.000, gerçek tepe +1.74 dBTP
  ```
  Örnek tepesi için ispat tutuyor: çıkış t anında x(t−d) ile çarpılan kazanç, held(τ) ≤ min(τ) ≤ w(t−d) (τ ∈ [t−d, t])
  değerlerinin ortalaması olduğundan ≤ w(t−d), dolayısıyla |çıkış| ≤ 1. Ancak ispat yalnızca örnekler içindir.
- Etki: Ekolayzer/bas yükseltmesi sonrası sinyal örnek düzeyinde 1,0'ı aşmazken DAC'ın yeniden oluşturduğu dalga
  +1…+3 dBTP'ye çıkabilir; yüksek frekanslı, sert sınırlanmış kayıtlarda analog kırpılma/cızırtı. Modülün başlığı
  "0 dBFS'i aşmama" vaadini örnek tepesi anlamında tutuyor; EBU R128'in −1 dBTP tavanı (loudness.rs'te uygulanan) bu
  katmanda yok.
- Yeniden üretme / önerilen test: `limiter.rs` testlerine fs/4'te 45° kaydırılmış, genliği 1,4 olan sinüs ekle;
  çıkışı 8× ara değerleyip tepesinin ≤ 1,0 (ya da ≤ −1 dBTP) olduğunu iddia et — mevcut kodla başarısız olur.
- Önerilen düzeltme: `wanted` hesabında örnek tepesi yerine 4× (ya da en az 2×) ara değerlenmiş tepeyi kullan
  (`loudness.rs`'teki 12 katsayılı çok fazlı sinc, 72 karelik bakış penceresi içinde bellek ayırmadan uygulanabilir)
  ve tavanı −1 dBTP'ye çek; ya da belgede sınırın örnek tepesi olduğunu açıkça yaz.

### [MAT-02] Ekolayzer: uç bantlar raf değil tepe; 20 Hz'de sürgünün ~yarısı; en kötü sapma belgedeki "~1 dB"in üstünde

- Önem: Orta
- Güven: kesin (tasarım Python'a bire bir taşındı)
- Yer: `audio/eq.rs:15-19` (belge), `:359-375` (Orfanidis tepe), `:433-434` (bant genişliği), `:812` (test toleransı 1,2 dB)
- Kanıt: `python3 -I hesap/eq10.py`, `hesap/eq10b.py`:
  ```
  Etkileşim düzeltmesiz kaskad, hepsi +6: 1 kHz'te +9.66 dB (düzeltmeli ~+6.0)       ← yöntem çalışıyor
  hepsi +12 @48k: 20 Hz'te +5.22 dB, 25 Hz +8.82, 20 kHz +7.65 dB, 1 kHz +12.77 dB
  zikzak ±12 @48k: tasarım noktası maks 1.11 dB | ızgara maks +1.14 dB @ 7652 Hz
  komşu zıt (+12,−12) @48k: tasarım noktası 0.72 dB | ızgara (ara frekans) +1.21 dB @ 540 Hz
  tek bant 1 kHz +12: tasarım noktası 0.51 dB | ızgara +1.02 dB @ 1075 Hz
  300 rastgele ayarda tasarım noktası en kötü sapma: 44.1k 0.87, 48k 0.84, 96k 0.82 dB
  ```
  Belge: "tasarım noktalarında en fazla ~1 dB"; test `assert!(error < 1.2)`. Ölçülen en kötü 1,11 dB (zikzak, 48 kHz):
  test geçer, belge iddiası tam değil. Välimäki–Liski'den farklı olarak bant genişliği 1,0·w0 (yazıda ≈ oktav
  bandı 0,707·w0), kenar oranı 0,4 (yazıda gB = √G, yani 0,5) ve prototip 12 dB (yazıda 17 dB) — kodun kendi
  ayar taramasıyla seçilmiş; bu bir hata değil, ama yayının sayıları değil.
- Etki: Kullanıcı 31 Hz'i +12 yaptığında alt bas (20–25 Hz) yalnızca +5…+9 dB yükselir; "sürgü = duyulan eğri"
  vaadi bant ortalarının dışında geçerli değil. Aralardaki ±1,2 dB sapma kulakla fark sınırında (müzikte ~1 dB).
- Yeniden üretme / önerilen test: `max_error` ölçümünü 20 Hz ve 20 kHz'te de yap; "hepsi +12"de 20 Hz'in ≥ +10 dB
  olduğunu iddia et (mevcut: +5,2).
- Önerilen düzeltme: En alt ve en üst bandı raf (low/high shelf) süzgeciyle tasarla (standart grafik ekolayzer
  uygulaması; Välimäki–Liski'nin sonraki sürümleri de kenarlarda raf kullanır) ya da belgeye "eğri 31 Hz–16 kHz
  arasında tanımlıdır" notunu ve "~1 dB" yerine ölçülen 1,2 dB'yi yaz.

### [MAT-03] Kulaklık düzeltmesinin ön kazanç sınırı sabit 96 kHz'te ve yalnızca 20 Hz–20 kHz'te hesaplanıyor

- Önem: Orta
- Güven: kesin (hesapla)
- Yer: `audio/peq.rs:236-240` (`sanitized`: `preamp_db.min(-peak)`), `:275-287` (`response_peak_db`: `let rate = 96_000.0`, 20 Hz–20 kHz tarama), `:112-116` (`HeadphoneState::new` eğri de 96 kHz)
- Kanıt: `python3 -I hesap/peq_rate.py`:
  ```
  HSC 10k +6 Q0.7   : tepe 96k +5.74 dB (ön kazanç buna göre), 48k +5.98, 44.1k +6.00 → 44.1k'da fark +0.25 dB
  hepsi birlikte    : tepe 96k +18.91 dB, 48k +15.95, 44.1k +15.13 → 44.1k'da fark −3.78 dB
  ```
  RBJ süzgeçleri çift doğrusal dönüşümle örnekleme hızına bağlı olarak bükülür; Nyquist'e yakın filtrelerin yanıtı
  hıza göre değişir. 44,1 kHz'te gerçek tepe tahminden 0,25 dB fazla (küçük taşma payı, sınırlayıcı yakalar); yüksek
  frekans filtreleri birikince tahmin 3,8 dB fazla (gereksiz kısma). Arayüzde çizilen eğri de 96 kHz'e göre:
  44,1 kHz'te çalan sesin eğrisi değil.
- Etki: Bazı AutoEq profillerinde ses gereğinden birkaç dB kısık; bazılarında ön kazanç 0,2–0,3 dB yetersiz.
- Yeniden üretme / önerilen test: `on_kazanc_tasmayi_onler` testini 44,1 kHz'te HSC 10 kHz +6 ile çalıştırıp
  `response_db(&[f], 44_100.0)` tepesinin `−preamp_db`'yi aşmadığını iddia et.
- Önerilen düzeltme: `response_peak_db`'yi `PeqProcessor::new`'de gerçek örnekleme hızıyla hesapla ve
  `bank.gain`'e oradan uygula; arayüz eğrisini de çalan hızla çiz (hız bilinmiyorsa 48 kHz daha temsilî).

### [MAT-04] Alt oktav üreticisinin seviyesi ve harmonik bozulması giriş frekansına göre çok değişiyor

- Önem: Orta
- Güven: kesin (simülasyon)
- Yer: `audio/bass.rs:75-80` (`OCTAVE_SMOOTH_HZ = 60`, `OCTAVE_DC_HZ = 18`, `OCTAVE_GAIN = 0.8`), `:371-392` (`Octaver::process`)
- Kanıt: `python3 -I hesap/bass.py` (giriş −6 dBFS sinüs, derinlik %100):
  ```
  giriş  40 Hz: alt oktav 20 Hz, girişe göre −5.5 dB; 3. harm −13.1 dB (alt oktava göre)
  giriş  55 Hz: alt oktav 27.5 Hz, girişe göre −2.6 dB; 3. harm −21.5 dB
  giriş  80 Hz: alt oktav 40 Hz, girişe göre −3.6 dB; 3. harm −32.3 dB
  giriş 100 Hz: alt oktav 50 Hz, girişe göre −7.0 dB
  giriş 120 Hz: alt oktav 60 Hz, girişe göre −12.1 dB
  İki tonlu bas (55 + 82.4 Hz): 27.5 Hz (−7 dBFS), 55.0 Hz (−21), 82.5 Hz (−34) → yalnızca 55 Hz'in oktavı üretiliyor
  ```
  Neden: kare dalganın temel bileşeni 4/π·zarf, ama 4. dereceden 60 Hz süzgeç tam 60 Hz'lik alt oktavı −6 dB,
  18 Hz süzgeç 20 Hz'i ~−2 dB keser; 20 Hz'lik alt oktavın 3. harmoniği (60 Hz) ise süzgeçten geçer. Tek "sign"
  değişkeni olduğu için iki eş zamanlı bas notasından yalnız biri izlenir (klasik oktav bölücü sınırı).
- Etki: "Derinlik" düğmesi 40–60 Hz'lik basta güçlü, 100–120 Hz'lik basta neredeyse yok (10 dB fark); çok alçak
  basta kare dalga harmonikleri duyulur. Belgedeki "0,8 × zarf" sabit genlik beklentisi gerçekleşmiyor.
- Yeniden üretme / önerilen test: 55 ve 110 Hz girişlerde alt oktav genliğini ölç, farkın ≤ 3 dB olduğunu iddia et
  (mevcut: ~10 dB).
- Önerilen düzeltme: Yumuşatma süzgecinin köşesini kaynak bandının yarısının üstüne (≈ 80–90 Hz) alıp sinüs
  benzeri çıktı için kare dalga yerine `sign·sin(φ/2)` tipi faz izleyici kullanmak ya da çıkışı giriş frekansına
  göre telafi etmek; en azından `OCTAVE_GAIN` belgesini "frekansa bağlı" diye düzeltmek.

### [MAT-05] Beat takibi Ellis 2007'den üç noktada ayrılıyor (bilinçli ama belgelenmemiş)

- Önem: Düşük
- Güven: kesin (formül karşılaştırması + sayısal)
- Yer: `analysis/beats.rs:30-33` (`TEMPO_PRIOR_OCTAVES = 1.0`, `TIGHTNESS = 100.0`), `:349-352` (önsel ve TPS2), `:410-416` (DP)
- Kanıt: Ellis 2007: W(τ) = exp(−½(log₂(τ/τ₀)/σ)²), σ = 1,4 oktav; TPS2(τ) = TPS(τ) + ½TPS(2τ) + ¼TPS(2τ±1) ve
  TPS zaten W ile ağırlıklı (yani 2τ terimi W(2τ) ile). Kod: `prior * (acf[lag] + 0.5*acf[2*lag] + …)` — tek W(τ).
  `python3 -I hesap/beats_dir.py`:
  ```
   60 BPM: kod 0.607  Ellis 0.775  oran 0.78
  200 BPM: kod 0.762  Ellis 0.871  oran 0.88
  τ=80 BPM: W(τ)=0.843 vs W(2τ)=W(40 BPM)=0.285   ← kodda yarım tempo terimi 3× daha ağır
  ```
  DP: ceza `TIGHTNESS·ln(gap/period)²` Ellis'in F = −(log Δt/τ)² biçimiyle aynı; α = 100 librosa'nın varsayılanı
  (Ellis'in metnindeki değer değil; onset normalize edildiği için librosa ile tutarlı). Yerel puan Gauss σ = T/32,
  librosa ile aynı. Geri izleme: son 2T içindeki en yüksek puan (librosa: ortanca eşikli yerel tepe).
- Etki: Uç tempolar (60, 200 BPM) ortaya göre Ellis'ten daha çok bastırılır; 2τ terimine yüksek ağırlık yavaş
  şarkılarda çift tempo seçimini kolaylaştırır. Testler sentetik; gerçek etki ancak DOGRULAMA ile görülür.
- Yeniden üretme / önerilen test: 65 BPM ve 195 BPM sentetik davul kayıtlarında (mevcut `drum_track`) tempo
  hatasını ölç; σ = 1,4 ile karşılaştır.
- Önerilen düzeltme: Yayından ayrılan sabitleri modül belgesinde "neden" ile yaz; `score` içinde 2τ terimlerine
  kendi önsellerini uygula (`prior(2*lag)`).

### [MAT-06] F‑ölçüsünde "en yakın" açgözlü eşleme, en büyük eşleme değil

- Önem: Düşük
- Güven: kesin (karşı örnek)
- Yer: `analysis/evaluate.rs:121-152`
- Kanıt: mir_eval/MIREX en büyük iki parçalı eşleme kullanır. Kod her gerçek vuruşa kullanılmamış en yakın tahmini
  verir. `hesap/beats_dir.py`:
  ```
  est=[0.95, 1.04] truth=[1.00, 1.05]: kod F=0.500, optimum F=1.000
  est=[0.96, 1.03, 1.10] truth=[1.00, 1.06, 1.12]: kod F=0.667, optimum F=1.000
  ```
  Sıralı listelerde "soldan sağa, tolerans içindeki ilk kullanılmamış tahmin" eşlemesi optimumdur ve aynı
  karmaşıklıktadır.
- Etki: Vuruşlar 70 ms'den çok daha seyrek olduğu için pratikte nadiren fark eder (çift tempo + kayma
  birleşiminde olabilir); ölçüt MIREX'ten küçük sapabilir, hedef "F ≥ 0,80" yorumunu etkilemez.
- Önerilen düzeltme: İki işaretçili sıralı eşleme (yukarıdaki) ile değiştir; karşı örneği test olarak ekle.

### [MAT-07] Tek kanallı dosyalarda BS.1770'ten bilinçli +3 dB sapma

- Önem: Düşük
- Güven: kesin
- Yer: `audio/loudness.rs:226-235` (`(1, _) => 2.0`)
- Kanıt: `hesap/loud.py`: `mono 1 kHz −23 dBFS (kodun ağırlığı 2) → −22.9933 LUFS; BS.1770 tek kanal → −26.0036`.
  Standartta tek kanalın ağırlığı 1'dir; kod "mono iki hoparlörden çalınır" gerekçesiyle 2 kullanır. Oynatıcı monoyu
  gerçekten iki kanala çoğalttığı (`resample.rs::output_channels`) için eşitleme amacı açısından **doğru** karar;
  ama ölçüm ffmpeg/libebur128 ile 3 dB farklı çıkar.
- Etki: Kullanıcı başka bir araçla karşılaştırırsa mono dosyalarda 3 dB fark görür. Belgeye yazılmalı.
- Önerilen düzeltme: Yok (karar doğru); `Loudness` belgesine ve varsa kullanıcı yardımına not.

### [MAT-08] Gerçek tepe ara değerleyicisi 18–20 kHz'de 0,17 dB'ye kadar düşük okuyor

- Önem: Düşük
- Güven: kesin (hesapla)
- Yer: `audio/loudness.rs:133-176` (12 katsayı/faz, Blackman, faz başına DC normalizasyonu)
- Kanıt: `hesap/loud.py`:
  ```
  f(kHz) faz0   faz1   faz2   faz3        tam ölçekli sinüs en kötü ölçüm
    15  -0.012 -0.072 -0.072 -0.012        15 kHz: -0.030 dBTP
    18  -0.131 -0.825 -0.825 -0.131        18 kHz: -0.168 dBTP
    20  -0.332 -2.431 -2.431 -0.332        20 kHz: -0.133 dBTP
  ```
  1 kHz'te −0,000 dB. BS.1770‑4 Ek 2'nin 4× süzgeci için beklenen doğruluk sınıfı içinde; `max_gain` kısa yolu
  (|Σcᵢxᵢ| ≤ max|x|·Σ|cᵢ|) matematiksel olarak doğru.
- Etki: Çok parlak kayıtlarda −1 dBTP tavanı fiilen −0,83 dBTP; sınırlayıcı (MAT‑01 düzeltilirse) bunu yakalar.
- Önerilen düzeltme: Gerekirse faz başına 16 katsayı ya da Kaiser penceresi (β ≈ 8) ile geçirme bandını 20 kHz'e
  kadar ±0,05 dB'ye düzeltmek; şu an için kabul edilebilir.

### [MAT-09] Ekran gecikmesi sabit 1/60 s; 144 Hz ve çift tamponlu 60 Hz'de senkron hedefinin sınırında

- Önem: Düşük
- Güven: yüksek
- Yer: `visual_bridge/mod.rs:26` (`DISPLAY_LEAD_SECONDS = 1.0 / 60.0`), `:38-41`
- Kanıt: `visual_time = heard + 1/60 − delay`. İşaret doğrulandı: aygıt sesi 200 ms geç çalıyorsa (ayar +200)
  görseller 200 ms geriden okunur (test `gorsel_zamani_...`). Duyulan kare: `consumed − padding − limiter.latency`
  (`wasapi.rs:360-363`) — sınırlayıcı gecikmesi doğru yönde düşülüyor. Arayüz IPC gecikmesini `age` ile telafi ediyor
  (`useVisualFeed.ts:55-56`, `sync.ts::extrapolateFrame`); kompozitör/tarama gecikmesi ise sabit 16,7 ms varsayımı.
  144 Hz ekranda gerçek değer ≈ 7 ms (görseller ~10 ms erken), 60 Hz'de üçlü tamponlamada ≈ 33 ms (~17 ms geç).
- Etki: ±20 ms hedefinin içinde ama bütçenin tamamını bu varsayım yiyebilir.
- Önerilen düzeltme: rAF zaman damgaları arasındaki ortanca aralığı ölçüp (ekran yenileme süresi) öncelik olarak
  onu kullanmak; kalibrasyon ekranında kullanıcıya hem ses hem görüntü gecikmesini ölçtürmek.

### [MAT-10] Yönetmen: "warmth" drop anında 0,15 sıçrıyor; nabız garantisi akışlar ayrı seyreltildiği için yalnızca taramayla doğrulanabiliyor

- Önem: Düşük
- Güven: kesin
- Yer: `director/mod.rs:148-157` (`thin` ayrı ayrı), `:233` (`warmth: 0.15 + 0.7*mood + 0.15*release`), `:324-331` (`decay`)
- Kanıt: `release` drop anında 0 → 1 (üstel sönme, 0'da 1) olduğundan `warmth` 0,15 sıçrar; belge "nabız dışındaki
  bütün değerler yumuşak değişir" der. Nabız: `thin(beats, 0.34)` tek akışta herhangi 1 s penceresinde en çok 3
  olay verir (0, 0,34, 0,68; dördüncüsü 1,02 s'de) — ispat. Ama `accents = thin(downbeats, 0.34)` ayrı seyreltilir:
  seyreltmede atılan bir vuruş ölçü başıysa vurgu nabızla çakışmaz. Tarama (`hesap/beats_dir.py`): 60–240 BPM,
  ölçü 3/4/5/7 → birleşik ayrı olay en kötü 3/s (122 BPM, 3/4). Garanti tutuyor, ancak genel ispat değil, tarama.
- Etki: Görsel; epilepsi sınırı (3/s) aşılmıyor. Drop'ta sıcaklık sıçraması tasarım gereği kabul edilebilir ama
  belgeyle çelişiyor.
- Önerilen düzeltme: `accents`'ı `pulses` kümesinin alt kümesi olarak seç (`thin` sonrası kesişim) → birleşik
  ≤ 3/s doğrudan ispatlanır; `warmth` için `release` yerine onun kısa smoothstep'li sürümünü kullan.

### [MAT-11] Equalizer APO "LS/HS" (Q yazılmayan raf) için varsayılan Q = 0,707 varsayımı

- Önem: Düşük
- Güven: olası (Equalizer APO belgesi varsayılanı açıkça yazmıyor; kaynak kodu doğrulanamadı)
- Yer: `audio/peq.rs:200-205`
- Kanıt: Equalizer APO yapılandırma referansı: "LS 6dB / LS 12dB: 6 / 12 dB per octave with corner freq.", "LSC: with
  center freq., x dB per oct." — APO raf eğimini dB/oktav (RBJ'nin S parametresi) ile tanımlar; `LSC … x dB` biçimi
  (örn. "ON LSC 10.8 dB Fc 300 Hz") kodda ayrıştırılmıyor (`value("q")` yoksa 0,707). RBJ'de S = 0,9 (≈12 dB/okt)
  için 1/Q = √((A+1/A)(1/S−1)+2) → Q ≈ 0,64–0,67 (A'ya göre).
- Etki: AutoEq her zaman `LSC … Q` yazdığı için AutoEq dosyalarında etkisi yok; elle yazılmış APO dosyalarında raf
  geçişi ≤ 0,5 dB farklı, "LSC 10.8 dB Fc …" biçimi ise sessizce 0,707 ile okunur.
- Önerilen düzeltme: "`LSC <x> dB Fc`" biçimini ayrıştırıp S'ye, oradan Q'ya çevirmek; belgede varsayılanı yazmak.

### Şüpheler (kanıtlanamadı, güven "olası")

- **Vuruş (punch) gerçek basta sızıntı:** sentetik bas gitar notalarında (5 ms atak) en çok 0,7 dB, ±3 dB
  tremoloda 1,4 dB raf yükselmesi ölçüldü (`hesap/bass.py`); düz sinüslerde 0,00 dB. "Sürekli bas aynen kalır"
  iddiası düz/az dalgalı bas için doğru; canlı çalınmış basta ±1 dB oynama olası. Düzeltme gerekmeyebilir; dinleme
  testi önerilir.
- **`response_db`'nin kosinüs biçimi derin zayıflatmada hassasiyet kaybediyor** (`biquad.rs:134-144`): HP 120 Hz'in
  10 Hz'teki −43 dB değerinde `|H(z)|`'den 1,3e‑3 dB sapma (`hesap/rbj2.py`). Tasarımda yalnızca kazanç bölgeleri
  kullanıldığından etkisi yok.

## 4. Güçlü yanlar (kanıtla)

- **RBJ katsayıları birebir** (`hesap/rbj.py`): tepe fc'de tam G (6,0000 dB), bant kenarları G/2 (±0,01 dB), raflar
  fc'de G/2 ve uzakta G/0, Butterworth −3,010 dB ve 10 fc'de −40,0 dB. Kutuplar 10 Hz / Q 30 / ±30 dB'de 44,1 ve
  192 kHz'te birim çember içinde (en kötü |p| = 0,999999030). f64 seçimi gerekli ve yeterli: 20 Hz @192 kHz'te
  1−a₂ = 6,6e‑4, f64 bağıl hata 3e‑13 → yanıt hatası 4e‑12 dB; f32 olsaydı 1e‑7 bağıl hata 1,7 dB hata verirdi
  (kod f32'ye geçirilmemeli). TDF2 (`process`) ve denormal temizliği doğru.
- **BS.1770‑4 K‑süzgeci** 48 kHz tablosuyla ≤ 9e‑16 farkla aynı; libebur128'in analog ön örneğinden türetim (f₀ =
  1681,97 Hz, G = 3,9998 dB, Q = 0,70718; RLB f₀ = 38,135 Hz, Q = 0,50033) diğer hızlarda ±0,03 dB içinde. Pay
  `[1,−2,1]`'in normalize edilmemesi standartla aynı (+0,031 dB, −0,691 sabitine gömülü). Kapılama sırası (mutlak →
  göreli −10 LU → ortalama) ve `>` karşılaştırmaları standartla aynı; EBU 3341 durum 1–5 testleri gerçek
  referanslarla ve standardın ±0,1 LU toleransıyla. Simülasyon: −23 dBFS → −22,993 LUFS.
- **Yeniden örnekleyici** (`hesap/resample.py`, rubato 5.0.1 kaynağından): 44,1→48 kHz'te 2058 noktalı
  Blackman‑Harris² sinc, kesme 21 903 Hz, 20 Hz–20 kHz dalgalanma ±0,0000 dB, −0,1 dB noktası 21 830 Hz, −100 dB
  22 030 Hz, görüntü/alias ≤ −142 dB (test −100/−140 dB ister; gerçek 23 kHz tonu −226 dB). Gecikme
  (`output_delay = fft_size_out/2`) baştan atılıyor; `continues()` ile boşluksuz geçişte süzgeç durumu korunuyor.
- **Ekolayzer etkileşim düzeltmesi gerçek**: düzeltmesiz kaskad "hepsi +6"da 1 kHz'te +9,66 dB verirdi; kod +5,96 dB.
  300 rastgele ayarda tasarım noktası sapması ≤ 0,87 dB; Gauss eleme kısmi pivotlu, 10×10 normal denklemler
  gerçek zamanlı yolda bellek ayırmadan.
- **Sınırlayıcının örnek tepesi ispatı** tutuyor (yukarıda); kazanç eğrisi sürekli (en büyük adım 0,5/72 = 0,0069).
- **Camelot 24/24, Krumhansl–Kessler 1982 profilleri 24/24 sayı aynı, Pearson ilinti doğru, binom kuyruğu
  `math.comb` ile 1e‑31 içinde** (`hesap/camelot_binom.py`): 12/16 → 0,038406.
- **Chebyshev harmonikleri** T₂–T₄ formülleri 1e‑15 içinde; drop eşiği "0,1 ≈ 6 dB" iddiası doğru (spektrogram
  bant ölçeği −66…−6 dB üzerinde doğrusal, `spectrogram.rs:719`).
- **Gecikme telafisinin yönü** her katmanda doğru: duyulan kare = tüketilen − aygıt dolgusu − sınırlayıcı gecikmesi;
  görsel zaman = duyulan + ekran − ses aygıtı; onset gecikmesi vuruşlardan çıkarılıyor (`beats.rs:197`).

## 5. Denetlenemeyenler ve sınırlar

- Derleme/test izni olmadığı için Rust kodu çalıştırılmadı; her hesap Python'a bire bir taşıma ile yapıldı.
  Taşımada yorum farkı riski küçük ama sıfır değil (özellikle `eq.rs` tasarımı; sonuçlar kodun test toleransları ve
  belgesiyle tutarlı çıktığı için taşıma doğru kabul edildi).
- Gerçek müzikle beat/ton/yapı doğruluğu (F‑ölçüsü, ton isabeti) ölçülmedi; yalnızca formüller karşılaştırıldı.
- Equalizer APO'nun kaynak kodu okunmadı (MAT‑11 "olası").
- Välimäki–Liski 2017'nin sabitleri (bant genişliği, kenar oranı, prototip kazancı) belleğe dayanılarak
  karşılaştırıldı; kodun farklı sabitleri "sapma" değil, ayar tercihi olarak değerlendirildi.
- Spektrogram/onset gecikme modeli (`ONSET_LATENCY_PER_WINDOW`) deneysel ölçüme dayanıyor; yeniden ölçülmedi.
- LRA (EBU Tech 3342) kodda yok; kapsam dışı sayıldı.

## 6. Önerilen öncelik sırası

1. **MAT‑01** — sınırlayıcıya örnekler arası tepe (en az 2–4× ara değerleme) ve −1 dBTP tavanı; ya da belgeyi düzelt.
2. **MAT‑03** — kulaklık ön kazancını ve eğriyi gerçek örnekleme hızıyla hesapla (küçük değişiklik, ölçülebilir kazanç).
3. **MAT‑02** — uç bantları raf yap ya da belgeyi/arayüzü "31 Hz–16 kHz arasında tanımlı, ±1,2 dB" diye düzelt.
4. **MAT‑04** — alt oktav seviyesini frekanstan bağımsızlaştır (yumuşatma köşesi/telafi).
5. **MAT‑06** — F‑ölçüsünde sıralı en büyük eşleme (5 satır).
6. **MAT‑05, MAT‑10, MAT‑09, MAT‑11, MAT‑07, MAT‑08** — belge notları ve küçük iyileştirmeler.
