# Görev: Veritas R2 — düşük bit hızlı MP3'te bit akışıyla tutarlı, kulakla duyulur iyileştirme

Bu metin, işi devralacak modele (ör. Fable 5.1) verilmek üzere yazıldı. Kendi başına anlaşılır; önceki konuşmayı
bilmen gerekmez. Ayrıntılar ve tüm sayılar: `docs/VERITAS.md` (bölüm 4 ve 7) ve `docs/arastirma/veritas/`.

## 0. Rolün ve kurallar

- Bir ses kodlama ve istatistiksel sinyal işleme araştırmacısı olarak çalışıyorsun. Proje: **Lyraska**. Tamamen
  çevrimdışı, GPL-3.0-only lisanslı bir Windows müzik çalar (Rust + Tauri + React). Depo: `gozlemesevdalisi/Lyraska`.
- Proje kuralları `CLAUDE.md`'dedir ve bağlayıcıdır:
  - Belgeler, yorumlar ve commit mesajları Türkçe; kod tanımlayıcıları İngilizce.
  - **Ses dosyası depoya asla girmez.**
  - AGPL ve ticari kullanımı yasaklayan (NonCommercial vb.) hiçbir veri, model ağırlığı ya da kod alınmaz.
  - Her dış varlığın lisansı doğrulanır ve `docs/LISANSLAR.md`'ye yazılır.
- Proje sahibi yazılımcı değildir. Sonuçları ona sade Türkçeyle, sayılarla ve önerinle birlikte anlatırsın.
- Bu bir **araştırma** görevidir. Programa (`src-tauri/`, `src/`) kod eklenmez. Çıktılar:
  - Python araştırma betikleri, `docs/arastirma/veritas/` altında;
  - `docs/VERITAS.md`'ye yeni bir bölüm;
  - günün devir notu (`docs/devlog/`).
- Dürüstlük:
  - Her sayı bir betikle tekrar üretilebilir olmalı.
  - Kâhin (gerçek değeri bilen) sonuçlar kâhin diye etiketlenir.
  - Ölçülmeyen şey "ölçülmedi" diye yazılır.
  - Olumsuz sonuç, olumlu sonuç kadar eksiksiz raporlanır.

## 1. Kurulmuş olgular (yeniden türetme, doğrudan kullan)

### 1.1 Model

```
x ∈ ℝⁿ           kodlayıcının gördüğü ses (LAME'in alçak geçiren süzgecinden sonra)
z = A·x          MP3 hibrit dönüşümü: 32 bant polifaz + MDCT (18 ya da 3×6) + örtüşme kelebekleri; granül 576 katsayı
x = S·z          standart çözücünün sentezi (ISO 11172-3)
q                dosyadaki tam sayılar;  r(q) = sgn·|q|^(4/3)·Δ  çözücünün noktası
Δ = 2^((global_gain − 210)/4) · 2^(−m·(scalefac + preflag·pretab))          uzun blok
Δ = 2^((global_gain − 210 − 8·subblock_gain)/4) · 2^(−m·scalefac)           kısa blok,  m = (1 + scalefac_scale)/2
```

### 1.2 Kutular (gerçek LAME 3.100 ile ölçüldü: kapsama %100)

```
q ≠ 0:  |z|/Δ ∈ [((q−1)^(4/3) + q^(4/3))/2, (q^(4/3) + (q+1)^(4/3))/2)     LAME adj43 kuralı (ISO'nun 0,4054'ü değil)
q = 0, bantta en az bir q ≠ 0:   |z| < Δ/2          (işaret bilinmez)
q = 0, bandın bütün q'ları 0:    |z| < Δ_genel/2,   Δ_genel = 2^((global_gain − 210)/4)
```

- ISO kuralıyla kapsama yalnızca %94–96.
- Bütün katsayıları sıfır olan bantlarda LAME ölçek çarpanını "fark etmez" sayar (`best_scalefac_store`, −2) ve
  sonradan değiştirir. Bu yüzden bu bantlarda yalnızca genel kazanç güvenilirdir.
- Tek istisna: LAME'in iç `max_nonzero_coeff` indeksindeki katsayı (96 kbps'te 15 katsayı); kutusuz sayılır.

### 1.3 Çözücü ve süzgeç bankası (`hibrit.py`, `deney.py`)

- Dökümdeki tam sayılardan kurulan ses, ffmpeg'in çözümüyle **−117 ile −125 dB arasında** aynıdır. Bu sonuç MPEG-1
  44,1 kHz, MPEG-2 24 kHz, 32 kHz, kısa blok ve M/S için geçerlidir.
- Polifaz penceresi D, LAME'in `mpglib/tabinit.c` tablosundan çalışma anında okunur (depoya kopyalanmaz).
  D[i] = (−1)^⌊i/64⌋·p[i]; p simetrik ilk örnek süzgecidir. ISO analiziyle yeniden kurma hatası −84,5 dB.
- **SᵀS = c_t·I granül başına:** c = 288 (uzun, başlangıç ve bitiş pencereleri), c = 96 (kısa blok). Dolayısıyla
  A = (SᵀS)⁻¹Sᵀ = Sᵀ/c_t. A·S = I (saf uzun blokta hata 3,6·10⁻⁹); tek istisna son granül (sinyal orada kesilir).
- **Tutarlılık kümesine kesin izdüşüm:** C = {x : (A·x)_k ∈ B_k ∀k} dışbükeydir ve

  ```
  P_C(x) = S · clip(A·x, lo, hi)
  ```

  Kanıt: ‖S·w‖² = Σ_t c_t‖w_t‖² olduğundan ‖x − S·z‖² = Σ_t c_t‖A·x − z‖²_t ifadesi katsayı başına ayrışır; en küçüğü
  kırpmadır. Sayısal olarak doğrulandı: 20 rastgele tutarlı noktanın hepsi daha uzak, P_C idempotent.

  **Sonuç:** her üretici ya da yinelemeli yöntem, adım başına bir analiz, bir kırpma ve bir sentezle dosyayla
  **kesin** tutarlı kılınabilir ("kutu bekçisi").

### 1.4 Ölçülmüş sonuçlar (R1)

- Deney müziği: altı parça (pop, piyano, akustik, elektronik, orkestra, vurmalı), 20 sn. Programla yazıldı
  (`beste.py`), FluidSynth + FluidR3_GM (MIT) ile çalındı.
- Ölçü: S(θ)'ya göre SNR ve Zimtohrli MOS (`deney.mos`; Google, Apache-2.0; 48 kHz).
- 64 kbps (24 kHz) MOS, sırasıyla pop, piyano, akustik, elektronik, orkestra, vurmalı:

| Yöntem                                      | MOS (6 parça)                       | Ort.      |
| ------------------------------------------- | ----------------------------------- | --------- |
| Standart çözücü                             | 4,349 4,722 4,604 4,763 4,504 4,825 | 4,628     |
| Wiener + kesik Gauss (en küçük hata)        | 4,171 4,749 4,552 4,750 4,500 4,821 | 4,591     |
| Wiener + kesik Gauss + bant enerjisi koruma | 4,354 4,754 4,603 4,770 4,502 4,828 | 4,635     |
| Faz-tutarlı tonal delik doldurma            | 4,370 4,738 4,607 4,766 4,504 4,828 | 4,636     |
| Bit akışından kestirilen enerjiyle gürültü  | 3,765 4,167 4,547 4,715 4,227 4,752 | 4,362     |
| Kâhin: deliklerin gerçek enerjisi, gürültü  | 4,553 4,652 4,682 4,782 4,552 4,796 | 4,670     |
| **Kâhin: q = 0 katsayılar gerçek**          | 4,872 4,867 4,876 4,883 4,857 4,894 | **4,875** |
| Kâhin: q ≠ 0 katsayılar gerçek              | 4,296 4,864 4,691 4,849 4,635 4,912 | 4,708     |

- 128 kbps'te çözücünün ortalaması 4,910, kâhin tavanı en fazla 4,955. **Bu görev 96 kbps ve altıyla ilgilidir.**

### 1.5 Bulgular

1. En küçük ortalama hatalı kestirim (sonsal ortalama) enerjiyi kısar ve MOS'u düşürür; algı–bozulma ödünleşimi
   (Blau ve Michaeli 2018). Bant enerjisi korununca MOS çözücüye eşitlenir.
2. Genliğe bakan önsellerin tavanı alçaktır. Kâhin Wiener 128 kbps'te +1,1–2,0 dB SNR verir ama MOS'u değiştirmez
   (4,910 → 4,912).
3. Algısal kayıp deliklerdedir (q = 0). Kâhin bunları gerçek değerle doldurunca +0,25 MOS; bu kazancın ~%87'si
   5 kHz üstündeki deliklerden gelir.
4. Deliklerin yalnızca enerjisi yetmez. Gerçek enerjiyle gürültü doldurma pop'ta +0,20, piyanoda −0,07 MOS verir;
   **tutarlı ince yapı** (işaret ve faz) gerekir.
5. Bit akışı bu yapıyı taşımaz:
   - Canlı bantlardaki deliklerin enerjisi Laplace modeliyle doğru kestirilir (sapma +0,7 / +1,8 dB).
   - Bütün bandı sıfır granüllerde komşu granüllerden yapılan kestirim 11–18 dB fazla çıkar.
   - Komşu 5 granül × 7 frekanstan öğrenen doğrusal kestirici deliklerin hatasını yalnızca +0,02 dB azaltır.

## 2. Çözülecek problem

Yalnızca dosyanın kanıtını (B) ve **genel bir müzik önselini** kullanarak, x̂ ∈ C koşuluyla algısal kaliteyi en
yükseğe çıkaran bir kestirici. Algı–bozulma kuramına göre hedef sonsaldan örneklemedir:

```
x̃ ~ p(x | A·x ∈ B)  ∝  p(x) · Π_k 1{(A·x)_k ∈ [lo_k, hi_k]}
```

Bu, sıradan bir "yapay zekâ ile iyileştirme" değildir. Fark şudur: dosyada kanıtı olan her şey korunur; model yalnızca
deliklerde, |z| < Δ/2 sınırı içinde serbest kalır.

**S1, bilgi kuramı.** Kâhin açığının (+0,25 MOS) ne kadarı ilkece kapatılabilir?

- Delik katsayıları ile (bit akışı + bağlam) arasındaki karşılıklı bilgiyi, deney derleminde öğrenilmiş bir koşullu
  yoğunlukla kestir. Ölçü olarak delik katsayısı başına bit cinsinden negatif log-olabilirliği kullan; karşılaştırma
  "kutu içinde düzgün" ve "sıfırda Laplace" modelleri olsun.
- Algısal açıkla ilişkisini sayısal olarak göster.

**S2, yöntem.** Tutarlılık izdüşümlü bir üretici örnekleyici:

- MDCT alanında bir skor/akış (score/flow matching) ya da difüzyon modeli. Koşulu: çözülmüş spektrum, kutu
  genişlikleri, blok türleri, M/S. Her adımdan sonra P_C uygulanır; alternatif olarak aralık olabilirliğiyle sonsal
  örnekleme (DPS) kullanılabilir.
- En yakın önceki çalışma: Villasana, Villemoes, Klejsa, Hedelin (Dolby), "Audio decoding by inverse problem solving",
  arXiv 2409.07858 (kendi kodekleri için; MP3 değil).
- Kısıtlar:
  - Yalnız işlemci, GPU yok.
  - Çevrimdışı arka planda şarkı süresi kadar ya da daha kısa sürede çalışmalı (4 çekirdek).
  - En fazla ~20 milyon parametre.
  - Ağırlıklar GPL-3.0 ile dağıtılabilir olmalı.

**S3, klasik karşılaştırma.** Öğrenme olmadan tonal deliklerde ne kadar gidilebilir?

- Aralık gözlemli EP / Kalman düzleştiricisiyle faz-tutarlı sinüzoidal izleme.
- MDCT katsayıları, sabit frekanslı bir tonun karmaşık genliğinde doğrusaldır; granül başına dönüş ω·576'dır.
  Frekans duyarlılığı gereksinimi < 1 Hz düzeyindedir.
- Uzun STFT'den tepe seçmeyle yapılan basit sürüm +0,008 MOS verdi; bunun tavanını göster.

**S4 (isteğe bağlı).** Kısa blok kutuları ve zaman zarfı önseliyle ön yankının azaltılması.

## 3. Veri ve lisans

- **Eğitim verisi yalnızca temiz kaynaklardan:**
  - Programla yazılmış parçalar: `beste.py`'yi genişlet (daha çok tür, çalgı, tempo, rastgelelik, efekt), FluidR3_GM
    (MIT) ile çal.
  - Ubuntu paketlerinden gelen başka örnek kütüphaneleri ancak lisansları tek tek doğrulanırsa.
  - MUSDB18 ve MoisesDB (ticari kullanım yasak) ve telifli şarkılar alınmaz.
  - Proje sahibinin kendi kayıpsız arşivi ancak onun onayıyla ve yalnız onun bilgisayarında kullanılır. Bu arşivle
    eğitilmiş ağırlıkların dağıtımı hukuken belirsizdir; önce sor.
- **Kodlama:** yamalı LAME 3.100 (`lame-3.100-veritas-dump.patch`; adımlar `README.md`'de), 64 ve 96 kbps CBR, V5–V7
  VBR. Gerçek değer θ dökümden gelir. Kodlayıcı çeşitliliği (FhG, eski LAME) ölçülmedi; bu bir sınırdır, raporla.
- **Sınama:** R1'in altı parçası eğitimde **asla** kullanılmaz; ayrıca yeni sınama besteleri üret.

## 4. Ölçüler ve kapılar

- Ölçüler:
  - `deney.mos` (Zimtohrli);
  - S(θ)'ya göre SNR;
  - kutu dışı oranı (her zaman 0 olmalı);
  - çalışma süresi;
  - parça başına en kötü durum.
- **K1:** 64 kbps'te altı sınama parçasında ortalama MOS ≥ çözücü + 0,10; hiçbir parça çözücüden 0,02'den fazla
  kötü değil. 96 kbps'te hiçbir parça kötüleşmiyor. Tutarlılık %100.
- **K2:** eğitimde görülmeyen besteler, eğitimdekilere yakın sonuç veriyor (ezber yok). Gerçek kayıtlara genelleme
  burada ölçülemez; bunu açıkça yaz.
- **K3 (senin kapsamın dışında):** proje sahibinin kör dinleme testi (ABX); dinlenecek örnekleri hazırla.
- K1 geçilmezse dur ve nedenini sayılarla yaz; "biraz daha deneyelim" ile ürün önerisine geçilmez.

## 5. Ortam (bu projede ölçülen)

- Linux kapsayıcı, 4 çekirdek, 15 GB bellek, GPU yok, ~3 GB boş disk.
- Ağ: yalnızca PyPI, Ubuntu apt, crates.io ve npm erişilebilir; GitHub web, arXiv ve Hugging Face kapalı.
  PyTorch işlemci sürümü pip ile kurulabilir (BSD).
- Gerekenler: ffmpeg, `fluidsynth`, `fluid-soundfont-gm`, Python'da numpy, scipy, mido, zimtohrli.
- LAME 3.100 kaynağı: sha256 `ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e`. Ubuntu'da
  `apt-get source lame` ile alınabilir (deb-src gerekir); erişemezsen proje sahibine sor.

## 6. Teslim

1. `docs/VERITAS.md`'ye "R2" bölümü (Türkçe): formülasyon ve kanıtlar, yöntem, sonuç tabloları (R1 ile aynı ölçüler
   ve parçalar), karar.
2. `docs/arastirma/veritas/r2_*.py`: `deney.Track` arayüzünü kullanan, tekrar üretilebilir betikler.
3. K1 geçerse bir ürün tasarım notu (kod değil): çözme yolunda nerede çalışır (gerçek zamanlı ses çıkışında değil),
   işlemci bütçesi, önbellek, bit-perfect modda kapalı oluşu, boşluksuz çalmayla uyum, model dosyasının boyutu ve
   lisansı.
4. Devir notu ve proje sahibine sade bir özet: ne denendi, sayılar, öneri.

## 7. Bilinen tuzaklar

- LAME'in `l3_enc`'i işaretsizdir; işaret `xr`'den gelir.
- scfsi: ikinci granülde yeniden kullanılan ölçek çarpanları −1 yazılır.
- sfb21 (uzun) ve sfb12 (kısa) için ölçek çarpanı gönderilmez.
- 96 kbps stereo → 32 kHz; 64 kbps → 24 kHz (MPEG-2, çerçeve başına tek granül). `deney.Track` ikisini de doğru
  çözer.
- LAME gecikmesi 1105 örnektir (ffmpeg ile hizalamada).
- Zimtohrli 48 kHz ister ve bant enerjisi dağılımına duyarlıdır: enerjiyi kısan her yöntem cezalanır. MOS bir
  modeldir, dinleme değildir.
- SNR tek başına ölçüt değildir: R1'de SNR'ı en çok artıran yöntemler MOS'u düşürdü.
