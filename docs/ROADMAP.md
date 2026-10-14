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

## Faz 1 — Çalan program, v0.1 (12 Ekim – 8 Kasım 2026)

- [x] Ses motoru: symphonia ile çözme, 64-bit iç işlem, kilitsiz halka tampon, WASAPI çıkışı
- [x] Dosya aç, çal/duraklat (yumuşak geçişli), durdur; sürükle-bırak; konum ve takılma sayacı
- [x] Sarma (ilerleme çubuğu, ←/→); önceden analiz edilen gerçek spektrum (60 kare/sn, 32 bant)
- [x] Müzik kütüphanesi (SQLite): klasör tarama, etiketler, Türkçe duyarlı arama; çalma sırası, önceki/sonraki, otomatik geçiş
- [x] 10 bantlı ekolayzer (sürgüler gerçekten duyulan eğri; bozulma koruması; hazır ayarlar; ayarlar kalıcı)
- [x] Şarkılar arası boşluksuz geçiş (AAC kodlayıcı dolgusu dahil); yerel hata/çökme günlüğü ve "Hata günlüğü" düğmesi
- [x] 3 sahne: nokta matris spektrum, VU ibreleri, "Gece göğü" (WebGL2 gölgelendirici: kuzey ışıkları ve Lyra takımyıldızı).
      0.0.22: sahneler CI'da Windows Edge ile denetlenir; WebGL2 açılamazsa neden hata günlüğüne yazılır ve gece
      göğü 2D yedek çizimle (Lyra dahil) görünür. Proje sahibinde ışıklar görünüyor (#24 kapandı)
- [ ] Kuzey ışıklarını göz kamaştırıcı ve estetik yapmak (proje sahibinin isteği; parlamasız, epilepsi sınırı
      içinde) ([#30](https://github.com/gozlemesevdalisi/Lyraska/issues/30)). 0.0.30: perdeler yeniden çizildi
      (katlanma, ince ışınlar, alttan yeşil üstte mor, dört derinlik, Vega'nın ışık çizgileri); üç manzara
      (göl, korona, karlı vadi) Ayarlar'da. Proje sahibinin seçimi ve onayı bekleniyor
- [x] Gerçek albüm kapakları (0.0.32): şarkının içindeki kapak büyük başlığın yanında ve sıradaki şarkı kartında;
      yoksa albüm adından özgün renk kapağı. Arayüzün vurgu renkleri çalan bölümün temasına yumuşakça geçer.
      Her panel ve sahne bir hata sınırı içinde: hata yalnızca o bölümü kapatır
- [ ] Sürükle-bırak sorunu ([#29](https://github.com/gozlemesevdalisi/Lyraska/issues/29))
- [x] Yalnızca ID3v1 etiketli MP3'lerde başlık/sanatçı okunması
      ([#27](https://github.com/gozlemesevdalisi/Lyraska/issues/27)). 0.0.33: Türkçe harfler (Windows-1254) ve
      UTF-8 doğru okunur; kütüphanedeki etkilenmiş kayıtlar bir sonraki taramada kendiliğinden düzelir.
      Proje sahibinin kendi MP3'lerinde doğrulaması bekleniyor

## Faz 2 — Görsel Yönetmen, v0.2 (9 Kasım – 20 Aralık 2026)

- [x] İşaretleme aracı: şarkı çalarken beat (Boşluk) ve drop (D) işaretleme, doğruluk ölçümü (F-ölçüsü)
- [x] Şarkı haritası analizi: beat (tempo ve vuruşlar), ölçü başı (4/4–3/4), bölümler, drop, enerji — gerçek
      şarkılarda doğruluk işaretleme aracıyla ölçülecek
- [x] Şarkı haritası önbelleği (0.0.20): analiz sonuçları (spektrum, beat, ölçü başı, bölümler, drop, enerji)
      SQLite'ta; anahtar yol + boyut + değiştirilme zamanı + analiz sürümü (algoritma değişince kendiliğinden yeniden
      hesaplanır). Kütüphane arka planda, düşük öncelikle analiz edilir (önce çalan, sonra sıradaki, sonra geri
      kalan); kütüphane listesinde BPM sütunu. 0.0.34: analiz sürümü dört parçaya ayrıldı (spektrum, ritim, ses
      yüksekliği, bas tepeleri); bir parça değişince yalnızca o yeniden hesaplanır, ritim şarkı çözülmeden
      karelerden (4 dakikalık şarkıda ~50 ms; tam analiz ~2,5 sn)
- [x] Gecikme telafisi ve kalibrasyon (0.0.21): verinin ekrana ulaşma süresi her karede ölçülüp telafi edilir; ses
      aygıtının ek gecikmesi (ör. Bluetooth) "Senkron" sekmesinde ayarlanır ya da tıklama kaydıyla ölçülür; vuruş
      göstergesiyle gözle denetlenir. Gerçek aygıtlarda ±20 ms hedefinin ölçümü proje sahibinin denemesiyle
- [ ] Koreografi: atmosfer, ritim ve doku katmanları — Görsel Yönetmen çekirdeği hazır (0.0.17); gece göğü
      bağlandı (0.0.18: bölüm teması, drop öncesi gerilim, drop açılımı, ölçü başı dalgası); gece otoyolu
      bağlandı (0.0.19). Sıradaki: spektrum ve VU sahneleri, doku katmanı
- [x] "Gece otoyolu" sahnesi (0.0.19): şerit çizgileri vuruşlara, sokak lambaları ölçü başlarına kilitli; ufuk
      parıltısı bölüm temasıyla renklenir; drop öncesi ufka çekilir, drop'ta yükselir ve yol iki kat hızlanır
- [x] Epilepsi güvenli modu (0.0.18): her zaman saniyede en fazla 3 parlama (testle sınanır); güvenli modda
      saniyede en fazla 1 nabız ve yarı hızda parlaklık değişimi. Ayar kaydedilir (Ayarlar paneli), açıkken
      ekranda "Güvenli mod" yazar
- [x] Yeni arayüz "Sahne" (0.0.25): sahne tüm pencereyi kaplar, yazı ve düğmeler cam katmanlarda; büyük şarkı
      adı; şarkı haritası şeridi (bölümler gökyüzünün o bölümdeki renginde, enerji, drop'lar); drop geri sayımı;
      sıradaki şarkı; kütüphane ve paneller sağdan açılan çekmecede; fare durunca sinema görünümü; 1–4 tuşları
      sahne seçer; Inter yazı tipi (OFL-1.1)
- [ ] "Gece sürüşü" nostalji modu (proje sahibi onayladı, A'dan sonra): ön camdan gece otoyolu, torpidoda
      özgün tasarım teyp — parlayan nokta matris ekran, ışık halkalı düğmeler, arkadan aydınlatmalı VU
      ibreleri. Hiçbir markanın adı, logosu ya da yazı tipi kullanılmaz

## Faz 3 — Profesyonel ses, v0.3 (21 Aralık 2026 – 17 Ocak 2027)

- [x] Yüksek kaliteli yeniden örnekleme (aygıtın hızına, Windows'a bırakmadan), taşma koruması, sinyal yolu göstergesi (öne alındı: 0.0.12).
      "Müzikler garip geliyor" bildiriminin proje sahibince doğrulanması bekliyor
      ([#25](https://github.com/gozlemesevdalisi/Lyraska/issues/25))
- [ ] Parametrik EQ (motor hazır: kulaklık düzeltmesi bunu kullanıyor; kullanıcının elle ayarlayacağı arayüz yok).
      Ekolayzerle ilgili kalan işler tek yerde: [#26](https://github.com/gozlemesevdalisi/Lyraska/issues/26)
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
- [ ] Crossfeed
- [ ] Retriever: sıkıştırılmış (MP3 vb.) sesi iyileştirme — çalışma adı; ürün içi adı özgün olacak
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

## Bekleyen kararlar (proje sahibinde)

- **Ses iş parçacığına Windows ses önceliği (MMCSS, "Pro Audio")**: ağır yükte takılmaya karşı daha sağlam;
  `unsafe` kod gerektirdiği için onay şart. Öneri: şimdilik yapılmasın, takılma görülürse ilk bu açılsın.
- **Ekolayzerde sıradaki iş** ([#26](https://github.com/gozlemesevdalisi/Lyraska/issues/26)): elle parametrik EQ
  arayüzü (öneri; lisans beklemiyor), hazır kulaklık listesi (önce ölçüm kaynaklarının lisansı), loudness.
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
