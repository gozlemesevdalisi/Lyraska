# Doğrulama listesi (gerçek cihazda)

Bu liste, **proje sahibinin bilgisayarında henüz denenmemiş** her özelliği içerir. Otomatik testler bunları
sanal ses aygıtıyla ve ekran kartsız tarayıcıyla sınıyor; gerçek kulaklık, hoparlör, ekran kartı ve gerçek
şarkılarla nasıl davrandığını yalnızca siz görebilirsiniz.

**Doğrulama haftası:** bu listedeki sonuçlar bildirilene kadar yeni özellik eklenmez; yalnızca bulunan hatalar
düzeltilir.

## Nasıl kullanılır

1. Test sürümünü kurun: https://github.com/gozlemesevdalisi/Lyraska/releases/download/test-surumu/Lyraska-Kurulum.exe
   Windows "bilinmeyen yayımcı" derse **Ek bilgi → Yine de çalıştır**.
2. **Ayarlar** (sağ üstteki dişli) → en altta sürüm **0.0.44 ya da sonrası** yazmalı.
3. Maddeleri sırayla deneyin. Her madde için bana yalnızca şunu yazmanız yeterli:
   - madde numarası ve **✅ oldu** ya da **❌ olmadı**,
   - olmadıysa ne gördüğünüzü bir cümleyle (ekran görüntüsü varsa çok iyi).
4. Bir şey ters giderse **Ayarlar → Hata günlüğü** düğmesine basın; açılan klasördeki `lyraska.log` dosyasını
   gönderin. Kişisel bilgi içermez (yalnızca program olayları ve dosya adları).

Hepsini bir günde yapmanız gerekmez. En önemlileri **kalın** yazılı: önce onları deneyin.

---

## A. Kütüphane

**A0. Eski kütüphane yedeğiniz açılıyor (#49, 0.0.35)**

- Nasıl:
  1. Lyraska'yı kapatın.
  2. Dosya Gezgini'nin adres çubuğuna `%APPDATA%\io.github.gozlemesevdalisi.lyraska` yazıp Enter'a basın.
  3. Şimdiki `library.sqlite3` dosyasının adını `library.sqlite3.yeni` yapın. Yanında `library.sqlite3-wal` ya da
     `library.sqlite3-shm` varsa onların da sonuna `.yeni` ekleyin (bunlar şimdiki kütüphanenin parçalarıdır; eski
     dosyayla karışmamalı).
  4. Yedeğiniz `library.sqlite3.bak` dosyasının adını `library.sqlite3` yapın.
  5. Lyraska'yı açın.
- Neye bakılır:
  - "Kütüphane açılamadı" uyarısı çıkmaz.
  - **Kütüphane** panelinde eski klasörleriniz ve şarkılarınız görünür.
  - Şarkılar arka planda analiz edilir: BPM sütunu yavaş yavaş dolar.
- Bilmeniz gereken: yedeği aldıktan sonra yeni kütüphaneye eklediğiniz klasörler yedekte yok; onları yeniden
  eklersiniz.
- Olmadıysa: ekrandaki uyarının "Neden:" satırını ve hata günlüğünü gönderin. Geri dönmek için Lyraska'yı kapatıp
  `.yeni` dosyalarının adını eski haline getirin.

**A1. Eski kütüphane yeniden analiz edilmiyor (0.0.34–0.0.36)**

- Nasıl: Yeni sürümü kurduktan sonra programı açın, **Kütüphane** panelinin altındaki "Şarkı haritası: X / Y şarkı
  analiz edildi" satırına bakın. 10 dakika sonra bir daha bakın.
- Neye bakılır:
  - Sayı bir süre düşebilir (drop güveni için şarkı haritaları yenileniyor), ama **dakikalar içinde** yeniden
    dolmalı. Şarkı başına milisaniyeler sürüyor; büyük kütüphanede en fazla 1–2 saat.
  - Bu sırada bilgisayar yavaşlamamalı, müzik takılmamalı.
  - BPM sütunu dolu kalmalı ya da kısa sürede yeniden dolmalı.
- Olmadıysa: kaç şarkınız olduğunu ve sayının ne kadar sürede dolduğunu yazın.

**A2. Pencereye sürükle-bırak (#29)**

- Nasıl: Dosya Gezgini'nden bir MP3, bir FLAC ve bir M4A şarkıyı Lyraska penceresine sürükleyin.
- Neye bakılır: sürüklerken pencerenin kenarında turuncu kesikli çerçeve ve "Bırakın: …" yazısı; bırakınca şarkılar
  kütüphaneye eklenir ve ilki çalar; altta kısa bir bildirim çıkar.
- Ayrıca: bir klasörü bırakın (kütüphaneye eklenir, taranır); bir `.txt` ya da `.wma` bırakın (anlaşılır bir "bu tür
  desteklenmiyor" mesajı).
- Olmadıysa: hata günlüğünü gönderin; içinde "Pencereye … öğe bırakıldı" satırı olup olmadığı nedeni gösterir.

**A3. Tek şarkı eklemek hızlı (0.0.33)**

- Nasıl: **Kütüphane → Şarkı ekle** ile kütüphanede olmayan bir şarkı seçin.
- Neye bakılır: şarkı birkaç saniye içinde listede görünür. Bütün kütüphanenin yeniden taranmasını beklemezsiniz
  ("Taranıyor…" yazısı kısa sürer).

**A4. Eski tip etiketli MP3'lerde Türkçe adlar (#27, 0.0.33)**

- Nasıl: Eskiden listede **dosya adıyla** ya da **bozuk harflerle** (Þ, Ý, Ð, ý, þ) görünen MP3'leriniz varsa
  programı açın. Açılıştaki taramada bu şarkıların etiketleri kendiliğinden bir kez yeniden okunur. Gerekirse
  **Kütüphane** panelinde "Şarkı ekle"nin yanındaki **dönen oklar** düğmesine (Kütüphaneyi yeniden tara) basın.
- Neye bakılır: tarama bitince şarkı adı, sanatçı ve albüm doğru Türkçe harflerle (Ş, İ, Ğ, ı, ş, ğ) görünür.

**A5. Üst klasör eklemek analizleri silmiyor (0.0.33)**

- Nasıl: Kütüphanede alt klasör varken (ör. `D:\Müzik\Pop`) üst klasörü (`D:\Müzik`) ekleyin.
- Neye bakılır: Pop'taki şarkıların BPM'leri kaybolmaz; "analiz edildi" sayısı düşmez.

**A6. Türkçe arama**

- Nasıl: Arama kutusuna Türkçe harfsiz yazın (ör. "gulumse", "sarki").
- Neye bakılır: "Gülümse", "Şarkı" gibi adlar bulunur.

## B. Çalma ve ses

**B1. Ses kalitesi (#25)**

- Nasıl: İyi bildiğiniz birkaç şarkıyı (MP3 ve FLAC) kulaklıkla dinleyin. **Ayarlar → Sesin yolu** satırına bakın.
- Neye bakılır: ses doğal; cızırtı, boğukluk, hız/ton farkı, kesilme yok. Gariplik varsa türünü ve "Sesin yolu"
  satırında yazanı bildirin.

**B2. Ses yüksekliği eşitleme (0.0.28; geçiş #55, 0.0.38)**

- Nasıl: Çok yüksek kaydedilmiş bir şarkı (yeni bir pop) ile sessiz kaydedilmiş bir şarkıyı (eski bir kayıt)
  kütüphaneden **sırayla** çalın: önce sessiz olan, sonra yüksek olan. **Ayarlar → Ses yüksekliği eşitleme**'yi
  kapatıp açarak karşılaştırın.
- Neye bakılır: açıkken iki şarkı yaklaşık aynı yükseklikte çalar. Yüksek şarkının ilk anı patlamaz ya da ezik
  çalmaz; geçişte ani sıçrama olmaz.

**B3. Duraklatma, sarma ve şarkı değiştirirken "tık" yok (0.0.23)**

- Nasıl: Çalarken **Boşluk** ile duraklatıp devam edin; ← → ile sarın; şeride tıklayın; sonraki şarkıya geçin.
- Neye bakılır: hiçbirinde "tık" ya da "pat" sesi yok; ses yumuşakça kısılıp açılıyor.

**B4. Boşluksuz geçiş (tık düzeltmesi #54, 0.0.38)**

- Nasıl: Şarkıların birbirine bağlı olduğu bir albüm (canlı kayıt ya da kesintisiz bir albüm) çalın. CD'den
  alınmış (44,1 kHz) bir albüm en iyisi: Windows'taki çoğu aygıt 48 kHz'te çalışır, o zaman şarkı dönüştürülür.
- Neye bakılır: şarkılar arasında boşluk ya da tık yok; ses sınırda kesintisiz sürer.

**B5. Kulaklık çıkarılınca (0.0.23)**

- Nasıl: Çalarken kulaklığı çıkarın, sonra **Çal**'a basın; kulaklığı geri takıp yine deneyin.
- Neye bakılır: program çökmez; Çal'a basınca o anki aygıtta kalınan yerden sürer.

**B6. Bit-perfect (özel mod, 0.0.28)**

- Önce: kulaklığı çıkarın ya da sesi kısın (özel modda Windows ses düzeyi çalışmayabilir).
- Nasıl: **Ayarlar → Bit-perfect** açın, bir FLAC çalın.
- Neye bakılır: "Sesin yolu" satırında "bit-perfect (özel mod, … bit)" yazar; diğer programların sesi duyulmaz. Açılamazsa
  nedeni yazar ve ses normal yoldan çalar. Denedikten sonra kapatın.

**B7. Uzun çalma, takılma yok**

- Nasıl: Bir saat boyunca kütüphaneden çalın (başka işler yaparken de olur).
- Neye bakılır: ses hiç takılmaz; **Ayarlar → Sesin yolu** altında "Kesinti: 0".

**B8. "Önceki" ve çalma sırası (#51, #52, 0.0.37)**

- Nasıl: Kütüphanede bir albümün ikinci şarkısına çift tıklayın; ilk 3 saniye içinde **Önceki**'ye basın. Sonra bir
  şarkı açılırken (hemen) başka bir şarkıya çift tıklayın.
- Neye bakılır: "Önceki"de ilk şarkı çalar, altta "1 / …" yazar ve şarkı bitince ikinciye geçer. İkinci denemede son
  çift tıkladığınız şarkı çalar ve sıra yazısı ona göredir.

**B9. Kör test (ABX, 0.0.40)**

- Önce: aynı şarkının iki sürümü gerekir, ör. bir FLAC ve ondan yapılmış bir MP3 (ya da iki farklı MP3: 128 ve
  320 kbps). İkisi aynı kayıttan olmalı.
- Nasıl: çekmecede **Kör test** sekmesini açın. **A** ve **B** satırlarındaki **Seç** ile iki dosyayı seçin;
  **Testi başlat**. Kulaklıkla **A**, **B** ve **X**'i istediğiniz kadar dinleyin, **X = A** ya da **X = B** deyin.
  16 deneme sürer; istediğiniz an **Testi bitir** ile çıkabilirsiniz.
- Neye bakılır:
  - A, B ve X arasında geçerken ses **aynı yerden** sürer; tık ya da boşluk yok, ses yüksekliği sıçramaz.
  - Bölümün sonunda ses yumuşakça başa döner.
  - Üstte "seviye farkı … dB eşitlendi · kayma … ms düzeltildi" yazabilir; bu normaldir.
  - Sonunda kaç doğru yaptığınız ve bunun şansla çıkma olasılığı yazar.
  - Test bitince ya da **Testi bitir**'e basınca önceden çalan şarkı kaldığı yerde (duraklatılmış) geri gelir.
  - İki farklı şarkı seçerseniz "aynı kayda benzemiyor" der, test başlamaz; çalan şarkı kesilmez.
- Bildirin: hangi iki dosyayla denediniz ve sonuç (ör. "FLAC – MP3 320: 16'da 9, ayırt edilemedi").

## C. Ekolayzer ve bas

**C1. Bas ve Derinlik (0.0.29)**

- Nasıl: Kulaklıkla **Ekolayzer**'ı açın; büyük **Bas** sürgüsünü 0'dan 18'e getirin; **Derinlik**'i %0'dan %100'e.
- Neye bakılır: bas belirgin artar, ses bozulmaz ve kısılmaz; Derinlik'le bas notalarının altında gümbürtü eklenir.

**C2. Vuruş (0.0.31)**

- Nasıl: Davullu bir şarkıda **Ekolayzer → Vuruş** sürgüsünü %0'dan %100'e getirin. **Kulüp** hazır ayarını deneyin
  (vuruş %70).
- Neye bakılır: davul vuruşunun ilk anı "göğse çarpar"; bas gitar ve uzun bas notaları şişmez.

**C3. Küçük hoparlör bası (0.0.29)**

- Nasıl: Dizüstü bilgisayarın kendi hoparlöründe **Küçük hoparlör** hazır ayarını seçin; **Küçük hoparlör bası**
  anahtarını açıp kapatın.
- Neye bakılır: açıkken bas hissi belirgin artar (hoparlör çalamadığı derin bas yerine harmoniklerini çalar).

**C4. Kulaklık düzeltmesi (AutoEq)**

- Nasıl: autoeq.app'ten kulaklığınızın "Equalizer APO" (ParametricEQ.txt) dosyasını indirin; **Ekolayzer →
  Kulaklık profili** ile yükleyin; açıp kapatın.
- Neye bakılır: profil adı görünür; açıkken ses dengesi değişir; kapalıyken eski hâline döner.
- 0.0.47'den beri Equalizer APO'nun bütün süzgeç türleri okunur (REW gibi programların dosyaları da): LS/HS, eğimli
  raflar ("LSC 6 dB"), "BW Oct", LP, HP, BP, NO, AP. Elinizde böyle bir dosya varsa yükleyin; Equalizer APO'da
  nasıl duyuluyorsa Lyraska'da da öyle duyulmalı. Okunamayan satır olursa hangi satır olduğu yazar.

## D. Görseller

**D1. Kuzey ışıklarının manzaraları (#30, 0.0.30)**

- Nasıl: **Gece göğü** sahnesinde (1 tuşu) bir şarkı çalın; **Ayarlar → Gece göğü**'nde **Göl**, **Korona** ve
  **Karlı vadi**'yi sırayla deneyin.
- Neye bakılır ve bildirin: hangisini en çok beğendiniz, hangisi varsayılan olsun, kalmasını istemediğiniz var mı?
  Işıklar göz kamaştırıcı ama rahatsız edici olmamalı.

**D2. Akıcılık (0.0.33)**

- Nasıl: Büyük kütüphaneyle bir şarkı çalarken **Kütüphane** panelini açık bırakın; sahneleri 1–5 tuşlarıyla
  değiştirin.
- Neye bakılır: görüntü takılmadan akar (saniyede 60 kare); panel açıkken de kapalıyken de aynı.

**D3. Drop sayacı, şeritteki DROP ve gökyüzünün gerilmesi (0.0.36, 0.0.39)**

- Nasıl: Belirgin drop'u olan bir elektronik şarkı çalın; drop'tan 30 saniye önce sağ altta "Drop yaklaşıyor"
  sayacı çıkmalı. Sonra drop'u olmayan sakin bir şarkı çalın.
- Neye bakılır: sayaç gerçek drop'ta sıfıra iner; alttaki şeritte o noktada "DROP" yazar; gökyüzü drop'a doğru
  gerilir ve drop'ta açılır. Sakin şarkıda ve "drop olmayan" yerlerde üçü de olmaz (hepsi aynı droplara bakar).
  Yanlış yerde çıkan ya da gerçek drop'ta çıkmayan drop'u şarkı adı ve saniyesiyle bildirin.

**D4. Albüm kapakları (0.0.32)**

- Nasıl: İçinde kapak resmi olan şarkılar çalın (çoğu MP3, FLAC, M4A'da vardır).
- Neye bakılır: kapak büyük başlığın solunda ve "Sıradaki" kartında görünür. Kapağı olmayan şarkıda albüm adından
  özgün renkli bir kapak.

**D5. Arayüzün renkleri bölüme uyuyor (0.0.32)**

- Nasıl: Nakaratı belirgin bir şarkıyı **Gece göğü**'nde çalın.
- Neye bakılır: nakarata geçince gökyüzüyle birlikte sürgülerin ve ışıkların rengi 2–3 saniyede yumuşakça değişir;
  ani parlama yok.

**D6. Diğer sahneler ve Görsel Yönetmen (VU ve spektrum 0.0.41)**

- Nasıl: **Gece otoyolu** (2), **VU ibreleri** (3), **Spektrum** (4) sahnelerinde drop'lu bir şarkı çalın.
- Neye bakılır: otoyolda çizgiler vuruşla, lambalar ölçü başıyla akar; drop'tan önce ufka çekilip drop'ta açılır.
  VU ibreleri ve spektrum sesle birlikte oynar.
- VU ve spektrumda ölçerlerin arkasında hafif bir ışık halesi var: rengi alttaki şeritte o bölümün rengi; sakin
  bölümde kısık, yoğun bölümde canlı. Drop'tan önce hale daralır (spektrumda tepe noktaları havada asılı kalır),
  drop'ta açılır. Değişimler yumuşak olmalı; ani parlama yok.

**D7. Epilepsi güvenli modu ve "animasyonları azalt"**

- Nasıl: **Ayarlar → Epilepsi güvenli modu**'nu açın. Ayrıca Windows'ta **Ayarlar → Erişilebilirlik → Görsel
  efektler → Animasyon efektleri**'ni kapatıp programı yeniden açın.
- Neye bakılır: güvenli modda görüntü belirgin biçimde sakinleşir, ekranda "Güvenli mod" yazar; animasyonlar
  kapalıyken hareket azalır. Hiçbir durumda ani tam ekran parlama yok.

**D8. Sinema görünümü**

- Nasıl: Çalarken fareye ve klavyeye birkaç saniye dokunmayın.
- Neye bakılır: düğmeler ve yazılar çekilir, yalnızca sahne kalır; fare kıpırdayınca geri gelir.

**D9. Gece sürüşü (0.0.44)**

- Nasıl: Üstteki **Gece sürüşü** düğmesine (ya da 5 tuşuna) basın, bir şarkı çalın; fareye birkaç saniye dokunmayın
  (sinema görünümü).
- Neye bakılır: ön camdan gece otoyolu (çizgiler vuruşla, lambalar ölçü başıyla); torpidoda teyp: kasetin
  makaraları çalarken döner, nokta matris ekranda sanatçı ve şarkı adı kayar, altında spektrum; iki VU ibresi sesle
  oynar; çal düğmesinin halkası yanar. Teyp, düğmeler ve başlık görünürken kısık durur, sinema görünümünde yavaşça
  belirir. Hiçbir yerde marka adı ya da logosu yok. Ani parlama yok.
- Bildirin: beğendiniz mi, değişmesini istediğiniz bir şey var mı (renk, teybin boyutu, kaset)?

## E. Senkron ve işaretleme

**E1. Kablolu kulaklıkta senkron (0.0.21)**

- Nasıl: Kablolu kulaklıkla bir şarkı çalın; **Senkron** panelindeki vuruş göstergesine bakın.
- Neye bakılır: nokta her vuruşta, vuruşu duyduğunuz anda kenara değer. Önde ya da geride kalıyorsa bildirin.

**E2. Bluetooth kulaklıkta ölçüm (0.0.21)**

- Nasıl: Bluetooth kulaklıkla **Senkron → Ölçümü başlat**; her tıklamada **Boşluk**'a basın.
- Neye bakılır: sonunda "ses yaklaşık … ms geç duyuluyor" yazar (genelde 150–250 ms); sonra şarkıda nokta vuruşla
  birlikte olur.

**E3. İşaretleme ve doğruluk (0.0.33 düzeltmesiyle)**

- Nasıl: Bir şarkıda **İşaretle → İşaretlemeye başla**; 30–60 saniye her vuruşta **Boşluk**, drop'ta **D**; sonra
  **Doğruluğu ölç**. Bluetooth kullanıyorsanız önce E2'yi yapın. Ayrıntılar: [ISARETLEME.md](ISARETLEME.md).
- Neye bakılır: "Gecikme düzeltilince" doğruluğu %80 ve üstü hedef. Sonucu ve şarkının türünü bildirin.

**E4. Ton (Camelot, 0.0.42)**

- Nasıl: Kütüphanede birkaç dakika bekleyin (yeni sürümde şarkılar bir kez daha arka planda çözülür; bu sırada
  müzik takılmamalı). **Ton** sütununa bakın. Tonunu bildiğiniz birkaç şarkı çalın; sağ alttaki bilgi kartında
  "8A · La minör" gibi bir yazı çıkar. **İşaretle** sekmesinde **Ton** listesinden doğru tonu seçip (birkaç beat de
  işaretleyip) **Doğruluğu ölç**'e basın.
- Neye bakılır: tonu bildiğiniz şarkılarda "aynı ton", "komşu ton" ya da "paralel ton" (✓) çıkmalı. Ton değiştiren
  bir şarkıda o bölüme gelince bilgi kartı vurgulu olarak bölümün tonunu gösterir. Yalnızca davul ya da konuşma
  olan kayıtlarda ton boş kalır ("—").
- Bildirin: şarkı adı, sizin bildiğiniz ton ve programın yazdığı ton (✓ olmayanlar en değerlisi).

**E5. Aksak ölçü (0.0.43)**

- Nasıl: Aksak usullü birkaç şarkı çalın (ör. 9/8 roman havası, 7/8 ya da 5/8 bir halk şarkısı). Kütüphanedeki
  şarkıların ritmi arka planda saniyeler içinde yenilenir (şarkılar yeniden çözülmez).
- Neye bakılır: sağ alttaki bilgi kartında "9/8 (2+2+2+3)" gibi ölçü yazar. **Gece otoyolu**'nda sokak lambaları
  ölçü başlarında geçer; **Senkron** panelindeki vuruş noktası uzun ve kısa vuruşlara uyar. Düz 4/4 şarkılar
  "4/4" kalmalı. Ölçüsü belirsiz bir şarkıda ölçü hiç yazmaz; bu bilerek yapıldı.
- Bildirin: aksak olduğunu bildiğiniz şarkıda programın yazdığı ölçü (ya da hiç yazmaması) ve şarkının adı.

## F. Hata günlüğü

**F1. Hata günlüğü açılıyor**

- Nasıl: **Ayarlar → Hata günlüğü**.
- Neye bakılır: Dosya Gezgini açılır ve `lyraska.log` seçili görünür (kullanıcı adınızda boşluk olsa da).

---

## Bildirdikleriniz

Sonuçları bana yazdıkça buraya işlenir.

| Madde                | Sonuç                                                   | Not                                                                                                                                    |
| -------------------- | ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| — (0.0.33'te)        | ❌ Kütüphane hiç açılmıyordu: `no such table: analyses` | #49. 0.0.5–0.0.19'un kütüphane dosyası 0.0.28'den beri açılmıyordu. 0.0.35'te düzeltildi (testli); A0 ile sizin yedeğinizle denenecek. |
| A0 (0.0.35)          | ✅ Oldu: eski yedek geri konunca kütüphane açıldı       | #49 kapandı (14 Ekim).                                                                                                                 |
| — (0.0.39)           | ✅ Denendi, sorun çıkmadı                               | 15 Ekim. Hangi maddelerin denendiği yazılmadı; madde madde sonuçlar gelince tabloya ayrı ayrı işlenecek.                               |
| Faz 1 (A2, D1 dahil) | ✅ "Faz 1 tamam"                                        | 16 Ekim. Sürükle-bırak (#29) ve kuzey ışıkları (#30) kapandı. Gece göğünün varsayılan manzarası aynı kaldı; değişsin isterseniz yazın. |
