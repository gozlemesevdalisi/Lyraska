# İşaretleme aracı (beat ve drop)

Programın beat ve drop bulmasını **gerçek şarkılarda** ölçmek için kullanılır. Yol haritasındaki hedef:
beat F-ölçüsü en az **%80**.

## Nasıl kullanılır

1. Kütüphaneden ya da "Dosya aç" ile bir şarkı açın.
2. Alt paneldeki **İşaretle** sekmesine geçin, **İşaretlemeye başla**'ya basın.
3. Şarkı çalarken:
   - ayağınızla tempo tutar gibi **her vuruşta** `Boşluk`,
   - şarkının patladığı/yükseldiği (drop) anlarda `D`,
   - yanlış bastıysanız `Geri` (Backspace) son işareti siler,
   - bitirmek için `Esc` (ya da düğme).
4. İşaretler siz bastıkça kendiliğinden kaydedilir ("Kaydedildi" yazısı).
5. En az 8 beat işaretledikten sonra **Doğruluğu ölç**: program şarkıyı analiz edip kendi bulduğu vuruşları
   sizinkilerle karşılaştırır. Sonuç da dosyaya yazılır.

**Bluetooth kulaklık ya da hoparlörle** işaretleyecekseniz önce **Senkron** sekmesinde "Ölçümü başlat" ile ses
gecikmesini ölçün. Bu tür aygıtlar sesi 150–250 ms geç çalar; program ölçülen gecikmeyi işaretlerden düşer
(0.0.33'ten önce düşmüyordu: o sürümlerle Bluetooth'ta yapılan işaretler geç kalmış olabilir).

İpucu: Şarkının tamamını işaretlemek gerekmez; 30–60 saniyelik bir bölüm de işe yarar. Farklı türlerden
(pop, rock, Türk sanat/halk, elektronik, rap, akustik) şarkılar en faydalısıdır.

## Sonuçlar ne demek

| Alan                      | Anlamı                                                                         |
| ------------------------- | ------------------------------------------------------------------------------ |
| Beat doğruluğu (F-ölçüsü) | Programın vuruşları sizinkilerle ±70 ms içinde ne kadar örtüşüyor              |
| Parmak gecikmesi          | Tuşa vuruştan ortalama ne kadar geç (+) ya da erken (−) bastığınız             |
| Gecikme düzeltilince      | Parmak gecikmeniz çıkarıldıktan sonraki doğruluk (asıl bakılacak sayı)         |
| Tempo (program / siz)     | Programın ve sizin vuruşlarınızdan çıkan BPM; biri ötekinin 2 katıysa söyleyin |

## Dosyalar nerede, bana nasıl gönderilir

- "İşaret klasörünü aç" düğmesi klasörü açar (uygulama veri klasöründe `isaretler`).
- Her şarkı için bir `.json` dosyası vardır. İçinde **ses yoktur**: vuruş ve drop zamanları, şarkının adı,
  sanatçısı, süresi ve bilgisayarınızdaki dosya yolu (yolda Windows kullanıcı adınız geçebilir).
- Bu dosyaları bir hata kaydına (GitHub → Issues) sürükleyip bırakarak gönderebilirsiniz.

## Telif

Ses dosyaları **hiçbir zaman** depoya girmez: `.gitignore` ses uzantılarını yok sayar ve CI'daki
`npm run check:audio` denetimi depoda ses dosyası bulursa durur. Tek istisna, projenin ffmpeg ile ürettiği
sentetik test sesleridir (`src-tauri/tests/data/`).
