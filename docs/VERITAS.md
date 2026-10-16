# Veritas: kayıplı dosyaları kanıtla iyileştirme (araştırma, 14–16 Ekim 2026)

**Durum:** araştırma ve ölçüm tamamlandı. R1 (bölüm 7): klasik yol kapıdan geçmedi. R2 (bölüm 8): öğrenmeli önsel de
kapıdan geçmedi ve nedeni ölçüldü (bit akışı deliklerin ince yapısını taşımıyor). **MP3 kolu kapandı.** Programda
Veritas kodu yok. Kör dinleme aracı (ABX) 0.0.40'ta programa girdi (çekmecede **Kör test**); sıradaki karar proje
sahibinin (bkz. [ROADMAP.md](ROADMAP.md), Faz 3, "Retriever").

Bu belge üç şeyi bir arada tutar:

- ses mühendisinin "MP3 safi gürültü, yazılımın etkisi olmaz" görüşüne ölçülmüş bir cevap;
- başka bir yapay zekânın gönderdiği "Lyraska Veritas, teknik şartname v1"in bağımsız değerlendirmesi;
- gerçek LAME 3.100 kodlayıcısıyla yapılan F0 kapsama ölçümü ve buna göre düzeltilmiş yol.

Sayıların hepsi [arastirma/veritas/](arastirma/veritas/) altındaki betiklerle tekrar üretilebilir.

## 1. Kısa cevap

**Ses mühendisinin haklı olduğu yerler**

- MP3 bilgi atar ve atılan bilgi geri gelmez. Hiçbir yazılım dosyada olmayan bilgiyi "kanıtla" geri getiremez.
- 128 kbps ve altında duyulabilir bozulmalar vardır: ön yankı (vuruştan önce gelen hışırtı), 16 kHz civarında
  kesilen tizler, tizlerde "kuş cıvıltısı" (birdie) denen titreşen çizgiler.
- **MP3'ü FLAC'a ya da WAV'a çevirmek hiçbir şey kazandırmaz.** Aynı bozulma, daha büyük bir dosyada durur.
- İyi bir çalar MP3'ü "daha iyi çalarak" iyileştiremez; yalnızca kötüleştirmemeyi garanti eder.

**Haksız olduğu yerler**

- "Safi gürültü" değil. Ölçüm (bölüm 4, deney sesinde): 320 kbps'te sinyal/gürültü oranı 39 dB. 96 kbps'te 22 dB ama bu gürültü
  kulağın duymadığı yerlere (maskelenen bölgelere) yerleştirilir; SNR algıyı ölçmez.
- Dinleme testleri: LAME'in V0–V3 ayarları ve 320 kbps, dinleyicilerin çoğu ve içeriğin çoğu için orijinalinden
  ayırt edilemez (Hydrogenaudio). Öte yandan eğitimli dinleyiciler 192 kbps'e kadar CD'yi tercih edebilmiştir (Pras ve ark.,
  AES 2009). Ayırt edilebilen "zor örnekler" keskin vuruşlu kayıtlardır (kastanyet, klavsen).
- "Yazılımın etkisi olmaz" yanlış: çalarlar sesi bozabilir (16 bit iç işleme, kaba yeniden örnekleme, kırpma).
  Lyraska'nın ses yolu bunları ölçerek dışlar: iç işleme 64 bit; ses yolunun eklediği bozulma ve gürültü ≤ −140 dB
  (`ses_yolu_bozulma_ve_gurultu_eklemez`); yeniden örneklemede Nyquist üstü ≥ 140 dB bastırılır; özel modda
  bit-perfect çıkış; eşitleme gerçek tepeyi −1 dBTP'nin üstüne çıkarmaz.

**Daha önemli olan: standart çözücü kanıtın tamamını kullanmaz.** MP3 dosyası her frekans katsayısı için bir tam sayı
(q) ve bir adım (Δ) taşır. Bunlardan gerçek değerin **hangi aralıkta** olduğu kesin olarak bilinir. Standart çözücü
bu aralıktan tek bir nokta seçer ve geri kalan bilgiyi kullanmaz. Veritas'ın fikri: aralığın dışına çıkmadan, müziğin
yapısına (tonların sürekliliği, komşu katsayılar) en uygun noktayı seçmek. Bu "uydurma" değildir: düzeltilen
katsayılar aynı adımlarla yeniden nicemlenirse dosyadaki tam sayıların aynısı çıkar.

## 2. MP3'te ne biliniyor: matematik

### 2.1 Nicemleme ve kutu

MP3 Layer III, sesi 576 katsayılık granüllere ayırır (32 bantlı polifaz süzgeç + MDCT, "hibrit süzgeç bankası").
Her katsayı θ için dosyada q (işaretli tam sayı) ve bant başına ölçek bilgisi bulunur:

```
Uzun blok:  Δ = 2^((global_gain − 210)/4) · 2^(−m · (scalefac + preflag · pretab))
Kısa blok:  Δ = 2^((global_gain − 210 − 8 · subblock_gain)/4) · 2^(−m · scalefac)
            m = (1 + scalefac_scale) / 2

Çözücünün noktası:  r(q) = sgn(q) · |q|^(4/3) · Δ
```

Kodlayıcının kuralı bilinirse her q için θ'nın içinde olduğu aralık (kutu) kurulur. ISO 11172-3'ün örnek
(bilgilendirici) kodlayıcısındaki kural ile LAME'in gerçekte kullandığı kural farklıdır:

```
ISO:   q = ⌊(|θ|/Δ)^(3/4) + 0,4054⌋          →  |θ|/Δ ∈ [(q − 0,4054)^(4/3), (q + 0,5946)^(4/3))
LAME:  karar sınırı komşu yeniden kurma       →  |θ|/Δ ∈ [((q−1)^(4/3) + q^(4/3))/2, (q^(4/3) + (q+1)^(4/3))/2)
       noktalarının ortası (adj43 tablosu)
q = 0 (iki kuralda da):  |θ| < Δ/2, işaret bilinmez
```

| q   | ISO kutusu (Δ birimi) | LAME kutusu (Δ birimi) | Çözücünün noktası |
| --- | --------------------- | ---------------------- | ----------------- |
| 0   | [0, 0,500)            | [0, 0,500)             | 0                 |
| 1   | [0,500, 1,863)        | [0,500, 1,760)         | 1,000             |
| 2   | [1,863, 3,565)        | [1,760, 3,423)         | 2,520             |
| 3   | [3,565, 5,506)        | [3,423, 5,338)         | 4,327             |
| 4   | [5,506, 7,638)        | [5,338, 7,450)         | 6,350             |
| 5   | [7,638, 9,932)        | [7,450, 9,726)         | 8,550             |

Ayrıntılar: uzun blokta sfb21, kısa blokta sfb12 için ölçek çarpanı gönderilmez (0 alınır); ikinci granül scfsi ile
birinci granülün ölçek çarpanlarını yeniden kullanabilir; ortak stereoda (M/S) kutular M ve S kanallarındadır.

### 2.2 Seviye 0: kutunun ağırlık merkezi

Müzikte katsayıların dağılımı sıfırda sivridir (Laplace'a yakın). Bu yüzden kutunun içindeki ortalama değer, çözücünün
noktasından biraz aşağıdadır. Ortalamayı (ağırlık merkezini) seçmek ortalama hatayı azaltır. Laplace(b) için hesap
(`sartname_dogrulama.py`; şartnamedeki ISO sayıları doğru çıktı):

| b/Δ        | 0,3     | 0,5     | 0,8     | 1,0     | 2,0     | 4,0     |
| ---------- | ------- | ------- | ------- | ------- | ------- | ------- |
| ISO kuralı | 0,65 dB | 0,22 dB | 0,04 dB | 0,03 dB | 0,14 dB | 0,25 dB |
| LAME kur.  | 0,70 dB | 0,39 dB | 0,22 dB | 0,18 dB | 0,11 dB | 0,07 dB |

Gerçek LAME verisinde, kâhin (gerçek θ'yı bilen) ağırlık merkezi bile toplam hatayı en fazla **0,14 dB** azaltıyor
(bölüm 4). Seviye 0 tek başına değmez; yalnızca daha güçlü yöntemlerin temelidir.

### 2.3 Örnekleme ile ortalama arasındaki seçim

Kutunun içinden olasılığa göre rastgele bir değer seçmek (örnekleme) kulağa doğal gelebilir ama ortalama karesel
hatayı ikiye katlar:

```
θ̃, θ ~ p(θ | q) bağımsız  →  E‖θ̃ − θ‖² = 2 · MMSE    (+3,01 dB hata; Monte Carlo: 2,0045)
```

Bu yüzden iki kip ayrı tutulur: **Sadık** (ortalama: en küçük hata) ve **Doğal** (örnekleme: daha canlı ama ölçülen
hatası daha büyük). Hangisinin tercih edildiğine yalnızca dinleme testi karar verir.

### 2.4 Aynı şarkının birden fazla kopyası

Aynı şarkının M farklı kodlaması varsa ve kutuları bağımsız kayıyorsa, kesişim daralır:

```
MSE_M = δ² / (2 (M+1)(M+2))    →  M = 2: 3,01 dB, M = 3: 5,23 dB, M = 5: 8,45 dB (Monte Carlo ile aynı)
```

Bu ideal bir üst sınırdır. Gerçek kopyalar çoğu zaman aynı kodlayıcıdan çıkar; kutuları bağımsız değildir, kazanç
çok daha küçüktür (şartnamenin kendi 16. bölümü de bunu söylüyor). Ayrıca kopyalar örnek düzeyinde hizalanmalıdır.

### 2.5 Dither

16 bitlik çıkışta TPDF dither, sinyal/nicemleme gürültüsü oranını 98,1 dB'den 93,3 dB'ye indirir (bozulmayı
duyulmayan düz bir gürültüye çevirmenin bedeli). Lyraska aygıta 24/32 bit ya da kayan
nokta gönderdiğinde gerekmez; 16 bitlik özel modda gerekirse mevcut çıkış katında yapılır, Veritas'ın işi değildir.

## 3. Literatür taraması (on dil)

Tarama Çince, Japonca, Korece, İngilizce, Almanca, Çekçe, Fransızca, İspanyolca, Portekizce, Rusça, Arapça ve
İskandinav dillerinde yapıldı. **Yöntem sınırı:** tarama sırasında makale sayfalarının çoğu açılamadı (ağ hatası);
sayıların bir kısmı özetlerden ve arama sonuçlarından okundu. "[doğrulanmadı]" işaretli bilgiler ve uygulamaya
geçmeden önce kullanılacak her sayı tam metinden doğrulanmalıdır.

### 3.1 Ana soru: MP3 kutuları kesin kısıt olarak kullanılmış mı?

**Hiçbir dilde, MP3 ya da AAC bit akışının nicemleme aralıklarını (q, ölçek çarpanları, global_gain) iyileştirmede
kesin kısıt olarak kullanan yayımlanmış bir çalışma bulunamadı.** Bu yüzden "ilk" demiyoruz; "MP3 için yayımlanmış
örneğini bulamadık" diyoruz. Fikrin kendisi yeni değil, yakın akrabaları var:

- **JPEG'de aynı fikir (1992–2012):** sıkıştırılmış resmi DCT katsayılarının nicemleme aralıklarına izdüşürerek
  blokluluğu gidermek (Zakhor 1992; Yang, Galatsanos, Katsaggelos 1993; Bredies ve Holler, "A total variation-based
  JPEG decompression model", SIAM J. Imaging Sci. 2012). Ehrlich ve ark. (ECCV 2020) ağı nicemleme tablosuyla
  koşullar. Veritas bunun MP3 karşılığıdır.
- **Tutarlı yeniden kurma kuramı:** Thao ve Vetterli (1994), Goyal, Vetterli ve Thao (1998): kutuya tutarlı
  kestirim, doğrusal kestirimden daha hızlı küçülen hata verir.
- **Dolby, "Audio decoding by inverse problem solving"** (Villasana T., Villemoes, Klejsa, Hedelin; arXiv 2409.07858,
  ICASSP 2025): bir dönüşüm kodeğini, nicemleme aralıklarından kurulan koşulla difüzyon modeliyle çözüyor; eski
  çözücüden daha iyi. En yakın çalışma budur, ama Dolby'nin kendi örnek kodeği içindir, MP3 değil; kodu bulunamadı.
- **Brno Teknik Üniversitesi (Çekçe kaynaklar dahil):** Záviška, Rajmic, Mokrý, "Audio dequantization using
  (co)sparse (non)convex methods" (ICASSP 2021; MATLAB kodu GPL-3.0); "Flexible framework for audio reconstruction"
  (DAFx 2020) zaman-frekans katsayılarının nicemlenmesini de kapsıyor. Ancak deneyler zaman alanında kaba (2–8 bit)
  PCM nicemlemesi üzerinde; algısal olarak şekillendirilmiş MP3 gürültüsüne aktarıldığı gösterilmemiş. Kovanda ve
  Rajmic (PHADQ, arXiv 2510.16813) faz bilgisini ekliyor.
- **Brauer, Gerkmann, Lorenz** (ICASSP 2016): konuşma kodeklerinde kutu kısıtlı seyrek yeniden kurma.
- **Lapierre ve Lefebvre** (ICASSP 2017): AAC çözücüsünde ölçek çarpanlarından bant gürültüsünü kestirip ön yankıyı
  azaltma; 12–32 kbps'te +0,22–0,45 dB SNR, yaklaşık +2 MUSHRA puanı.
- **Fraunhofer** (Gupta ve ark., ICASSP 2022, arXiv 2201.12039): LC3 konuşma kodeği için MDCT alanında son süzgeç;
  16 kbps'te yaklaşık +10 MUSHRA. Aralıkları kısıt olarak kullanmıyor; MDCT katsayılarını değiştirmenin zaman
  alanındaki örtüşme iptalini (TDAC) bozabileceğini hatırlatıyor.
- **Opus LACE / NoLACE** (Büthe, Mustafa, Valin): çözücü parametrelerine koşullu son süzgeç; yalnız konuşma.

### 3.2 Derin öğrenmeyle MP3 iyileştirme ve bant genişletme

| Çalışma                                            | Lisans                                     | Not                                                                         |
| -------------------------------------------------- | ------------------------------------------ | --------------------------------------------------------------------------- |
| Apollo (Tsinghua + Tencent AI Lab; Kai Li, Yi Luo) | Kod CC BY-SA 4.0; ağırlıkların lisansı yok | Gerçek MP3 (24–128 kbps) ile eğitilmiş, kodu açık; 128 kbps'te kazanç küçük |
| Lattner ve Nistal (Sony CSL Paris, 2021)           | Kod bulunamadı                             | 16/32 kbps'te iyi; **64 kbps'te düz MP3'ten biraz kötü**                    |
| AudioSR (Haohe Liu)                                | Kod MIT, ağırlık Apache-2.0                | MP3 ile eğitilmemiş; 6 GB; GPU ister                                        |
| BABE (Aalto, Moliner ve Välimäki)                  | MIT                                        | Kör bant genişletme; A100'de 8 sn ses için ~1 dk                            |
| AEROMamba (UFRJ, Brezilya)                         | CC0                                        | Bant genişletme; PAQM sürümü 32 kbps MP3'e yönelik, deposunda lisans yok    |
| Samsung FTD-CNN (Porov ve ark., AES 2018)          | Kod yok                                    | MP3 müziği, MDCT genliği üzerinde [sayılar doğrulanmadı]                    |
| ETRI (Kore, 2020)                                  | Kod yok                                    | Müzik süper çözünürlük, +3,41 dB SNR                                        |
| NVIDIA A2SB, Adobe "Music Enhancement"             | **Ticari kullanım yasak: alınamaz**        |                                                                             |

**Ders:** üretici (yapay zekâ) yöntemler düşük bit hızında yardım ediyor, iyi dosyaları **kötüleştirebiliyor**
(Sony CSL, 64 kbps). Ağırlık lisansları çoğu zaman belirsiz ya da eğitim verisi (MUSDB18) ticari kullanımı
yasaklıyor. Bu yüzden Veritas'ın temeli öğrenilmiş model değil, kanıta dayalı kestirimdir.

### 3.3 Dillere göre öne çıkanlar

- **Çince:** Apollo (yukarıda). QQ Music 臻品母带, Kugou 蝰蛇 (MediaTek ile), NetEase 超清母带: yalnızca pazarlama
  açıklamaları; yüzde iddiaları tanımsız. Tayvan NCTU (Chi-Min Liu, 2003–2009): MDCT alanında üst bant tamamlama,
  "sıfır bant dither" [ayrıntı doğrulanmadı].
- **Japonca:** Sony DSEE (HX/Extreme/Ultimate): üst bandı alt banttan öğrenilmiş katsayılarla kestirir; ilgili Sony
  patentleri (ör. US10134418) var. JVC K2, Maxell Bit-Revolution, Yamaha Compressed Music Enhancer: kapalı. 2007 tarihli
  akademik çalışmalar 16 kHz'te kesilmiş MP3'ün tizini tamamlamaya yönelik. MP3 için Japonca derin öğrenme çalışması
  bulunamadı.
- **Korece:** Samsung UHQ Upscaler (algoritma açıklanmamış), Samsung FTD-CNN (yukarıda); 2007–2008 çalışmaları MP3
  çözücüsünün MDCT alanında üst bant tamamlama (Liu yönteminin geliştirmeleri).
- **İngilizce:** Dolby, Lapierre ve Lefebvre, AudioSR, Hydrogenaudio dinleme testleri; Pras ve ark. (AES 2009).
- **Almanca / Çekçe:** Brno (GPL-3.0 kodu var), Fraunhofer son süzgeci ve "kuş cıvıltısı" patentleri, Brauer ve ark.
- **Fransızca:** Sony CSL Paris (yukarıda); Nantes (Grumiaux ve Lagrange) DDSP ile bant genişletme (deposu lisanssız);
  Inria'nın kırpılma onarımı (SPADE).
- **Portekizce:** AEROMamba (CC0); Porto'da (INESC TEC) farklı süzgeçlerle eğitilmiş bant genişletme.
- **Rusça:** Habr'da MP3/AAC/Opus ile eğitilmiş 8,8 milyon parametreli bir model yazısı (kod yok; LAME'in 1105 örneklik
  gecikmesinin karşılaştırmada düzeltilmesi gerektiğini hatırlatıyor); HiFi++ (Samsung AI Moskova).
- **İskandinav:** Aalto (BABE, MIT; Bäckström grubunun ek bilgi gerektirmeyen çözücü son süzgeçleri, konuşmada
  yaklaşık +4 dB algısal SNR); SBR Stockholm'de (Coding Technologies) icat edildi, iletilen zarf bilgisi ister.
- **İspanyolca, Arapça:** kayda değer bir kaynak bulunamadı.

## 4. Ölçüm: gerçek LAME 3.100 ile F0 kapsama testi

Şartnamenin her şeyi bağladığı varsayım şu: "gerçek katsayı θ, çözücünün kurduğu kutunun içindedir". Bu varsayım
ölçülmeden tek satır kod yazılmamalıydı; ölçüldü.

**Yöntem:** LAME 3.100'ün kaynak koduna küçük bir ölçüm yaması eklendi
([lame-3.100-veritas-dump.patch](arastirma/veritas/lame-3.100-veritas-dump.patch)). Yama, nicemleme bittikten sonra her
granülün θ değerlerini (kodlayıcının nicemlediği MDCT katsayıları), q değerlerini ve yan bilgisini döker. Kutular
yalnızca çözücünün görebildiği bilgiden kuruldu (scfsi ve gönderilmeyen bantlar çözücünün kuralıyla). Deney sesi:
12 saniyelik sentetik stereo müzik (piyano benzeri notalar, titreşimli yaylı katman, kick, hi-hat). Gerçek şarkı
kullanılmadı (telif ve depo kuralı).

Katsayılar üç sınıfa ayrıldı:

- **A:** q ≠ 0. Değer bir aralıkta, işareti biliniyor.
- **B:** q = 0, kodlayıcının üst kesiminin altında. |θ| küçük, işareti bilinmiyor.
- **C:** üst kesimin üstü. Kodlayıcı alçak geçiren süzgeçle silmiş; kanıt yok.

### 4.1 Sonuçlar

| Ölçü                                             | 96 kbps\*   | 128 kbps    | 320 kbps    | V2 (VBR)    |
| ------------------------------------------------ | ----------- | ----------- | ----------- | ----------- |
| Katsayı payı A / B / C                           | 24 / 71 / 5 | 23 / 70 / 7 | 56 / 41 / 3 | 35 / 61 / 4 |
| A kapsaması, ISO kuralı (c = 0,4054)             | %95,954     | %95,705     | %93,999     | %94,597     |
| **A kapsaması, LAME kuralı**                     | **%100**    | **%100**    | **%100**    | **%100**    |
| B kapsaması, tek kutu (\|θ\| < Δ/2)              | %99,398     | %99,705     | %99,997     | %100        |
| **B kapsaması, bant kuralı** (4.2)               | **%99,997** | **%100**    | **%100**    | **%100**    |
| Hata enerjisinin A / B payı                      | %65 / %35   | %61 / %39   | %84 / %16   | %79 / %21   |
| Katsayı alanında SNR                             | 25,8 dB     | 27,8 dB     | 40,0 dB     | 32,4 dB     |
| Seviye 0 üst sınırı (kâhin ağırlık merkezi)      | 0,11 dB     | 0,14 dB     | 0,04 dB     | 0,09 dB     |
| B'deki hatanın tamamı silinse kazanç (üst sınır) | 1,85 dB     | 2,12 dB     | 0,74 dB     | 1,03 dB     |

\* LAME 96 kbps stereo'da sesi 32 kHz'e indirir; bu sütun 32 kHz'tedir.

Aynı deney sesinin ffmpeg (libmp3lame) ile bit hızına göre dalga biçimi ölçümü (`bit_hizi.py`):

| Bit hızı  | 96 kbps  | 128 kbps | 192 kbps | 256 kbps | 320 kbps |
| --------- | -------- | -------- | -------- | -------- | -------- |
| SNR       | 21,8 dB  | 23,2 dB  | 27,8 dB  | 34,4 dB  | 39,4 dB  |
| Üst kesim | 15,3 kHz | 16,7 kHz | 18,8 kHz | 19,5 kHz | 20,3 kHz |

### 4.2 Bulgular

1. **Şartnamenin yuvarlama varsayımı LAME'de yanlış.** ISO kuralıyla (c = 0,4054) q ≠ 0 katsayılarının %4–6'sı
   kutunun dışında kalıyor. LAME 3.100 karar sınırını komşu yeniden kurma noktalarının ortasına koyuyor (`adj43`
   tablosu); bu kuralla kapsama dört ayarda da **tam %100**. Sonuç: kutu kuralı **kodlayıcıya bağlıdır**; dosyanın
   hangi kodlayıcıdan çıktığı (LAME etiketi) okunmalı, tanınmayan kodlayıcıda kutu genişletilmeli.
2. **q = 0 kutusu ilk biçimiyle kesin değil, bant kuralıyla kesin.** İhlallerin tamamına yakını, bütün katsayıları
   sıfır olan bantlarda. LAME bu bantların ölçek çarpanını "fark etmez" sayıp (`best_scalefac_store`, −2) sonradan
   0'a çekiyor, scfsi ile birinci granülden alıyor ya da preflag ekliyor; çözücünün gördüğü Δ, nicemlemede kullanılan
   Δ değil. Bu bantlarda yalnızca genel kazancın adımı güvenilirdir ve kanıtlanabilir sınır şudur:

   ```
   Bütün q'ları 0 olan bant:  |θ| < Δ_genel / 2,   Δ_genel = 2^((global_gain − 210)/4)
   ```

   (ölçek çarpanı ≥ 0 ve subblock_gain ≥ 0 olduğundan nicemlemedeki adım Δ_genel'den büyük olamaz). Bu kuralla ihlal
   dört ayarda da **sıfır**. Kalan 15 katsayının (yalnızca 96 kbps, kısa blok) hepsi LAME'in iç `max_nonzero_coeff`
   indeksinde; LAME o indeksteki katsayıyı ölçümde granüllerin %99'unda sıfırlıyor. Çözücü bu indeksi dosyadan
   bilemez; kesimin hemen altındaki son katsayı kutusuz sayılır.

3. **Kazancın yeri B sınıfı ama tavanı alçak.** Hatanın %16–39'u kodlayıcının tam sıfırladığı katsayılarda. Yalnızca
   bu hatanın tamamı silinse bile kazanç 0,7–2,1 dB olurdu. Daha fazlası için A kutularının içinde de güçlü bir önsel
   bilgi (tonların zaman içindeki sürekliliği) gerekir: Seviye 2.
4. **Seviye 0 tek başına değmez:** gerçek veride en fazla 0,14 dB.
5. **Kesim üstü (C) kanıtla geri gelmez.** 128 kbps'te 16,7 kHz'in üstü silinmiş; orayı doldurmak tahmindir, ayrı
   tutulmalı ve öyle adlandırılmalıdır.

### 4.3 Şartnamenin 17. bölümündeki prototip

Gönderilen prototip (`veritas_proto.py`: sinüzoidal model + kutuya izdüşüm + kutu ihlal oranıyla kendini doğrulayan
kapı) oyuncak kodekte yeniden çalıştırıldı, aynı sonucu verdi:

| Sinyal                 | Kutu ihlali | Kabul edilen kare | SNR            |
| ---------------------- | ----------- | ----------------- | -------------- |
| Durağan tonlar         | %19,1       | %95,5             | 16,9 → 20,1 dB |
| Vibrato (model yanlış) | %98,4       | %0                | 16,7 → 16,7 dB |

İkinci satır asıl önemli olandır: model sese uymadığında kapı onu reddediyor ve zarar vermiyor. Ancak prototip ISO
kuralını ve 576'lık düz bir MDCT kullanıyor; gerçek LAME dosyasında (adj43 kuralı, hibrit süzgeç bankası, blok
geçişleri, M/S) henüz denenmedi. +3 dB'lik sonuç gerçek MP3 için bir beklenti değil, sınanacak bir hipotezdir.

## 5. Şartnamenin bağımsız değerlendirmesi

**Doğru bulunanlar:** kutu tablosu, Laplace için Seviye 0 kazançları, 2·MMSE özdeşliği, çoklu kopya formülü, dither
hesabı (`sartname_dogrulama.py` ile bağımsız olarak yeniden hesaplandı); seviyelere ayrılmış yapı; "Sadık" ve
"Doğal" kiplerin ayrılması; kendini doğrulayan kapı; kesim üstünün ayrı ve isteğe bağlı tutulması.

**Düzeltilmesi gerekenler:**

1. Yuvarlama sabiti: LAME 3.100'de ISO değil `adj43` kuralı (4.2/1). Kodlayıcı tanıma zorunlu.
2. "B sınıfı kesin" değil; bant kuralıyla kesin (4.2/2).
3. Seviye 0 beklentisi gerçek veride 0,04–0,14 dB.
4. Çıkış zinciri (dither, 24 bit, gerçek tepe): Lyraska'da zaten var ve ölçülüyor; Veritas yeniden yazmaz, çözme
   tarafında katsayıları düzeltip mevcut zincire verir.
5. En yakın önceki çalışma (Dolby 2025) ve JPEG'deki karşılığı şartnamede yok; "daha önce yapılmamış" iddiası
   "MP3 için yayımlanmış örneği bulunamadı" diye düzeltilmeli.
6. Hibrit süzgeç bankası sıkı bir çerçeve değildir; bilinmeyenler katsayılar olarak alınmalı (kutu izdüşümü katsayı
   başına kesin), önsel bilgi çözülmüş dalga biçiminde uygulanmalı. Katsayı değişikliği blok sınırlarında örtüşme
   iptalini bozmamalı; bu ayrıca test edilir.
7. Kesim üstü bant genişletme için patent riski var: Sony'nin bant genişletme patentleri, Fraunhofer'in "kuş
   cıvıltısı" son işlem patentleri (US 11,373,666; 11,562,756; 12,067,995). Bu tekniklere benzeyen her adım
   uygulamadan önce incelenir. MP3'ün kendi patentlerinin süresi 2017'de doldu.
8. Hazır yapay zekâ modelleri: lisansı (kod **ve** ağırlık) doğrulanmadan alınmaz; Apollo'nun ağırlıklarının lisansı
   yok, eğitim verisi ticari kullanımı yasaklıyor olabilir.

## 6. Yol: düzeltilmiş plan

Her adımın sonunda ölçülebilir bir kapı var. Kapı geçilmezse durulur ve sonuç bu belgeye yazılır; "biraz daha
deneyelim" ile ürüne girmez.

> **R1 ve R2 sonrası güncelleme (bölüm 7 ve 8):** F1 klasik önsellerle (R1) ve öğrenmeli önselle (R2) denendi, ikisi de
> kapıdan geçmedi. F0 (Rust bit akışı ayrıştırıcı) ve F3–F4 bu yüzden yapılmaz. Kör dinleme aracı (F2'nin aracı)
> 0.0.40'ta eklendi (çekmecede **Kör test**); tek başına değerlidir.

- **F0, bit akışı ve kutular (Rust).** MP3 bit akışı ayrıştırıcı (yan bilgi, ölçek çarpanları, Huffman) → q, Δ,
  blok türü, M/S; LAME etiketini okuma; kutu kuralı (adj43 + bant kuralı; tanınmayan kodlayıcıda genişletilmiş kutu);
  kendi ters nicemleme ve hibrit sentez.
  - Kapı: kendi çözümümüz symphonia'yla örnek düzeyinde aynı (f32 duyarlılığında); Rust kutuları bu belgedeki
    kapsamayı aynen veriyor; düzeltilmiş katsayılar yeniden nicemlenince aynı q'lar çıkıyor (%100).
- **F1, Seviye 2 gerçek dosyada.** Önce Python araştırma betiği, F0'ın çıktısıyla: sinüzoidal model, kutuya
  izdüşüm, kare başına kapı; gerçek LAME dökümleri ve proje sahibinin kendi kayıpsız arşivinden çiftler.
  - Kapı: 128 kbps'te ortalama ≥ +1 dB SNR; ViSQOL (Apache-2.0) ya da PEAQ (GstPEAQ, LGPL) ile iyileşme; hiçbir
    karede ölçülür kötüleşme yok.
- **F2, kör dinleme.** ABX ve MUSHRA benzeri test aracı; proje sahibi ve ses mühendisi dinler.
  - Kapı: 128 kbps'te anlamlı tercih (ör. ABX'te 16 denemede en az 12 doğru, p ≈ 0,04).
- **F3, ürüne alma.** "MP3 iyileştirme (Sadık)" ayarı; çözme tarafında çalışır, ses çıkış geri çağrısında değil;
  boşluksuz çalma ve şarkı haritasıyla uyumlu; bit-perfect modda kapalı. Hız ölçümüne göre çalarken ya da arka
  planda önceden işlenir.
  - Kapı: ses yolu kuralları (bellek ayırmaz, takılma yok) testlerle; işlemci yükü ölçülür.
- **F4, AAC (M4A).** Aynı fikir; TNS, PNS ve HE-AAC'nin SBR'ı ayrıca ele alınır.
  - Kapı: F0–F2'nin AAC karşılığı.

Ayrı karar olarak:

- **Kesim üstü bant genişletme (C sınıfı):** kanıt yok; ancak F2'den sonra, ayrı bir ayar ("tiz tamamlama, tahmin"),
  varsayılan kapalı, patent incelemesinden sonra.
- **Kör test aracı (ABX):** F2 için gerekli, ama tek başına da değerli: proje sahibi kendi kulağıyla "320 kbps ile
  FLAC'ı ayırabiliyor muyum?" sorusunu cevaplayabilir. Ses mühendisiyle tartışmayı kanıta bağlamanın en ucuz yolu.

**Dürüst beklenti:** 256–320 kbps ve V0–V2 dosyalarda Veritas ölçülebilir küçük bir SNR kazancı verebilir ama duyulur
bir fark beklenmez; bu dosyalar zaten çoğu kişi için şeffaftır. 128 kbps ve altında hata büyük ve kazancın
yeri var; tonal içerikte oyuncak testte +3 dB, gerçek dosyada henüz ölçülmedi. Vuruşlu ve gürültülü seslerde (ön
yankı) ayrı bir yöntem gerekir. Kesim üstü kanıtla geri gelmez.

## 7. R1: gerçek dalga biçiminde ölçüm (14 Ekim 2026)

Proje sahibi MP3 için tam inisiyatif verdi ("daha iyi bir çözüm bulmaya zorla"). F1'in sorusu soruldu: kutulara sadık
bir kestirim **kulağın duyduğu** bir iyileşme getiriyor mu?

### 7.1 Düzenek

- **Çözücü (`hibrit.py`):** MP3 hibrit süzgeç bankası numpy ile yazıldı. Dökümdeki tam sayılardan kurulan ses ffmpeg'in
  çözümüyle aynı: fark −117 ile −125 dB arası (float hassasiyeti). Bu sonuç MPEG-1 44,1 kHz, MPEG-2 24 kHz, 32 kHz, kısa
  blok ve M/S için geçerli.
  - Polifaz penceresinin ISO tablosu, simetrik bir ilk örnek süzgecin her 64 örnekte işaret değiştirmiş hâlidir:
    D[i] = (−1)^⌊i/64⌋ · p[i]. Bu düzenle ISO analiziyle yeniden kurma hatası −84,5 dB.
  - Ölçülen yapı: SᵀS = 288·I (uzun blok), 96·I (kısa blok). Analiz operatörü A = Sᵀ/c granül başına kesin.
- **Deney müziği (`beste.py`):** programla yazılmış altı MIDI parça (pop, akustik, piyano, elektronik, orkestra,
  kastanyet + klavsen). FluidSynth ve gerçek enstrüman örnekleri içeren FluidR3_GM (MIT) ses bankasıyla çalındı.
- **Ölçü:** S(θ)'ya göre zaman alanında SNR ve **Zimtohrli** (Google, Apache-2.0) ile tahmini MOS (1–5). MOS bir
  modeldir, dinleme testi değildir.

### 7.2 Sonuçlar, 64 kbps (LAME 3.100, 24 kHz)

MOS (yüksek iyi); parantez içinde SNR farkı (çözücüye göre). Hiçbir yöntem kutu dışına çıkmadı (ölçüldü: 0).

| Yöntem                                        | pop          | piyano       | akustik      | elektronik   | orkestra     | vurmalı      | Ortalama  |
| --------------------------------------------- | ------------ | ------------ | ------------ | ------------ | ------------ | ------------ | --------- |
| Standart çözücü (SNR 17,4–20,2 dB)            | 4,349        | 4,722        | 4,604        | 4,763        | 4,504        | 4,825        | 4,628     |
| Laplace ağırlık merkezi                       | 4,167        | 4,634        | 4,545        | 4,737        | 4,496        | 4,802        | 4,564     |
| Wiener + kesik Gauss (en küçük hata)          | 4,171 (+0,8) | 4,749 (+1,3) | 4,552 (+0,8) | 4,750 (+0,9) | 4,500 (+0,6) | 4,821 (+0,5) | 4,591     |
| **Wiener + kesik Gauss + enerji koruma**      | 4,354 (+0,5) | 4,754 (+1,1) | 4,603 (+0,6) | 4,770 (+0,7) | 4,502 (+0,5) | 4,828 (+0,4) | **4,635** |
| **Faz-tutarlı tonal delik doldurma**          | 4,370        | 4,738        | 4,607        | 4,766        | 4,504        | 4,828        | **4,636** |
| Bit akışından enerjiyle gürültü doldurma      | 3,765        | 4,167        | 4,547        | 4,715        | 4,227        | 4,752        | 4,362     |
| Kâhin: Wiener (gerçek güç biliniyor)          | 4,314        | 4,827        | 4,672        | 4,826        | 4,620        | 4,859        | 4,686     |
| Kâhin: deliklerin gerçek enerjisi, gürültü    | 4,553        | 4,652        | 4,682        | 4,782        | 4,552        | 4,796        | 4,670     |
| Kâhin: q ≠ 0 katsayılar gerçek                | 4,296        | 4,864        | 4,691        | 4,849        | 4,635        | 4,912        | 4,708     |
| **Kâhin: q = 0 katsayılar (delikler) gerçek** | 4,872        | 4,867        | 4,876        | 4,883        | 4,857        | 4,894        | **4,875** |

### 7.2b Sonuçlar, 128 kbps (44,1 kHz), altı parçanın ortalaması

| Yöntem                                    | MOS (ortalama) | SNR farkı (çözücüye göre) |
| ----------------------------------------- | -------------- | ------------------------- |
| Standart çözücü                           | 4,910          | —                         |
| Wiener + kesik Gauss + enerji koruma      | 4,906          | +0,2 … +0,7 dB            |
| Faz-tutarlı tonal delik doldurma          | 4,910          | −0,2 … +0,1 dB            |
| Kâhin: Wiener (gerçek güç biliniyor)      | 4,912          | +1,1 … +2,0 dB            |
| Kâhin: q = 0 katsayılar (delikler) gerçek | 4,943          | +0,6 … +1,7 dB            |
| Kâhin: q ≠ 0 katsayılar gerçek            | 4,955          | +5 … +9 dB                |

128 kbps'te çözücü zaten 4,87–4,97 arasında; bilgiyi kâhin olarak bilsek bile kazanılacak en fazla +0,045 MOS.
İyileştirmenin anlamlı olduğu yer 96 kbps ve altı.

### 7.3 Bulgular

1. **En küçük hata kulağa daha kötü gelir.** Sonsal ortalama (Laplace, Wiener) SNR'ı artırıyor ama enerjiyi kısıyor:
   MOS düşüyor. Bu, algı–bozulma ödünleşiminin (Blau ve Michaeli, 2018) ölçülmüş hâlidir. Her bandın enerjisi
   korunursa SNR kazancının çoğu kalıyor (+0,4–1,1 dB) ve MOS çözücüye eşit ya da biraz üstünde.
2. **Genliğe bakan önsellerin tavanı alçak.** Gerçek sinyalin gücünü bilen kâhin Wiener bile 128 kbps'te +1,8 dB SNR,
   64 kbps'te ortalama +0,06 MOS veriyor. MP3 gürültüsü sinyalin bulunduğu yerde durduğu için (nicemleyici |θ|^(3/4)
   kuralıyla çalışır) genlik maskeleri onu ayıramaz.
3. **Algısal kayıp tizdeki deliklerde.** Yalnızca q = 0 katsayıları gerçek değerle doldurulunca ortalama MOS +0,25
   artıyor (5 kHz üstü tek başına bunun çoğunu açıklıyor). Buna karşılık q ≠ 0 katsayıların hepsi kusursuz yapılınca
   SNR +4–6 dB artıyor ama MOS yalnızca +0,08 artıyor.
4. **Deliklerin yalnızca enerjisi yetmez; tutarlı ince yapı gerekir.** Gerçek enerjiyle gürültü doldurmak gürültülü
   parçalarda yardım ediyor (pop +0,20), tonal parçalarda zarar veriyor (piyano −0,07); ortalama +0,04. Rastgele
   işaretle gerçek genlik de aynı sonucu veriyor.
5. **Bu bilgi bit akışında yok.** Canlı bantlardaki deliklerin enerjisi Laplace modeliyle doğru kestiriliyor
   (+0,7 / +1,8 dB sapma). Bütün bandı sıfırlanmış granüllerde ise komşu granüllerden kestirim 11–18 dB fazla çıkıyor;
   o granülde enerji gerçekten düşük ve dosya bunu kanıtlamıyor (yalnızca |θ| < Δ_genel/2). Faz-tutarlı tonal doldurma
   küçük ama hep olumlu kazanç veriyor: tiz tonlar granüllerin çoğunda tamamen silinmiş.
6. **Yerel komşulardan doğrusal kestirim de yetmiyor.** Bir deliğin değerini komşu 5 granül × 7 frekanstaki çözülmüş
   değerlerden öğrenen doğrusal kestirici (12 frekans bölgesi, birini dışarıda bırakarak sınandı), deliklerin hatasını
   yalnızca +0,02 dB azaltıyor; MOS değişmiyor (`r2_dogrusal.py`). Delik değerleri komşu verilerde basitçe yakalanacak
   biçimde bulunmuyor; bunu ancak doğrusal olmayan, gerçek müzikten öğrenmiş bir üretici model yapabilir.

### 7.4 Karar

- F1'in kapısı geçilmedi: SNR ölçütü (≥ +1 dB) yalnızca piyanoda sağlandı; algısal ölçüde iyileşme yok denecek
  kadar küçük (+0,007 MOS). **Kutulara sadık klasik yöntemler ürüne alınmaz.** MP3 bit akışı ayrıştırıcısı (F0) da bu
  yüzden şimdilik yazılmaz: duyulur bir kazanç getirmeyen işi programa koymak, ses yolunu karmaşıklaştırır.
- Kâhin tavanı (+0,25 MOS) bilginin nerede eksik olduğunu gösteriyor: tizdeki deliklerin tutarlı ince yapısı. Bunu
  ancak gerçek müzikten öğrenmiş bir önsel tahmin edebilir; kutular da onun dosyayla çelişmesini engeller
  ("kutu bekçisi": herhangi bir iyileştiricinin çıktısı A ile katsayılara çevrilip kutulara kırpılırsa, dosyada kanıtı
  olan her şey korunur, uydurma yalnızca deliklerde ve |θ| < Δ/2 sınırında kalır).
- Öneri (proje sahibinin kararı):
  1. **Kör dinleme aracı (ABX):** ses mühendisiyle tartışmayı kulakla, kanıtla bitirir ("320 kbps ile FLAC'ı
     ayırabiliyor muyum?").
  2. **R2, öğrenmeli önsel araştırması:** kutularla sınırlanmış, delikleri dolduran küçük bir model.
     - Görev tanımı, devralacak model için kendi başına anlaşılır biçimde:
       [arastirma/veritas/R2_GOREV.md](arastirma/veritas/R2_GOREV.md).
     - Dayanağı: tutarlılık kümesine izdüşüm kesin ve ucuz, P_C(x) = S·clip(A·x, lo, hi). Çünkü SᵀS = c_t·I granül
       başına (uzun blokta 288, kısada 96; son granül hariç A·S = I). Her üretici yöntem adım başına bir analiz, bir
       kırpma ve bir sentezle dosyayla kesin tutarlı kılınabilir.
     - Eğitim verisi lisans sorunu olmadan üretilebilir: programla yazılmış parçalar ve MIT lisanslı ses bankası; ayrıca
       proje sahibinin kendi kayıpsız arşivi, yalnızca kendi bilgisayarında.
     - Risk: gerçek kayıtlara ve vokale genelleme burada ölçülemez.

## 8. R2: öğrenmeli önsel (15–16 Ekim 2026)

Görev tanımı: [arastirma/veritas/R2_GOREV.md](arastirma/veritas/R2_GOREV.md). Soru: yalnızca dosyanın kanıtını
(kutular) ve genel bir müzik önselini kullanarak delikleri (q = 0) dolduran bir kestirici, 64–96 kbps'te kulağa
gelen bir iyileşme getirebilir mi? Kapı **K1:** 64 kbps'te altı parçada ortalama MOS ≥ çözücü + 0,10, hiçbir parça
0,02'den fazla kötü değil; 96 kbps'te hiçbir parça kötüleşmiyor; tutarlılık %100.

Bütün sayılar `docs/arastirma/veritas/r2_*.py` ile tekrar üretilir (adımlar
[arastirma/veritas/README.md](arastirma/veritas/README.md)); düzenek, ölçüler ve altı deney parçası R1'in aynısıdır
(bölüm 7). Düzenek bu oturumda sıfırdan kuruldu ve R1'in tablosunu aynen verdi (çözücümüz − ffmpeg −122 dB; altı
parçanın çözücü ve kâhin MOS'ları bölüm 7.2 ile birebir). MOS bir modeldir (Zimtohrli), dinleme değildir.

**Kısa sonuç: K1 geçilmedi.** En iyi kâhinsiz sonuç 64 kbps'te ortalama +0,006 MOS (sonsal ortalama; çözücüden
ayırt edilemez), örnekleyen her yöntem ortalamada çözücüden kötü (akış −0,07 … bağlamsız −0,24). Nedeni bu bölümde sayılarla
gösteriliyor: deliklerin ince yapısı hakkında bit akışının ve bağlamın taşıdığı bilgi, kulağa gelen bir iyileşme için
gerekenin çok altında; komşu granüllerin **gerçek** değerleri bile bu bilgiyi artırmıyor.

### 8.1 Düzenek ve veri

- **Eğitim müziği (`r2_beste.py`):** 160 parça × 12 sn, tohumdan rastgele ton, mod, akor dizisi, tempo (66–170),
  2–5 parti (melodi, akor, bas, pad, arpej, karşı ses), 70 GM çalgısı, isteğe bağlı davul, yankı ve koro; FluidSynth +
  FluidR3_GM (MIT). Aynı üreticiden 16 **sınama** parçası (eğitimde yok). R1'in altı parçası eğitimde **hiç** yok.
  Hepsi yamalı LAME 3.100 ile 64 ve 96 kbps CBR; gerçek değer θ dökümden. Toplam 373 bin granül-kanal.
- **Model (`r2_model.py`):** frekans ekseninde genişletilmiş 1-B evrişim (12 artık blok, 64 kanal, 205 bin
  parametre). Girdi: hedef granül ve ±2 komşu granülün çözülmüş katsayıları (asinh(r/s)), adım logaritmaları, delik ve
  sıfır bant maskeleri, blok türü, M/S, örnekleme hızı, frekans konumu, öteki kanalın hedef granülü. Yalnız işlemci;
  her eğitim 4 çekirdekte 15–30 dk (1 dönemin altı: 4000 adım × 64 granül).
  - **Yoğunluk başı (S1):** her delik için u = θ/s ∈ [−½, ½] üzerinde 4 bileşenli kesik Gauss karışımı (s = Δ canlı
    bantta, Δ_genel sıfır bantta). Kutu içinde düzgün dağılımın yoğunluğu 1 olduğundan −log₂ p(u) doğrudan "düzgüne
    göre kazanılan bit"tir; aynı modelden bağımsız örnek, sonsal ortalama (MMSE) ve beklenen enerji E[θ²] alınır.
  - **Akış başı (S2):** akış eşleme (flow matching), granülün bütün deliklerini birlikte üretir; 32 Euler adımı, her
    adımdan sonra kutuya kırpma (P_C'nin katsayı alanındaki hâli).
  - Üç bağlam: **çözücü** (yalnızca çözücünün gördüğü), **bağlamsız** (komşu granül ve öteki kanal silinmiş: marjinalin
    öğrenilmiş hâli) ve **kâhin bağlam** (komşu granüllerin GERÇEK θ'sı; yalnızca tavan).
- **Ölçüler (`r2_degerlendir.py`):** R1'inkiler (SNR, Zimtohrli MOS, kutu dışı oranı; her yöntemde ölçüldü, hepsinde 0) + delik katsayısı başına bit + bant başına delik enerjisi kestirim hatası (dB) + süre.
- **Sıfır bant tavanı** (çözücünün bildiği bir olgu, kâhin değil): bütün q'ları sıfır bantlarda kesin sınır
  Δ_genel/2'dir, ama LAME genel kazancı bazen anlamsız bırakır: bir sınama parçasında bir S kanalı granülünde
  Δ_genel canlı bantların Δ'sının **65 536 katı** çıktı (gerçek |θ| ≈ 10⁻⁴). Kutu geçerli ama bilgisiz; modelin
  oraya yazdığı küçük ortalama (u ≈ 0,02) 4,7'lik bir katsayı, yani bir patlama oldu (tek granül, MOS 4,705 → 3,195).
  Kural: sıfır bantlara yazılan değer, o granülün canlı bantlarındaki en büyük Δ/2'yi aşamaz (sınama parçasında gerçek
  değerlerin %99,86'sı bu sınırın içinde; Δ_genel/maks canlı Δ oranı granüllerin %90'ında 1, %99'unda ≤ 8); canlı bandı
  olmayan granüle hiçbir şey yazılmaz. Kesin kutu değil, öngörüdür; bütün model doldurmalarına uygulandı.

### 8.2 Kâhin ayrıştırması: açık nerede? (`r2_tonal.py`)

64 kbps, MOS. Kâhinler gerçek değeri bilir; yalnızca açığın yerini gösterir (R1'in +0,25'lik tavanı: 4,628 → 4,875).

| Delikler gerçek değerle, yalnızca…         | pop   | piyano | akustik | elektronik | orkestra | vurmalı | Ort.      |
| ------------------------------------------ | ----- | ------ | ------- | ---------- | -------- | ------- | --------- |
| Standart çözücü                            | 4,349 | 4,722  | 4,604   | 4,763      | 4,504    | 4,825   | 4,628     |
| Tüm delikler                               | 4,872 | 4,867  | 4,876   | 4,883      | 4,857    | 4,894   | **4,875** |
| Canlı bant delikleri (SNR +1,2 … +1,9 dB)  | 4,579 | 4,837  | 4,742   | 4,846      | 4,639    | 4,882   | 4,754     |
| Sıfır bant delikleri (SNR +0,0 … +0,1 dB)  | 4,703 | 4,758  | 4,760   | 4,811      | 4,713    | 4,841   | 4,764     |
| Yalnız işaret doğru, genlik = bandın RMS'i | 4,790 | 4,797  | 4,805   | 4,848      | 4,767    | 4,871   | 4,813     |
| Gerçeğin tonal olmayan kalıntısı (12 dB)   | 4,836 | 4,780  | 4,859   | 4,836      | 4,834    | 4,871   | 4,836     |

- **Sıfır bantlar** (katsayıların %14–23'ü) açığın yarısını taşır ve **SNR'ı hiç değiştirmez**: enerjileri çok küçük,
  ama kulak yokluklarını duyar. Canlı bant delikleri SNR'ın tamamını, MOS'un yarısını verir.
- **İşaret tek başına açığın dörtte üçünü kapatır** (4,813): deliklerin genliği bandın RMS'iyle eşitlenip yalnızca
  işaret doğru yazılınca. R1'de aynı genlikle **rastgele** işaret 4,670 vermişti. Fark, ince yapının değeridir.
- **Tonal olmayan kalıntı açığın %84'ünü kapatır**; tonal kısım (8.3) küçük kalır.

### 8.3 S3: tonal deliklerin tavanı (`r2_tonal.py`)

Gerçek sinyal uzun STFT'de (8192) tepe seçmeyle tonal parçaya ayrılır (tepe belirginliği 6/12/20 dB, ±2 kutu); tonal
parça yalnızca deliklere, kutuya kırpılarak yazılır. Hiçbir tonal yöntem bu kâhinden ileri gidemez. "Tonal POCS",
R1'in faz-tutarlı doldurmasının yinelemeli, kâhinsiz hâlidir (tepe maskesi izdüşümü ↔ kutu izdüşümü P_C).

| Yöntem                                   | 64 kbps ort. MOS | parça aralığı | 96 kbps ort. MOS | parça aralığı |
| ---------------------------------------- | ---------------- | ------------- | ---------------- | ------------- |
| Standart çözücü                          | 4,628            |               | 4,797            |               |
| Kâhin tonal tavan, tepe 6 dB             | 4,730 (+0,10)    | +0,02 … +0,22 | 4,833 (+0,04)    | −0,00 … +0,07 |
| Kâhin tonal tavan, tepe 12 dB            | 4,656 (+0,03)    | +0,01 … +0,07 | 4,806 (+0,01)    | −0,00 … +0,03 |
| Kâhin tonal tavan, tepe 20 dB            | 4,633 (+0,01)    | −0,00 … +0,03 | 4,798 (+0,00)    | −0,00 … +0,01 |
| Kâhin tonal tavan, 12 dB, en çok 30 tepe | 4,629 (+0,00)    | −0,00 … +0,01 | 4,795 (−0,00)    | −0,01 … +0,00 |
| Tonal POCS ×1 (yalnız çözücü bilgisi)    | 4,633 (+0,01)    | −0,00 … +0,02 | 4,795 (−0,00)    | −0,01 … +0,01 |
| Tonal POCS ×8                            | 4,627 (−0,00)    | −0,04 … +0,04 | 4,790 (−0,01)    | −0,03 … +0,01 |
| Tonal POCS ×8, tepe 6 dB                 | 4,562 (−0,07)    | −0,12 … +0,01 | 4,733 (−0,06)    | −0,17 … +0,02 |

- Tonal tavan kâhinle bile **+0,03 (12 dB) … +0,10 (6 dB)**; 6 dB'lik eşik gürültüye yakın bileşenleri de "tonal"
  sayar. Belirgin tonların (20 dB) deliklerdeki payı MOS'ta sıfırdır: belirgin tonlar zaten q ≠ 0'dır.
- Kâhinsiz yineleme kazanmaz; agresif eşikte zarar verir. **Aralık gözlemli sinüzoidal izleme (EP/Kalman) için üst
  sınır +0,03 … +0,10 MOS; bunun kâhinsiz elde edilen kısmı +0,005.** Ayrıntılı izleyici yazılmadı: tavan onu
  gerektirmeyecek kadar alçak.

### 8.4 Bilgi–algı eğrisi: kaç bit gerekir?

Delik katsayısına gürültülü kâhin verilir: y = u + N(0, σ²), kutuya kırpılır; kazanılan bit −log₂ p(u | y) (kutuda
düzgün önselle kesik Gauss sonsal; modellerle aynı ölçü). İki ölçek: σ kutuya göre (α/2) ve gerçek delik RMS'ine
göre (σ = α·RMS; RMS'i bilmek kâhindir). 64 kbps, altı parça ortalaması:

| Ölçek | α    | bit/delik | MOS ort. | parça aralığı                  |
| ----- | ---- | --------- | -------- | ------------------------------ |
| kutu  | 0,10 | +2,3      | 3,878    | piyano 3,269 … vurmalı 4,733   |
| kutu  | 0,50 | +0,4      | 3,229    | orkestra 2,749 … vurmalı 4,022 |
| RMS   | 1,00 | +2,7      | 4,768    | piyano +0,02 … pop +0,36       |
| RMS   | 0,50 | +3,5      | 4,848    | piyano +0,11 … pop +0,48       |
| RMS   | 0,25 | +4,4      | 4,869    |                                |
| RMS   | 0,05 | +6,5      | 4,875    |                                |

- **Seviye yanlışsa bit sayısı kurtarmaz.** Kutu ölçeğinde 2,3 bit/delik (her deliğin değeri kutunun %5'i hatayla!)
  MOS'u 3,9'a düşürür: sıfır bantlarda kutu (Δ_genel/2) gerçek seviyenin 20–40 dB üstündedir; o ölçekte eklenen en
  küçük gürültü bile hışırtıdır.
- **Seviye doğruysa** gözlem SNR'ı 0 dB (α = 1; seviyeye ek ≈ 0,5 bit yapı) ortalama +0,14, 6 dB (α = 0,5; ≈ 1,2 bit
  yapı) +0,22 verir. **K1'in +0,10'u için gereken: doğru seviye ve delik başına ≥ 0,5 bit tutarlı yapı.**

### 8.5 S1: delikler hakkında bit akışı ve bağlam ne kadar bilgi taşıyor?

Delik katsayısı başına, kutu içinde düzgün dağılıma göre kazanılan bit (−log₂ p(u); yüksek iyi). R1'in altı parçası
(eğitimde yok), ortalama. "Marjinal h(u)" parçanın kendi deliklerinin histogramıdır (kâhin: yalnızca karşılaştırma).

| Model                                       | 64: tümü | canlı bant | sıfır bant | < 5 kHz | ≥ 5 kHz | 96: tümü | canlı | sıfır |
| ------------------------------------------- | -------- | ---------- | ---------- | ------- | ------- | -------- | ----- | ----- |
| Kutu içinde düzgün                          | 0        | 0          | 0          | 0       | 0       | 0        | 0     | 0     |
| Sıfırda Laplace (bant uyumu, R1 modeli)     | −0,38    | −0,48      | 0\*        | −0,24   | −0,44   | −0,31    | −0,37 | 0\*   |
| Marjinal h(u) (kâhin histogram)             | +1,26    | +0,57      | +5,23      | +0,31   | +1,87   | +1,14    | +0,39 | +6,27 |
| Öğrenilmiş, bağlamsız                       | +1,90    | +0,78      | +5,70      | +0,44   | +2,58   | +1,75    | +0,52 | +6,49 |
| Öğrenilmiş, çözücü bağlamı (±2 granül)      | +2,02    | +0,87      | +5,90      | +0,52   | +2,71   | +1,85    | +0,59 | +6,72 |
| Öğrenilmiş, **kâhin bağlam** (gerçek komşu) | +2,02    | +0,87      | +5,89      | +0,51   | +2,72   | +1,84    | +0,60 | +6,66 |

\* Bütün q'ları sıfır bantta Laplace uyumu yapılamaz; düzgün sayıldı.

- **Sıfırda Laplace düzgünden kötüdür** (−0,2 … −0,6 bit): kodlayıcının sıfırladığı katsayılar sıfıra yığılmaz,
  kutunun içine neredeyse düzgün yayılır. R1'de Laplace ağırlık merkezinin MOS'u düşürmesinin nedeni budur.
- **Bilginin çoğu seviyedir ve sıfır bantlardadır.** Bağlamsız modelin +1,9 bitinin kaynağı sıfır bantlar (+5,7:
  "bu bant Δ_genel/2'nin çok altında"); canlı bantlarda +0,8 bit, bunun da büyük kısmı marjinalin biçimi (+0,6).
- **Çözücü bağlamı yalnızca +0,1 bit ekler** (canlı bantta +0,09). Eğitim kaybı da aynı şeyi söyler: 2500 adımda
  bağlamsız −1,69, bağlamlı −1,63 bit.
- **Kâhin bağlam bir şey eklemez: komşu granüllerin gerçek değerleri bilinse bile +0,00 bit.** Delik katsayıları,
  bitişik granüllerin değerinden bağımsız denecek kadar yenidir. Bu, özbağlanımlı (önceki granülün örneğine koşullu)
  ya da çok granüllü birleşik bir üretici modelin de tavanıdır: komşuyu kusursuz bilmek yetmiyorsa, kusurlu örneğini
  bilmek hiç yetmez. R1'in 6. bulgusu (doğrusal komşu kestirimi +0,02 dB) doğrusal olmayan modelle de değişmedi.
  Sınır: modeller 1 dönemin altında eğitildi; daha uzun eğitim bu farkı büyütebilir ama 8.4'teki ≥ 0,5 bit yapı
  eşiğine 0,00'dan ulaşması beklenmez.
- **Seviye kestirimi:** canlı bantlarda model doğru (bant başına ortanca hata +0,1 … +0,4 dB; R1'in Laplace
  kestirimi +0,5 … +2,3 dB). Sıfır bantlarda parçaya göre çok değişken: pop/akustik/elektronik +0,8 … +2,6 dB,
  orkestra +6, piyano **+12 dB** (96 kbps'te +16). Sessiz tizli parçalarda ("piyano": pedal, sönüm) bit akışı
  seviyeyi taşımıyor; öğrenilmiş önsel eğitim müziğinin ortalamasını yazıyor.

### 8.6 S2: örnekleyiciler (`r2_model.py`, `r2_degerlendir.py`)

MOS; parantez içinde SNR farkı. Kutu dışı oranı her satırda 0 (ölçüldü). Sıfır bant tavanı kuralı uygulanmış.

| Yöntem (64 kbps)                           | pop          | piyano       | akustik      | elektronik   | orkestra     | vurmalı      | Ort.      |
| ------------------------------------------ | ------------ | ------------ | ------------ | ------------ | ------------ | ------------ | --------- |
| Standart çözücü                            | 4,349        | 4,722        | 4,604        | 4,763        | 4,504        | 4,825        | 4,628     |
| Bağlamsız model, örnek                     | 4,451 (−1,3) | 3,877 (−1,1) | 4,595 (−1,0) | 4,743 (−1,7) | 3,878 (−0,9) | 4,775 (−1,6) | 4,386     |
| Çözücü bağlamı, örnek                      | 4,448 (−1,3) | 4,238 (−1,1) | 4,637 (−1,1) | 4,748 (−1,4) | 4,142 (−1,0) | 4,791 (−1,2) | 4,501     |
| Çözücü bağlamı, örnek, yalnız canlı bant   | 4,348 (−1,2) | 4,642 (−1,1) | 4,589 (−1,0) | 4,743 (−1,3) | 4,418 (−0,9) | 4,792 (−1,2) | 4,589     |
| Çözücü bağlamı, **sonsal ortalama (MMSE)** | 4,367 (+0,0) | 4,721 (+0,0) | 4,605 (+0,0) | 4,764 (+0,0) | 4,521 (+0,0) | 4,825 (+0,0) | **4,634** |
| Kâhin bağlam, örnek                        | 4,458 (−1,3) | 4,182 (−1,1) | 4,634 (−1,1) | 4,746 (−1,4) | 4,142 (−1,0) | 4,792 (−1,3) | 4,493     |
| **Akış örnekleyici** (granül birlikte)     | 4,454 (−0,9) | 4,398 (−0,8) | 4,593 (−0,7) | 4,748 (−0,9) | 4,360 (−0,8) | 4,813 (−0,9) | 4,561     |
| Akış + bant enerjisi koruma                | 4,441 (−0,8) | 4,388 (−0,8) | 4,590 (−0,6) | 4,748 (−0,8) | 4,356 (−0,7) | 4,819 (−0,8) | 4,557     |
| Kâhin: delikler gerçek                     | 4,872 (+1,7) | 4,867 (+1,5) | 4,876 (+1,3) | 4,883 (+1,9) | 4,857 (+1,3) | 4,894 (+2,0) | 4,875     |

| Yöntem (96 kbps)                           | pop          | piyano       | akustik      | elektronik   | orkestra     | vurmalı      | Ort.      |
| ------------------------------------------ | ------------ | ------------ | ------------ | ------------ | ------------ | ------------ | --------- |
| Standart çözücü                            | 4,633        | 4,883        | 4,744        | 4,847        | 4,748        | 4,927        | 4,797     |
| Bağlamsız model, örnek                     | 4,571 (−1,3) | 3,901 (−0,8) | 4,753 (−0,8) | 4,826 (−1,5) | 3,839 (−0,7) | 4,908 (−1,2) | 4,467     |
| Çözücü bağlamı, örnek                      | 4,636 (−1,2) | 4,383 (−0,8) | 4,778 (−0,8) | 4,834 (−1,2) | 4,421 (−0,7) | 4,911 (−1,0) | 4,660     |
| Çözücü bağlamı, örnek, yalnız canlı bant   | 4,653 (−1,1) | 4,844 (−0,8) | 4,750 (−0,7) | 4,834 (−1,2) | 4,690 (−0,7) | 4,912 (−1,0) | 4,780     |
| Çözücü bağlamı, **sonsal ortalama (MMSE)** | 4,641 (+0,0) | 4,881 (+0,0) | 4,744 (+0,0) | 4,847 (+0,0) | 4,747 (+0,0) | 4,926 (+0,0) | **4,798** |
| Kâhin bağlam, örnek                        | 4,643 (−1,2) | 4,280 (−0,7) | 4,777 (−0,8) | 4,835 (−1,2) | 4,350 (−0,7) | 4,908 (−1,1) | 4,632     |
| **Akış örnekleyici** (granül birlikte)     | 4,701 (−0,9) | 4,693 (−0,6) | 4,773 (−0,5) | 4,841 (−0,9) | 4,570 (−0,6) | 4,918 (−0,8) | 4,749     |
| Akış + bant enerjisi koruma                | 4,687 (−0,7) | 4,689 (−0,6) | 4,771 (−0,5) | 4,841 (−0,8) | 4,561 (−0,5) | 4,922 (−0,7) | 4,745     |
| Kâhin: delikler gerçek                     | 4,881 (+1,4) | 4,924 (+0,9) | 4,894 (+0,9) | 4,914 (+1,7) | 4,880 (+0,9) | 4,940 (+1,5) | 4,906     |

- **Örnekleme ortalamada zarar verir** (64 kbps: bağlamsız −0,24, çözücü bağlamı −0,13, kâhin bağlam −0,13, akış
  −0,07). Zararın yeri sıfır bantlar: piyano ve orkestrada seviye 6–12 dB yüksek yazılınca tiz hışırtı. Seviyenin
  doğru olduğu üç parçada (pop, akustik, elektronik) örnekleme +0,10 / +0,03 / −0,02; pop'ta R1'in gerçek enerjiyle
  gürültüsüne (+0,20) yaklaşıyor.
- **Yalnız canlı bant deliklerini örneklemek** zararı sınırlar ama kazandırmaz (−0,04): canlı bantlarda seviye doğru,
  ince yapı yine rastgele; kulak rastgele yapıyı sıfırdan iyi bulmuyor (R1 bulgu 4).
- **Sonsal ortalama (MMSE)** her modelde çözücüyle aynı (+0,006 … +0,012): ortalama sıfıra yakın çıkıyor, çünkü
  model işareti bilmiyor. Bu, bölüm 8.2'nin tersten okunuşudur: işaret bilinmeyince en küçük hata kestirimi 0'dır.
- **Akış örnekleyici (granülün bütün delikleri birlikte)** örnekleyenlerin en iyisi: 64 kbps'te ortalama −0,07
  (pop +0,11, akustik −0,01, elektronik −0,02; piyano −0,32, orkestra −0,14), 96 kbps'te −0,05 (pop +0,07, piyano −0,19).
  Bağımsız örneklemeye göre piyanoda +0,16 daha iyi ve SNR kaybı yarı yarıya (−0,8 dB): granül içi birliktelik tiz
  hışırtıyı azaltıyor ama kaldırmıyor; seviye hatası aynı. Komşu granülle bağımsızlık sorunu kalır; 8.5'e göre onu
  çözecek bilgi de yok. Yalnızca pop +0,10'a ulaşıyor; K1'in "hiçbir parça 0,02'den kötü değil" koşulu 64'te de
  96'da da iki parçada (piyano, orkestra) bozuluyor.
- Süre (20 sn parça, tek iş parçacığı, dört ölçüm yan yana koşarken): yoğunluk modeli 7–25 s (şarkı süresinin altında);
  akış 32 Euler adımıyla 370–730 s, yani bütçenin (4 çekirdekte şarkı süresi) çok üstünde. Kapı geçilmediği için
  hızlandırılmadı (daha az adım, daha küçük ağ mümkün); bu da yöntemin lehine bir sonuç değil.

### 8.7 K2: ezber ve genelleme

64 kbps; aynı üreticiden 16 sınama parçası (eğitimde yok) ve eğitimden 4 parça; R1'in altı parçası başka bir
üreticiden (başka çalgılar, yapı, 20 sn).

| Küme (64 kbps)                                                       | Eğitim (4 parça)      | Sınama (16, aynı üretici) | R1 (6, başka üretici) |
| -------------------------------------------------------------------- | --------------------- | ------------------------- | --------------------- |
| Bit: bağlamsız / çözücü bağlamı / kâhin bağlam                       | +1,57 / +1,70 / +1,70 | +1,87 / +2,00 / +2,00     | +1,90 / +2,02 / +2,02 |
| Bit, canlı bant: bağlamsız / çözücü / kâhin                          | +0,59 / +0,67 / +0,68 | +0,78 / +0,88 / +0,89     | +0,78 / +0,87 / +0,87 |
| MOS çözücü                                                           | 4,638                 | 4,614                     | 4,628                 |
| Çözücü bağlamı, örnek: fark ort. [en kötü, en iyi]                   | −0,05 [−0,08, −0,03]  | −0,03 [−0,23, +0,23]      | −0,13 [−0,48, +0,10]  |
| Çözücü bağlamı, MMSE: fark ort. [en kötü, en iyi]                    | +0,00 [−0,00, +0,01]  | +0,00 [−0,01, +0,01]      | +0,01 [+0,00, +0,02]  |
| Akış örnekleyici: fark ort. [en kötü, en iyi]                        | −0,03 [−0,10, +0,02]  | −0,02 [−0,13, +0,16]      | −0,07 [−0,32, +0,11]  |
| Sıfır bant enerji hatası, çözücü bağlamı (ortanca dB; ort., en kötü) | +0,8, +1,5            | +2,7, +6,0                | +4,4, +12,0           |
| Kâhin: delikler gerçek, fark ort.                                    | +0,24                 | +0,26                     | +0,25                 |

- Bit kazancı sınama parçalarında eğitimdekilerden **düşük değil** (ezber yok); R1 parçaları da aynı aralıkta.
  Kâhin bağlam üç kümede de çözücü bağlamıyla aynı: komşu granülün gerçek değeri hiçbir kümede bilgi eklemiyor.
- Örnekleme sınama parçalarında da ortalamada zarar verir (−0,03; akış −0,02): çözücünün kötü olduğu yoğun parçalarda
  (MOS 4,3–4,4) +0,12 … +0,23, seyrek ve tonal parçalarda −0,1 … −0,23. Yani kazanç "gürültülü içerikte gürültü
  doldurma"dır; tonal içerikte ince yapı eksikliği cezalandırılır. Sıfır bant tavanı kuralından önce bir parçada
  (0510) tek granül −1,5 MOS'a mal olmuştu; kuralla o parçada MMSE 4,704 = çözücü, örnek −0,06, akış −0,01.
- **Gerçek kayıtlara genelleme ölçülmedi:** eğitim ve sınama aynı ses bankasından; vokal, gerçek oda, analog gürültü,
  başka kodlayıcılar (FhG, eski LAME) yok. Bu bir sınırdır. Sonuç olumsuz olduğu için bu sınır kararı değiştirmez:
  sentetik müzikte bile bilgi yoksa gerçek müzikte de yoktur; gerçek müzikte eksik olan, daha iyi bir önselin
  bulabileceği türden bir düzenlilik değil, dosyanın taşımadığı ince yapıdır.

### 8.8 Karar ve öneri

- **K1 geçilmedi; "biraz daha deneyelim" yok (görev kuralı 4).** Ürün tasarım notu yazılmadı (yalnızca K1 geçilirse
  istenmişti). Model ağırlıkları depoya konmadı. Programa kod eklenmedi.
- Neden, tek cümleyle: 64 kbps'te algısal açığın (+0,25) kaynağı tizdeki deliklerin **ince yapısı** (işaret/faz; 8.2)
  ve sıfır bantların **seviyesi**; bit akışı ikisini de taşımıyor (8.5: bağlam +0,1 bit, gerçek komşu +0,0 bit; sıfır
  bant seviyesi parçaya göre 12 dB yanlış), kulağa gelen kazanç için ise doğru seviye ve ≥ 0,5 bit yapı gerekiyor (8.4).
  Öğrenilmiş genel bir müzik önseli bu bilgiyi yerine koyamıyor: koyduğunda (örnekleme) ortalama MOS düşüyor,
  koymadığında (MMSE) çözücüyle aynı kalıyor.
- Üç yolda da tavan ölçüldü: klasik (R1: +0,007), tonal izleme (S3 kâhin: +0,03 … +0,10, kâhinsiz +0,005), öğrenmeli
  (R2: +0,006). **Veritas'ın MP3 kolu burada kapanır;** yeniden açılması için yeni bir bilgi kaynağı gerekir (aynı
  şarkının ikinci bir kopyası, bölüm 2.4; ya da kayıpsız bir referans), genel bir önsel değil.
- Öneri (proje sahibinin kararı):
  1. **Kör dinleme aracı (ABX)** artık programda (0.0.40, çekmecede **Kör test**): ses mühendisiyle tartışmayı kulakla
     bitirir ve bu belgenin "MOS modeldir" sınırını da kapatır. İsteyen, bu araştırmanın örneklerini de onunla
     dinleyebilir (`r2_degerlendir.py` çıktıları WAV olarak yazdırılabilir; bu sürümde yazdırılmadı).
  2. Yol haritasındaki Retriever maddesi "araştırıldı, kapıdan geçmedi" diye işaretlenir; AAC (F4) için aynı deney
     tekrarlanmadan karar verilmez ama MP3 sonucu beklentiyi düşürür.
  3. Proje sahibinin kendi kayıpsız arşiviyle tek bir doğrulama (kendi bilgisayarında): bu belgenin sentetik müzik
     sınırını kapatır. Beklenti: aynı sonuç.

## 9. Tekrar üretme

Adımlar ve döküm biçimi: [arastirma/veritas/README.md](arastirma/veritas/README.md). LAME yalnızca ölçüm için,
depo dışında yamalı derlenir; programa girmez.

## Kaynaklar

- Villasana T., Villemoes, Klejsa, Hedelin, "Audio decoding by inverse problem solving", arXiv 2409.07858 (ICASSP 2025)
- Blau, Michaeli, "The perception-distortion tradeoff", CVPR 2018, arXiv 1711.06077
- Lipman, Chen, Ben-Hamu, Nickel, Le, "Flow matching for generative modeling", ICLR 2023, arXiv 2210.02747 (R2 akış başı)
- Chung, Kim, Mccann, Klasky, Ye, "Diffusion posterior sampling for general noisy inverse problems", ICLR 2023,
  arXiv 2209.14687 (R2'de anılan DPS seçeneği; denenmedi, 8.5'e göre gerek kalmadı)
- Záviška, Rajmic, Mokrý, "Audio dequantization using (co)sparse (non)convex methods", ICASSP 2021, arXiv 2010.16386;
  kod: github.com/zawi01/audio_dequantization (GPL-3.0)
- Mokrý, Rajmic, Záviška, "Flexible framework for audio reconstruction", DAFx 2020, arXiv 2004.11162
- Kovanda, Rajmic, "Audio dequantization using instantaneous frequency", arXiv 2510.16813
- Brauer, Gerkmann, Lorenz, "Sparse reconstruction of quantized speech signals", ICASSP 2016
- Lapierre, Lefebvre, "Pre-echo noise reduction in frequency-domain audio codecs", ICASSP 2017
- Gupta ve ark. (Fraunhofer IIS), MDCT alanında son süzgeç (LC3), arXiv 2201.12039 (ICASSP 2022)
- Kai Li, Yi Luo, Apollo, arXiv 2409.08514 (ICASSP 2025); github.com/JusperLee/Apollo
- Lattner, Nistal, "Stochastic restoration of heavily compressed musical audio using GANs", Electronics 10(11), 2021,
  arXiv 2207.01667
- Haohe Liu, AudioSR, arXiv 2309.07314; Moliner, Välimäki, BABE, arXiv 2306.01433; AEROMamba, arXiv 2411.07364
- Porov ve ark. (Samsung), "Music enhancement by a novel CNN architecture", AES 145, 2018 [ayrıntılar doğrulanmadı]
- Zakhor, IEEE TCSVT 1992; Yang, Galatsanos, Katsaggelos, IEEE TCSVT 1993; Bredies, Holler, SIAM J. Imaging Sci. 2012;
  Ehrlich ve ark., "Quantization guided JPEG artifact correction", ECCV 2020
- Thao, Vetterli, IEEE TSP 1994; Goyal, Vetterli, Thao, IEEE Trans. Inf. Theory 1998
- Hydrogenaudio LAME wiki ve halka açık dinleme testleri (128 kbps MP3, 2008; çok biçimli, 2014); Pras ve ark., AES 127,
  2009, bildiri 7879
- ISO/IEC 11172-3 (MPEG-1 Layer III); LAME 3.100 kaynak kodu (`quantize_pvt.c`: `adj43`; `takehiro.c`:
  `best_scalefac_store`)
