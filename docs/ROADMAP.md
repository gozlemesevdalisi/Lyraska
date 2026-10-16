# Lyraska — Yol haritası

Görselleri şarkıyı önceden bilen, tamamen çevrimdışı çalışan Windows müzik çalar.
Lansman hedefi: **Mart 2027 başı (v1.0)**.

## Kalite hedefleri

| Hedef                  | Ölçüt                            |
| ---------------------- | -------------------------------- |
| Beat tespiti doğruluğu | F-ölçüsü en az **0,80**          |
| Ses–görüntü senkronu   | **±20 ms**                       |
| Görsel akıcılık        | 1080p, dahili GPU'da **60 fps**  |
| Kararlılık             | 1 saat çalmada **sıfır takılma** |

## Faz 0 — Temel (5–11 Ekim 2026)

- [x] Tauri 2 + React + TypeScript iskeleti; Rust tarafı ses motoru, analiz ve görsel köprüsü modüllerine ayrılmış
- [x] Karşılama ekranı (eski araba teybi ekranı havasında, özgün tasarım)
- [x] CI: her push ve PR'da Rust + TypeScript testleri, lint, lisans denetimi
- [x] Windows kurulum dosyası ve sabit "test-surumu" indirme linki
- [x] CLAUDE.md, README, Türkçe hata kaydı formu

## Faz 1 — Çalan program, v0.1 (12 Ekim – 8 Kasım 2026) — tamamlandı

Proje sahibi, 16 Ekim 2026: "Faz 1 tamam."

- [x] Ses motoru: symphonia ile çözme, 64-bit iç işlem, kilitsiz halka tampon, WASAPI çıkışı
- [x] Dosya aç, çal/duraklat (yumuşak geçişli), durdur; sürükle-bırak; konum ve takılma sayacı
- [x] Sarma (ilerleme çubuğu, ←/→); önceden analiz edilen gerçek spektrum (60 kare/sn, 32 bant)
- [x] Müzik kütüphanesi (SQLite): klasör tarama, etiketler, Türkçe duyarlı arama; çalma sırası, önceki/sonraki, otomatik geçiş
      (0.0.35: 0.0.5–0.0.25'in kütüphane dosyaları 0.0.28'den beri açılmıyordu, düzeltildi; analiz önbelleği bozuksa
      kütüphane yine açılır, açılamazsa ekranda uyarı çıkar)
- [x] 10 bantlı ekolayzer (sürgüler gerçekten duyulan eğri; bozulma koruması; hazır ayarlar; ayarlar kalıcı)
- [x] Şarkılar arası boşluksuz geçiş (AAC kodlayıcı dolgusu dahil); yerel hata/çökme günlüğü ve "Hata günlüğü" düğmesi
- [x] 3 sahne: nokta matris spektrum, VU ibreleri, "Gece göğü" (WebGL2 gölgelendirici: kuzey ışıkları ve Lyra takımyıldızı).
      0.0.22: sahneler CI'da Windows Edge ile denetlenir; WebGL2 açılamazsa neden hata günlüğüne yazılır ve gece
      göğü 2D yedek çizimle (Lyra dahil) görünür. Proje sahibinde ışıklar görünüyor (#24 kapandı)
- [x] Kuzey ışıklarını göz kamaştırıcı ve estetik yapmak (proje sahibinin isteği; parlamasız, epilepsi sınırı
      içinde) ([#30](https://github.com/gozlemesevdalisi/Lyraska/issues/30)). 0.0.30: perdeler yeniden çizildi
      (katlanma, ince ışınlar, alttan yeşil üstte mor, dört derinlik, Vega'nın ışık çizgileri); üç manzara
      (göl, korona, karlı vadi) Ayarlar'da. Proje sahibi onayladı (16 Ekim; #30 kapandı)
- [x] Gerçek albüm kapakları (0.0.32): şarkının içindeki kapak büyük başlığın yanında ve sıradaki şarkı kartında;
      yoksa albüm adından özgün renk kapağı. Arayüzün vurgu renkleri çalan bölümün temasına yumuşakça geçer.
      Her panel ve sahne bir hata sınırı içinde: hata yalnızca o bölümü kapatır
- [x] Sürükle-bırak sorunu ([#29](https://github.com/gozlemesevdalisi/Lyraska/issues/29)). 0.0.23: bırakılan şarkılar
      kütüphaneye eklenir ve çalar, sürüklerken pencerede vurgu, desteklenmeyen türde anlaşılır mesaj. Proje sahibi
      onayladı (16 Ekim; #29 kapandı)
- [x] Yalnızca ID3v1 etiketli MP3'lerde başlık/sanatçı okunması
      ([#27](https://github.com/gozlemesevdalisi/Lyraska/issues/27)). 0.0.33: Türkçe harfler (Windows-1254) ve
      UTF-8 doğru okunur; kütüphanedeki etkilenmiş kayıtlar bir sonraki taramada kendiliğinden düzelir.
      Faz 1 ile onaylandı (16 Ekim)

## Faz 2 — Görsel Yönetmen, v0.2 (9 Kasım – 20 Aralık 2026)

- [x] İşaretleme aracı: şarkı çalarken beat (Boşluk) ve drop (D) işaretleme, doğruluk ölçümü (F-ölçüsü)
- [x] Şarkı haritası analizi: beat (tempo ve vuruşlar), ölçü başı (4/4–3/4), bölümler, drop, enerji — gerçek
      şarkılarda doğruluk işaretleme aracıyla ölçülecek. 0.0.36: her drop'un güveni (bas dönüşü, enerji artışı,
      bölüm sınırı, sürme); drop sayacı yalnızca güveni yüksek droplar için. 0.0.39: aynı kural her yerde (şeritteki
      DROP, işaretleme, Görsel Yönetmen'in beklentisi ve açılımı)
- [x] Şarkı haritası önbelleği (0.0.20): analiz sonuçları (spektrum, beat, ölçü başı, bölümler, drop, enerji)
      SQLite'ta; anahtar yol + boyut + değiştirilme zamanı + analiz sürümü (algoritma değişince kendiliğinden yeniden
      hesaplanır). Kütüphane arka planda, düşük öncelikle analiz edilir (önce çalan, sonra sıradaki, sonra geri
      kalan); kütüphane listesinde BPM sütunu. 0.0.34: analiz sürümü dört parçaya ayrıldı (spektrum, ritim, ses
      yüksekliği, bas tepeleri); bir parça değişince yalnızca o yeniden hesaplanır, ritim şarkı çözülmeden
      karelerden (4 dakikalık şarkıda ~50 ms; tam analiz ~2,5 sn)
- [x] Gecikme telafisi ve kalibrasyon (0.0.21): verinin ekrana ulaşma süresi her karede ölçülüp telafi edilir; ses
      aygıtının ek gecikmesi (ör. Bluetooth) "Senkron" sekmesinde ayarlanır ya da tıklama kaydıyla ölçülür; vuruş
      göstergesiyle gözle denetlenir. Gerçek aygıtlarda ±20 ms hedefinin ölçümü proje sahibinin denemesiyle
- [x] Koreografi: atmosfer, ritim ve doku katmanları — Görsel Yönetmen çekirdeği hazır (0.0.17); gece göğü
      bağlandı (0.0.18: bölüm teması, drop öncesi gerilim, drop açılımı, ölçü başı dalgası); gece otoyolu
      bağlandı (0.0.19). 0.0.41: VU ve spektrum sahneleri "sahne ışığı"yla bağlandı (hale bölüm renginde, ruh
      hâliyle güçlenir, droptan önce daralır, drop'ta açılır; spektrumda tepe noktaları droptan önce asılı
      kalır); doku katmanı bütün sahnelerde (vuruş yoğunluğu akışı, tizlerin canlılığı kırpışmayı ve tepe
      noktalarını hızlandırır)
- [x] "Gece otoyolu" sahnesi (0.0.19): şerit çizgileri vuruşlara, sokak lambaları ölçü başlarına kilitli; ufuk
      parıltısı bölüm temasıyla renklenir; drop öncesi ufka çekilir, drop'ta yükselir ve yol iki kat hızlanır
- [x] Epilepsi güvenli modu (0.0.18): her zaman saniyede en fazla 3 parlama (testle sınanır); güvenli modda
      saniyede en fazla 1 nabız ve yarı hızda parlaklık değişimi. Ayar kaydedilir (Ayarlar paneli), açıkken
      ekranda "Güvenli mod" yazar
- [x] Yeni arayüz "Sahne" (0.0.25): sahne tüm pencereyi kaplar, yazı ve düğmeler cam katmanlarda; büyük şarkı
      adı; şarkı haritası şeridi (bölümler gökyüzünün o bölümdeki renginde, enerji, drop'lar); drop geri sayımı;
      sıradaki şarkı; kütüphane ve paneller sağdan açılan çekmecede; fare durunca sinema görünümü; 1–5 tuşları
      sahne seçer; Inter yazı tipi (OFL-1.1)
- [x] **Ton tespiti (Camelot)** (0.0.42): şarkının tonu (ör. "La minör") ve Camelot kodu (ör. 8A), güven
      değeriyle. Kütüphanede "Ton" sütunu, çalarken bilgi kartında (ton değişen bölümde o bölümün tonu, vurgulu);
      işaretleme aracında ton seçimi ve "Doğruluğu ölç"te karşılaştırma. Akıllı Geçiş (Faz 6) ve Otomatik DJ
      (Faz 7) ton uyumunu buradan alır. Gerçek şarkılarda isabet proje sahibinin işaretleriyle ölçülecek.
  - Yöntem: kroma (12 perde sınıfı; tepe frekansları parabolle inceltilir, akort kayması bulunur) + Krumhansl–
    Kessler ton profilleriyle ilinti; şarkının tonu 15 sn'lik pencerelerin oylamasıyla. Yapay zekâ modeli
    gerekirse önce kod ve ağırlık lisansı doğrulanır.
  - Kroma, saklanan 32 bant kareden çıkmaz (perde çözünürlüğü yetmez): yeni bir analiz parçası olur, kendi sürümüyle
    (`analysis::cache::VERSIONS`). Eklenince kütüphane bir kez çözülür; spektrum ve ritim yeniden hesaplanmaz.
  - Güven düşükse (atonal, perküsyon, ton değiştiren şarkı) ton gösterilmez; şarkı içinde ton değişiyorsa bölüm
    bölüm.
  - Kalite hedefi: işaretli şarkılarda ton isabeti (aynı ton ya da Camelot komşusu) ≥ %80; işaretleme aracına ton
    girişi eklenir.
- [x] **Aksak ölçü desteği** (0.0.43): 5/8, 7/8, 9/8 ve Türk müziğinin aksak usulleri (ör. 9/8 = 2+2+2+3, 7/8 =
      2+2+3); bilgi kartında "9/8 (2+2+2+3)". Gerçek aksak şarkılarda isabet proje sahibinin işaretleriyle
      ölçülecek.
  - Ölçü sekizlik ızgarasında puanlanır: düzenli (2+2+2+2, 2+2+2, 3+3) ve aksak gruplamaların (dönüşleriyle) her
    başlangıç konumu vurguyla (başlangıç gücü + bas vuruşu) ilintilenir. Aksak açıkça daha uygunsa şarkının
    vuruşları grup başları olur (9/8'de ölçüde 4 uzun-kısa vuruş), ölçü başları ızgaradan gelir.
  - **Güven düşükse ölçü vurgusu kapanır:** görseller ölçü başını vurgulamaz (lamba, dalga), yalnızca vuruşa tepki
    verir; bilgi kartında ölçü yazmaz. Yanlış ölçü vurgusu, hiç vurgu olmamasından kötüdür. Düzenli ölçüde güven,
    8 ölçülük pencerelerde aynı ölçü başı konumunun kazanma payından.
  - Yalnızca ritim parçasının sürümü artar: kütüphane çözülmeden yenilenir.
  - Kalite hedefi: işaretli aksak şarkılarda ölçü başı isabeti; test verisi sentetik 9/8 ve 7/8 davul kayıtları.
- [x] "Gece sürüşü" nostalji modu (0.0.44; proje sahibi onayladı): ön camdan gece otoyolu (aynı çizim), torpidoda
      özgün tasarım teyp: makaraları dönen kaset, kayan şarkı adı ve spektrumlu nokta matris ekran, ışık halkalı
      düğmeler, arkadan aydınlatmalı VU ibreleri. Işıklar Görsel Yönetmen'in sahne ışığından; sinema görünümünde
      tam belirir. Hiçbir markanın adı, logosu ya da yazı tipi kullanılmaz. Proje sahibinin görüşü bekleniyor

## Faz 3 — Profesyonel ses, v0.3 (21 Aralık 2026 – 17 Ocak 2027)

- [x] Yüksek kaliteli yeniden örnekleme (aygıtın hızına, Windows'a bırakmadan), taşma koruması, sinyal yolu göstergesi (öne alındı: 0.0.12).
      "Müzikler garip geliyor" bildiriminin proje sahibince doğrulanması bekliyor
      ([#25](https://github.com/gozlemesevdalisi/Lyraska/issues/25))
- [x] Parametrik EQ (0.0.49; motor ve elle ayar arayüzü).
      Ekolayzerle ilgili kalan işler tek yerde: [#26](https://github.com/gozlemesevdalisi/Lyraska/issues/26)
  - [x] Süzgeç çekirdeği (0.0.49; proje sahibi, 16 Ekim 2026: "en üst düzey mühendislik, en üst düzey matematik"):
        sekiz tür (tepe, raflar, alçak/yüksek/bant geçiren, çentik, tüm geçiren); RBJ'nin yanında analog eğriye
        eşlenmiş tasarım (44,1 kHz'te ortalama sapma tepe süzgecinde 0,70 → 0,16 dB, alçak geçirende 3,97 → 0,16 dB;
        hiçbir ayarda RBJ'den uzak değil). Kulaklık profilleri Equalizer APO'nun kaynak kodundaki anlamlarla okunur
        (LS/HS köşe frekansı, Q'suz rafta S = 0,9, eğim, BW Oct, LP/HP/BP/NO/AP)
  - [x] Elle ayar arayüzü (0.0.49): Ekolayzer panelinde "Parametrik" görünümü; grafikte sürüklenen bantlar (Shift
        ince ayar, tekerlek Q, çift tıklama bant ekler), klavye, sayısal kutular, sekiz tür, bant aç/kapa, Equalizer
        APO dosyası yükleme/kaydetme, eşit ses yüksekliğinde A/B karşılaştırma, tasarım seçimi (analog eğriye sadık /
        Equalizer APO ile aynı)
- [x] AutoEq kulaklık profilleri: ParametricEQ.txt içe aktarma (öne alındı: 0.0.13). Hazır profil listesi, ölçüm
      kaynaklarının lisansı doğrulanınca
- [x] EBU R128 loudness (0.0.28, öne alındı; proje sahibi, 8 Ekim 2026: "bas dedin mi gerçekten hissedilsin"):
      şarkılar −14 LUFS'e getirilir (varsayılan açık, yükseltme gerçek tepeyi −1 dBTP'de tutar); ekolayzer sesi
      kısmadan yükseltebilir (eşitlemenin açtığı boşluk kullanılır); yeni hazır ayarlar "Bas", "Derin bas",
      "Küçük hoparlör". Bas raf filtresi gerekmedi: sürgüler zaten duyulan eğri, sorun ön kazançtaydı
- [x] Bas motoru (0.0.29; proje sahibi: "bas dedin mi gerçekten o bas hissedilsin", "bas deli dehşet olsun"):
      ekolayzerin üstünde büyük "Bas" düğmesi (100 Hz raf, 0–18 dB; 12 üstü kulüp düzeyi), "Derinlik" (bas
      notalarının bir oktav altı), küçük hoparlör bası (dizüstünde çalınamayan alt bas yerine harmonikleri) ve
      şarkının ölçülen bas tepelerine göre akıllı taşma koruması (bas yükselince ses gereksiz kısılmaz).
      Hazır ayarlar: Bas, Derin bas, Kulüp, Küçük hoparlör. 0.0.31: "Vuruş" (proje sahibinin seçimi: davul
      vuruşunun ilk anı 8 dB'ye kadar güçlenir, bas şişmez)
- [ ] Crossfeed — çekirdek hazır (0.0.49, `audio/crossfeed.rs`, testli): orta ses bit bit aynı, yan kanal hiçbir
      frekansta yükselmez, kulaklar arası seviye ve zaman farkı hedefte. Kalan: ses yoluna bağlama (bastan sonra,
      eşitlemeden önce; bit-perfect'te kapalı), kalıcı ayar, komutlar, arayüz (Ekolayzer panelinde kulaklık bölümü)
- [ ] Retriever: sıkıştırılmış (MP3 vb.) sesi iyileştirme — çalışma adı; ürün içi adı özgün olacak
  - Araştırma ve ölçüm: [VERITAS.md](VERITAS.md) (14 Ekim 2026). MP3 her katsayı için gerçek değerin hangi aralıkta
    (kutu) olduğunu kesin olarak taşır; gerçek LAME 3.100 ile doğru kutu kuralı (adj43 + bant kuralı) kapsama %100.
  - R1 sonucu (gerçek dalga biçimi, algısal ölçü): kutulara sadık klasik yöntemler SNR'ı 0,2–1,3 dB artırıyor ama
    duyulur bir iyileşme vermiyor; algısal kayıp tizdeki deliklerde ve onları dolduracak bilgi bit akışında yok.
    Klasik yol kapıdan geçmedi, ürüne alınmaz.
  - [x] Kör dinleme aracı (ABX, 0.0.40, proje sahibinin kararı): çekmecede **Kör test**. Aynı şarkının iki sürümü
        örnek düzeyinde hizalanır, ses yükseklikleri eşitlenir, işlenmeden çalınır; A/B/X aynı konumda tıksız
        değişir, sonuç binom sınamasıyla en sonda verilir.
  - R2 sonucu (15–16 Ekim 2026, öğrenmeli önsel; VERITAS.md bölüm 8): kutularla sınırlanmış öğrenilmiş model de
    kapıdan geçmedi (en iyi +0,006 MOS; örnekleyen yöntemler ortalamada çözücüden kötü). Nedeni ölçüldü: deliklerin
    ince yapısı hakkında bit akışı ve bağlam ≈ 0,1 bit taşıyor, komşu granüllerin gerçek değeri bile bilgi eklemiyor;
    duyulur kazanç için doğru seviye ve ≥ 0,5 bit yapı gerekiyor. **MP3 kolu kapandı**; yeniden açılması için yeni bir
    bilgi kaynağı gerekir (aynı şarkının ikinci kopyası ya da kayıpsız referans). AAC için ayrı karar.
- [x] Bit-perfect mod (0.0.28, öne alındı): WASAPI özel mod, ayar olarak, varsayılan kapalı (proje sahibinin
      kararı). Şarkı kendi hızında, aygıtın en yüksek tamsayı biçiminde (32 → 24 → 16 bit) çalar; örneklerin bit bit
      aynı çıktığı uçtan uca testli. Aygıt desteklemezse ya da başka program tutuyorsa normal yola geçilir, nedeni
      yazılır
- [x] Ses kalitesi ölçülebilir testlerde (0.0.28): bütün ses yolu (kulaklık düzeltmesi + ekolayzer + eşitleme +
      çıkış) en fazla −140 dB bozulma ve gürültü ekler (ölçülen −152 dB); yeniden örneklemede Nyquist üstü tonlar
      ≥ 140 dB bastırılır (ölçülen −195 dB)

## Sürekli: mimari güvenceler

- [x] (0.0.26) Rust ↔ arayüz veri tipleri üretiliyor (ts-rs) ve CI'da denetleniyor; komut sözleşmesi denetimi;
      ses yolunda bellek ayırma testi; Clippy ile unwrap/expect/panic yasağı; veritabanı göçleri (`storage.rs`);
      ana ekran hook'lara ayrıldı; `docs/MIMARI.md`

## Faz 4 — Görsel kütüphane ve v1.0 (18 Ocak – 28 Şubat 2027; lansman Mart 2027 başı)

- [ ] 15+ özgün sahne
- [ ] Butterchurn / MilkDrop desteği
- [ ] Kinetik şarkı sözleri
- [ ] Preset sistemi
- [ ] Kod imzalama (Windows "bilinmeyen yayımcı" uyarısının kalkması)
- [ ] **Açık şarkı haritası formatı** (plan, 9 Ekim 2026): Lyraska'nın şarkı haritası (vuruşlar, ölçü başları,
      bölümler, droplar ve güvenleri, enerji, ton) belgelenmiş, sürümlü bir JSON biçiminde dışa ve içe aktarılır.
  - **Ses ve şarkı sözü içermez** (telif): yalnızca zamanlar ve sayılar. İşaretleme dosyaları gibi.
  - **Ses parmak iziyle eşleşir:** dosya adı ya da etiket değil, sesin kendisinden çıkarılan kısa bir parmak izi.
    Aynı kaydın farklı biçimleri (MP3/FLAC) ve farklı adları eşleşir; farklı kayıtlar (canlı, remaster) eşleşmez.
    Parmak izi programda, çevrimdışı hesaplanır. Yöntem eklenmeden önce lisansı doğrulanır (Chromaprint
    LGPL-2.1; başka bir uygulama da olabilir).
  - **Topluluk paylaşır:** insanların düzelttiği haritalar (ör. işaretleme aracıyla) dosya olarak paylaşılır; içe
    aktarılan harita programın analizinin önüne geçer. İsteğe bağlı bir topluluk deposu ileride düşünülebilir;
    program **hiçbir zaman internete bağlanmak zorunda değildir**, telemetri yoktur. Paylaşılan harita ses
    içermediği için kayıt sahibinin hakkına dokunmaz.
  - Biçim belgesi (`docs/`) ve şema sürümü; eski sürümün dosyaları okunur. Kötü niyetli dosyaya karşı sınırlar
    (boyut, değer aralıkları) ve testler.

## Faz 5 — Büyüme (v1.0 sonrası)

- [ ] Sistem sesi modu (bilgisayarda çalan her sesi görselleştirme)
- [ ] Video dışa aktarma
- [ ] macOS ve Linux sürümleri
- [ ] Oda ışıkları entegrasyonu
- [ ] Yapay zekâ destekli analiz

## Faz 6 — Akıllı Geçiş (v1.0 sonrası)

Şarkıları ve sırayı kullanıcı seçer; Lyraska yalnızca aralarındaki geçişi yapar.

- [ ] İki desteli çalma: iki şarkı ayrı çözülür, çıkışta karıştırılır; tempo eşitleme ses iş parçacığında değil
      çözme tarafında yapılır; boşluksuz çalma ayrı mod olarak bozulmadan kalır
- [ ] Geçiş planlayıcı: giden şarkının son cümlesi ile gelenin ilk cümlesi (8/16/32 ölçü) hizalanır; geçiş
      uzunluğu ölçü cinsinden
- [ ] Ses yüksekliği eşitleme (EBU R128) ve ton uyumu uyarısı (engellemez)
- [ ] Güvenlik kuralı: tempo farkı varsayılan olarak %8'den büyükse, tempo şarkı içinde değişiyorsa ya da beat
      güveni düşükse hizalama yapılmaz, kısa düz geçiş kullanılır
- [ ] Perdeyi bozmadan tempo eşitleme (aday: Signalsmith Stretch, MIT; Rubber Band GPL olduğu için ikinci
      seçenek; lisans eklemeden önce doğrulanır)
- [ ] Geçiş stilleri: crossfade, bas takası, yankılı çıkış
- [ ] Kullanıcı kontrolü: şarkı çifti başına geçiş noktası, stil ve uzunluk ayarı; "geçişi dinle" önizlemesi
- [ ] Görsel Yönetmen geçişi de koreografe eder
- Kalite hedefi: üst üste binme boyunca iki şarkının vuruşları arasındaki fark en fazla 10 ms; şüphede düz
  geçişe düşülür

## Faz 7 — Otomatik DJ (v1.0 sonrası; plan, 9 Ekim 2026)

Faz 6'da sırayı kullanıcı seçer; burada **sırayı program seçer**, geçişi Akıllı Geçiş yapar.

- [ ] Kullanıcı bir başlangıç (şarkı, albüm, çalma listesi ya da kütüphanenin bir kısmı) ve isteğe bağlı süre ya da
      ruh hâli seçer; program sıradaki şarkıyı seçmeye başlar.
- [ ] Seçim ölçütleri, şarkı haritasından (çevrimdışı):
  - **BPM:** sıradaki şarkının temposu yakın (varsayılan en fazla %6 fark; yarım/çift tempo da sayılır).
  - **Ton:** Camelot tekerleğinde aynı, komşu ya da paralel ton (Faz 2 ton tespiti; güveni düşük ton hesaba katılmaz).
  - **Enerji eğrisi:** set boyunca ısınma → zirve → iniş; ani enerji düşüşü ve tekrar eden sanatçı önlenir.
- [ ] Açıklanabilir: her seçimin nedeni görünür ("124 BPM, 8A → 9A, enerji yükseliyor"); kullanıcı şarkıyı
      atlatabilir, kilitleyebilir, sıradakini değiştirebilir.
- [ ] Geçişler Faz 6'nın güvenlik kurallarıyla: şüphede düz geçiş. Drop güveni düşük şarkıda drop'a göre hizalama
      yapılmaz.
- [ ] Kalite hedefi: proje sahibinin dinleme denemesinde "elle seçmiş gibi" bulunan geçiş oranı; ölçütler ve
      ağırlıklar ayarlanabilir, testlerde sentetik kütüphaneyle sınanır.

## Bekleyen kararlar (proje sahibinde)

- **Ses iş parçacığına Windows ses önceliği (MMCSS, "Pro Audio")**: ağır yükte takılmaya karşı daha sağlam;
  `unsafe` kod gerektirdiği için onay şart. Öneri: şimdilik yapılmasın, takılma görülürse ilk bu açılsın.
- **Ekolayzerde sıradaki iş** ([#26](https://github.com/gozlemesevdalisi/Lyraska/issues/26)): elle parametrik EQ
  arayüzü (öneri; lisans beklemiyor), hazır kulaklık listesi (önce ölçüm kaynaklarının lisansı), loudness.
- **Retriever / Veritas'ta sıradaki adım** ([VERITAS.md](VERITAS.md)): kör dinleme aracı (ABX) 0.0.40'ta geldi;
  "320 kbps ile FLAC'ı ayırabiliyor muyum?" sorusunu kulakla cevaplar. R2 sonucu geldi: kapıdan geçmedi, MP3 kolu
  kapandı (bölüm 8); kâhin ayrıştırması bizim düzeneğimizde birebir tekrarlandı (devir notu 2026-10-16).
- **Fikir havuzu** (aşağıda): hangilerinin yol haritasına gireceği.

## Fikir havuzu (karar bekliyor)

Proje sahibinin sorularından doğan öneriler. Her biri eklenmeden önce lisansı (kod **ve** model
ağırlıkları) ayrıca doğrulanır; ticari kullanımı yasaklayanlar alınmaz.

- **Kaynak ayrıştırma** (Demucs benzeri; kod MIT): şarkıyı davul, bas, vokal ve diğerlerine ayırıp
  Görsel Yönetmen'e ayrı ayrı vermek. Görseller davula ayrı, vokale ayrı tepki verir.
- **Yapay zekâ ile beat/bölüm tespiti**: klasik yöntemlerden daha isabetli; F-ölçüsü ≥ 0,80 hedefine
  yardım eder. Bilinen bazı modellerin ağırlıkları ticari kullanımı yasakladığı için seçim dikkatle yapılmalı.
- **Windows uzamsal ses** (Spatial Audio API): kullanıcı Windows Sonic ya da satın aldığı bir uzamsal
  ses eklentisini açtıysa Lyraska bunu bilinçli kullanır. Dolby'nin kendi teknolojileri kapalı kaynak ve
  ücretli olduğundan GPL-3.0 programa doğrudan eklenemez.
- **WebGPU**: WebGL2'den sonraki nesil; daha karmaşık sahneler için (WebView2 desteği olgunlaşınca).
