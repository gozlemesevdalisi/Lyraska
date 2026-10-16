# İkinci kalite denetimi — 16 Ekim 2026

Proje sahibinin isteği: "İki kere denetim yap; ikincisi bağımsız ve üst düzey profesyonel olsun." Bu belge iki
denetimi bir arada sunar:

- **Bölüm A — yeniden denetim (bağımsız değil):** ilk denetimi (`DENETIM.md`) yapan ve düzeltmeleri yazan taraf, 55
  bulgunun durumunu çıkardı ve birleşen düzeltmeleri karşıt gözle yeniden okudu.
- **Bölüm B — bağımsız denetim:** altı ayrı denetçi (ses motoru, analiz ve Görsel Yönetmen, kütüphane ve çekirdek,
  arayüz, yayın ve güvenlik, matematik ve DSP) aynı kod kopyasını birbirinden ve ilk denetimden habersiz inceledi.
  Tam raporları `docs/denetim-2/` altında, kelimesi kelimesine.

Denetlenen kod: `main` @ `25f83f8` (PR #82 ve #84 birleşmiş; #85 henüz değil). Bölüm A ayrıca yerelde hazır bekleyen
PR 3–6 dallarını kapsar.

## 0. Yönetici özeti (proje sahibi için)

- **Program sağlam bir temelde.** Altı bağımsız denetçinin puanları 6–8/10 (ortalama 7,2). Hepsi aynı şeyi söyledi:
  gerçek zamanlı ses yolu, DSP matematiği, veri göçleri, tip sözleşmeleri ve kalite kapıları "bu büyüklükte bir açık
  kaynak projede alışılmadık ölçüde disiplinli". Matematik denetçisi RBJ, BS.1770-4, rubato, Krumhansl–Kessler,
  Camelot ve binom testini kendi hesabıyla yeniden türetti: **hepsi doğru**. Kritik (hemen durdur) bulgu **yok**.
- **İlk denetimin 9 yüksek önemli bulgusunun 9'u ya birleşti ya yerelde hazır** (Bölüm A.1). Bağımsız denetçiler bu
  bulguların çoğunu habersizce yeniden buldu (Bölüm 4): iki denetim birbirini doğruluyor.
- **Bağımsız denetim 7 yüksek önemli yeni/ek bulgu çıkardı:** boş görünen bir kaynak klasörün (sürücü harfi
  değişmesi, bağlı olmayan disk) bütün şarkıları ve saatlerce süren analizleri silmesi (KUT-01); bozuk veritabanından
  kurtulma yolu olmaması (KUT-02); kulaklık takılınca sesin eski aygıtta kalması (SES-01); sahnelerin boştayken de
  tam hızda çizilmesi (ARA-01); yayın hattının CI'yi beklememesi, sabitlenmemiş eylemler ve commit izlenemezliği
  (YAY-01/02/03; üçü PR 4'te hazır). Ayrıntı ve öncelik sırası Bölüm 5'te.
- **Kararınızı bekleyen dört konu** (Bölüm 5.3): bozuk veritabanı dosyasının kenara alınıp yeniden kurulması; ses iş
  parçacığına Windows önceliği vermek için `unsafe` kullanımı (MMCSS); WebView2 kurulumunun internet istemesi ile
  "çevrimdışı" vaadi; gerçek şarkılarla doğruluk ölçümü (işaretleme aracı).

## 1. Kapsam ve yöntem

### 1.1 Bölüm A (yeniden denetim)

Kaynak: `main` @ `25f83f8` ve yerel PR 3–6 dalları. (1) `DENETIM.md`'nin 55 bulgusunun her biri kodda doğrulandı ve
durumu yazıldı; (2) #82 ve #84 farkları satır satır karşıt gözle okundu ("bu düzeltme neyi bozmuş olabilir?");
(3) bütün kalite kapıları yeniden çalıştırıldı. Denetçi düzeltmeleri yazan tarafla aynı olduğu için kör nokta riski
vardır; Bölüm B bu riske karşıdır.

### 1.2 Bölüm B (bağımsız denetim)

- **Yalıtım:** her denetçi yalnızca salt okunur bir kopyayı (`main` @ `25f83f8`) gördü. `DENETIM.md`, devir notları,
  GitHub issue/PR'lar ve bu oturumun çalışma dalları okuma yasağındaydı. Denetçilere hiçbir bulgu, hipotez ya da
  öncelik verilmedi; yalnızca alan, dosya listesi, standartlar ve rapor biçimi verildi (ortak kural metni
  `docs/denetim-2/BRIEF.md`).
- **Yöntem şartı:** her bulgu dosya:satır ve kod alıntısı ya da hesapla kanıtlanır; güven derecesi (kesin / yüksek /
  olası) ayrı yazılır; önem ölçeği Kritik / Yüksek / Orta / Düşük. Standartlar: ITU-R BS.1770-4, EBU R128/Tech 3341,
  RBJ Audio EQ Cookbook, WASAPI, WCAG 2.x (2.3.1 parlama, 2.5.8 hedef boyutu), ISO 9241-391, SPDX, GitHub Actions
  güvenlik uygulamaları, Windows veri klasörü kuralları.
- **Çalıştırılanlar:** matematik ve ses denetçileri formülleri python3 ile yeniden türetti (betikler
  `docs/denetim-2/hesap/`, hepsi yeniden üretilebilir). Yayın denetçisi 14 kalite kapısını kendi başına koşturdu
  (`docs/denetim-2/kapilar-ozet.tsv`): 13 geçti; `cargo deny check advisories` yalnızca Linux hedefindeki `gtk`
  zincirindeki bakımsız bir crate uyarısıyla kaldı (Windows ikilisinde yok, güvenlik açığı değil). Arayüz denetçisi
  tsc, eslint ve vitest koşturdu (temiz).
- **Sınır:** bütün denetçiler aynı model ailesinden (Claude) ayrı bağlamlarda çalıştı; farklı bir ekip ya da insan
  denetçi kadar bağımsız sayılmamalı. Linux'ta yapıldı: WASAPI ve gerçek aygıt davranışı yalnızca kod okumasıyla.

## 2. Bölüm A — yeniden denetim

### A.1 Bulgu durumu (ilk denetimin 55 maddesi)

Durum sözlüğü: **birleşti** = main'de, test ile; **hazır** = yerel dalda yazıldı ve test edildi, PR sırada;
**açık** = henüz yapılmadı; **onay** = proje sahibinin kararı gerekir.

| Kod | Konu (kısa)                                         | Durum                  | Nerede / not                                                             |
| --- | --------------------------------------------------- | ---------------------- | ------------------------------------------------------------------------ |
| K1  | Okunamayan klasör "silinmiş" sayılıyor              | birleşti               | #82; `removal_candidates`, `ScanSummary.unreadable`                      |
| S1  | MP3'te ilk 47 örnek                                 | birleşti               | #84; `pts + trim_start`; `formats.rs` 66 150 örnek, korelasyon > 0,98    |
| U1  | Açılamayan şarkı sırayı öldürüyor                   | birleşti               | #84; `openPath` sonuç döndürür, otomatik geçişte atlama                  |
| C1  | Test sürümü CI'yi beklemiyor                        | hazır (PR 4)           | `workflow_run` kapısı                                                    |
| C2  | Yazma izni + sabitlenmemiş eylemler                 | hazır (PR 4)           | iş bazlı izin, SHA sabitleme, `persist-credentials: false`, token yok    |
| C3  | Üçüncü taraf lisans bildirimleri yok                | hazır (PR 5)           | `UCUNCU-TARAF.txt` (179 crate + 6 npm), CI denetimi, Ayarlar > Lisanslar |
| U3  | Görsel veri karede 2–3 kez                          | birleşti               | #85; tek paylaşımlı döngü, ≤ 60 istek/sn                                 |
| A2  | Yarım analiz "tamam" diye saklanıyor                | birleşti               | #82; `truncated`, `saved() → None`                                       |
| A1  | Nabız seyreltmesi 172–176 BPM'de düzensiz           | birleşti               | #85; `stride`: sabit adım + 20 ms pay                                    |
| K2  | Analiz tablosu sorguları yavaşlatıyor mu            | açık (ölçüm gerek)     | önce büyük kütüphanede `library_status` süresi ölçülecek                 |
| K3  | Tarama bayrağı `scanning=false` yarışı              | açık                   | küçük; bağımsız denetim KUT-09 ile aynı kök                              |
| K4  | Çıkarılan kaynağın taraması yazıyor                 | birleşti               | #82                                                                      |
| K5  | Kör test hazırlığı kilit altında                    | birleşti               | #84; `spawn_blocking` + `abx_install`                                    |
| K6  | Ayar kaydı fsync yok, tek bozuk alan her şeyi siler | birleşti               | #82; okuma hatası yolu için bkz. KUT-06                                  |
| S2  | Bit-perfect dar bit derinliğinde yanıltıyor         | birleşti               | #84; paylaşımlı yola dönüş + açıklama                                    |
| S3  | Özel modda şarkı sonunda arabellek tekrarı          | birleşti               | #84; bir tampon sessizlik; cihazda doğrulanacak (DOGRULAMA B11)          |
| S4  | MMCSS yok (öncelik)                                 | onay                   | `unsafe` gerektirir; bağımsız denetim SES-03 ile aynı                    |
| A3  | Geçişte görsel şarkı seçimi gecikmeyi saymıyor      | birleşti               | #85; `playing_at(offset)`                                                |
| A4  | Nabız ∪ vurgu garantisi                             | birleşti (kısmen)      | #85; vurgular nabzın alt kümesi; sarmadaki durumsuzluk TS sınırlayıcıda  |
| A5  | Drop zamanı ölçü fazına körü körüne bağlı           | açık                   | analiz iyileştirmesi; bkz. ANL-12                                        |
| A6  | Yenilemede eskimiş ses yüksekliği hızlı ölçülmüyor  | açık                   |                                                                          |
| U2  | Boşluksuz olmayan geçiş 250 ms yoklamaya bağlı      | açık                   | gizli pencerede zamanlayıcı kısılması; olay tabanlı bildirim gerekir     |
| U4  | Arama sonrası seçim eski sırada                     | açık                   | bağımsız denetim ARA-03 ile aynı                                         |
| U5  | Çekmecede odak yönetimi yok                         | açık                   | bağımsız denetim ARA-06 ile aynı                                         |
| U6  | Sahne döngüsü hatası hata sınırına ulaşmıyor        | birleşti               | #85                                                                      |
| U7  | Kütüphane yoklaması uçuştaki isteği izlemiyor       | açık                   | küçük                                                                    |
| C4  | Yayında iptal linki boş bırakabilir                 | hazır (PR 4)           | `cancel-in-progress: false`; önce yükle, sonra sil                       |
| C5  | Güvenlik uyarıları denetlenmiyor                    | hazır (PR 4)           | `Güvenlik uyarıları` işi (cargo-deny advisories, npm audit)              |
| C6  | Aynı sürümle farklı ikililer; commit ekranda yok    | açık                   | bağımsız denetim YAY-03 "yüksek" dedi; bkz. Bölüm 5                      |
| C7  | Sürüm eşitliği testi yok                            | hazır (PR 4)           | `surum_package_json_ile_ayni`                                            |
| C8  | Yayımcı adı / telif satırı                          | hazır (PR 4)           | `publisher: Lyraska`                                                     |
| C9  | Bindings denetimi eski dosyayı yakalamıyor          | hazır (PR 4)           | klasör önce boşaltılır                                                   |
| C10 | timeout yok, araç zinciri `@stable`                 | hazır (PR 4)           | `timeout-minutes`, Rust 1.97.0                                           |
| S5  | EQ korumasına bildirilen yükseltme anlık            | açık                   | ses yolu; dikkatli test gerektirir                                       |
| S6  | Bas/vuruş zarfı denormal                            | açık                   | küçük; `flush`                                                           |
| S7  | Bozuk `elst`'te u64 taşması                         | açık                   | küçük; `checked_mul`                                                     |
| S8  | Kör test 0,5 sn'den önce negatif kayma              | açık                   | bağımsız denetim SES-08 ile aynı                                         |
| S9  | Hata yutma noktaları                                | açık                   | bkz. SES-09                                                              |
| S10 | `SLOTS = 8` yuva ezilmesi                           | açık                   | uç durum                                                                 |
| S11 | 5/7.1 kanal ağırlıkları                             | açık                   | bağımsız denetim SES-06 ile aynı                                         |
| A7  | Tek global ölçü fazı                                | açık                   | bkz. ANL-04/05                                                           |
| A8  | Arka plan `skip` kümesi oturum boyu kalıcı          | açık                   | A2 düzeltmesiyle etkileşir (ANL-16, Y4)                                  |
| A9  | Kilit altında analiz `join`                         | açık                   | bağımsız denetim ANL-06 ve SES-02 ile aynı                               |
| A10 | Yenileme başarısızsa sıradaki bekliyor              | açık                   |                                                                          |
| A11 | Sessizlikte hayalet vuruş                           | açık                   | analiz iyileştirmesi                                                     |
| A12 | FAT32 yaz/kış saati `mtime`                         | açık                   | bkz. KUT-15                                                              |
| A13 | Spektrum karesi gecikmesi                           | açık                   | bağımsız denetim ANL-03 ile aynı (ölçtü: 44,1 kHz'te +15 ms)             |
| A14 | `dedup` işlevsiz, Windows ayrılmış adlar vb.        | açık                   | bkz. ANL-15                                                              |
| K7  | Bozuk veritabanında kurtulma yolu                   | onay (tasarım)         | bağımsız denetim KUT-02 "yüksek" dedi; öneri Bölüm 5.3                   |
| K8  | OneDrive yer tutucuları indiriliyor                 | açık                   |                                                                          |
| K9  | Sembolik bağ sessizce atlanıyor                     | açık                   |                                                                          |
| K10 | NFD Türkçe arama                                    | açık                   | küçük                                                                    |
| K11 | Tarama sürerken liste baştan isteniyor              | açık                   | bağımsız denetim KUT-04 ve ARA-14 ile aynı                               |
| K12 | Günlükte kullanıcı adı                              | açık                   | maskeleme                                                                |
| K13 | Tek kopya koruması yok                              | açık                   | tauri-plugin-single-instance                                             |
| K14 | UTF-8 olmayan yollar; alttan taşma                  | kısmen (#82: taşma)    | `saturating_sub`; lossy yol açık                                         |
| K15 | `indexing_slicing` lint'i                           | açık                   |                                                                          |
| U8  | Harita/ton sorgusu tek hatada vazgeçiyor vb.        | kısmen (PR 6: kısayol) | sorgu yeniden deneme açık                                                |
| U9  | Güvenli modda VU lambası 2/sn, şerit hızı           | açık                   | görsel güvenlik; bkz. ARA-07                                             |
| U10 | Duraklatılmışken tam hız çizim vb.                  | açık                   | bağımsız denetim ARA-01 "yüksek" dedi; bkz. Bölüm 5                      |
| C11 | npm lisans yalnız yasak listesi; belgeler eski vb.  | kısmen (PR 4–6)        | çift koşu, `-First 1`, README, hata formu, Faz 2 yapıldı; kalanlar açık  |

Sayım: birleşti 11, hazır 11 (+2 kısmen), onay 2, açık 29. İlk denetimin 9 yüksek önemli maddesinin 9'u ya
birleşti ya hazır.

### A.2 Birleşen düzeltmelerin karşıt okuması (yeni gözlemler)

Her düzeltme için "bu neyi bozmuş olabilir?" sorusuyla fark yeniden okundu. Bulunanlar küçük; hiçbiri geri alma
gerektirmiyor.

- **Y1 (düşük, kesin) — 32 bit kayan noktalı kaynakta özel mod hiç açılmıyor.** `source_bits` F32/F64 için 32
  sayılıyor; aygıtlar özel modda çoğunlukla 24 geçerli bit verir → 32 bit float WAV/FLAC'ta bit-perfect her zaman
  kapanır ve açıklama yazılır. Kayan noktalı kaynak zaten tamsayı aygıtta bit bit aynı çıkamaz; davranış doğru, ama
  açıklama "şarkı 32 bit" diyor; "kayan noktalı" demesi daha anlaşılır olur. Bağımsız denetim SES-10 aynı noktaya
  başka açıdan değindi (kayan noktalı kaynak "bit-perfect" sayılmamalı).
- **Y2 (düşük, kesin) — `requested` eski kalabiliyor.** Kullanıcı sıranın dışından (Dosya aç) bir dosya açar ve o
  dosya açılamazsa `status.track = null` ve `requested` hâlâ sıranın eski şarkısı → sıra "etkin" görünür; Sonraki,
  sıranın eski konumundan devam eder. Kötü bir davranış değil (sıra kaybolmuyor) ama belgelenmemiş. `usePlayback.ts`.
- **Y3 (bilgi) — Sarmanın hemen ardından görseller gecikme kadar bekler.** `playing_at` çıkış akışının başından
  önceye gitmez: 200 ms ses gecikmesi ayarlıysa sarmadan sonraki ilk 200 ms görseller sarılan noktada durur (eskiden
  sarılan noktanın 200 ms öncesini gösteriyordu: kulağa gelmeyen içerik). Yeni davranış duyulanla uyumlu; test buna
  göre güncellendi.
- **Y4 (düşük, kesin) — Kesilen analiz oturum boyunca yeniden denenmiyor.** A2 düzeltmesiyle kesilen analiz
  `saved() = None` → arka plan `Outcome::Failed` → `skip` kümesine girer (A8). Ağ kopması geçince de program yeniden
  açılana kadar o şarkı analiz edilmez. Sonsuz döngü yok (`background.rs:146-151`). Bağımsız denetim ANL-16 öteki
  yüzünü buldu: bozuk kuyruklu dosya her açılışta baştan analiz edilir. İkisi birlikte çözülmeli: kesilen analizi
  "eksik" bayrağıyla saklamak ve zaman aşımlı `skip`.
- **Y5 (düşük, olası) — `stride` ortanca aralığı kullanıyor.** Tempo şarkı ortasında ikiye katlanırsa adım yavaş
  yarıya göre seçilir; hızlı yarıda son güvence olan `thin` devreye girer ve orada desen yine düzensiz olabilir. Hız
  garantisi bozulmaz. İyileştirme: adımı bölüm bölüm seçmek (ANL-04 ile birlikte).
- **Y6 (bilgi) — Özel mod boşaltması yalnızca yoklamalı özel moda yazıldı.** `wasapi.rs`'te özel mod yoklamalıdır;
  kapsam tam. Gerçek sürücü davranışı cihazda doğrulanacak (DOGRULAMA B11).
- **Y7 (bilgi) — Kök klasör yokken silme olmuyor.** `scan_folder` kök yoksa `FolderMissing` döner. Ama bağımsız
  denetim KUT-01 asıl boşluğu gösterdi: kök **var ama boş** görünüyorsa (sürücü harfi başka diske gitmiş) hepsi
  silinir. K1 düzeltmesi bunu kapsamıyor; yeni iş.
- **Y8 (düşük, kesin) — Tarama uyarısı sayı veriyor, yer vermiyor.** "3 alt klasör ya da dosya okunamadı" deniyor
  ama hangileri olduğu günlüğe de yazılmıyor. Günlüğe yolların yazılması (K12 maskelemesiyle) destek için gerekli.
- **Y9 (CI'da yakalandı, düzeltildi) — Önbellekten yenilemede kareler iş parçacığı içinde yayımlanıyordu.** #85'in
  CI koşusunda `eskiyen_parca_acilista_yenilenir_kareler_hemen_hazir` yarış yüzünden kırmızı oldu; kareler artık
  iş parçacığı açılmadan yayımlanır (`publish_cached`) ve belirlenimci bir test eklendi.

### A.3 Kalite kapıları

Bu oturumda `main` @ `25f83f8` ve PR 3 dalında hepsi yeşil: `cargo fmt --check`, `cargo clippy --all-targets --locked
-D warnings` (Linux ve `x86_64-pc-windows-gnu`), `cargo test --locked` (326 birim + 4 kodek; 1 ignored: gerçek aygıt
gerektiren ayar keşfi), bindings farkı yok, `cargo deny check licenses bans sources`, `cargo deny check advisories`
(Linux'ta 0 açık; 1 bakımsız uyarısı), `npm audit --omit=dev` (0), prettier, eslint, tsc, vitest (275), `npm run
build`, `check:commands` (42 komut), `check:licenses` (242 paket), `check:audio` (276 dosya), `check:notices` (PR 5
dalında).

## 3. Bölüm B — bağımsız denetim özeti

Tam raporlar (denetçinin kendi sözleriyle, dosya:satır kanıtlarıyla): `docs/denetim-2/ses.md`, `analiz.md`,
`kutuphane.md`, `arayuz.md`, `yayin.md`, `matematik.md`. Aşağıda yalnızca puanlar, dağılım ve her denetçinin kendi
seçtiği en önemli üç bulgu var.

| Alan                          | Puan | Bulgu (Y/O/D)   | Denetçinin en önemli üç bulgusu                                                                                                                                                     |
| ----------------------------- | ---- | --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ses motoru (SES)              | 8/10 | 14 (1/5/8) + 4  | SES-01 aygıt değişince akış eski aygıtta kalır; SES-02 kilit altında bloklayan işler (sarma/açmada görseller donar); SES-04 ilk ses için 0,6 s bekleme                              |
| Matematik ve DSP (MAT)        | 8/10 | 11 (0/4/7) + 2  | MAT-01 sınırlayıcı örnekler arası tepeyi ölçmüyor (+2,9 dBTP geçebilir); MAT-02 EQ uç bantları raf değil tepe (20 Hz'de sürgünün yarısı); MAT-03 kulaklık ön kazancı 96 kHz'e sabit |
| Analiz ve Yönetmen (ANL)      | 7/10 | 19 (0/7/12) + 3 | ANL-01 176,4/192 kHz'te FFT penceresi kare adımından kısa (sesin %30–36'sı analiz dışı); ANL-03 spektral kare gecikmesi (+15 ms @ 44,1 kHz); ANL-04 tek küresel tempo               |
| Arayüz (ARA)                  | 7/10 | 17 (1/6/10) + 4 | ARA-01 sahneler boşta/duraklatmada da 60 kare/sn tam ekran çizer, zayıf GPU koruması yok; ARA-02 görsel veri 2–3 ayrı akış (#85 ile kapandı); ARA-03 arama sonrası Enter hatası     |
| Kütüphane ve çekirdek (KUT)   | 7/10 | 17 (2/5/10) + 2 | KUT-01 boş görünen kök bütün şarkıları ve analizleri siler; KUT-02 bozuk veritabanından kurtulma yolu yok; KUT-03 analiz önbelleği Roaming AppData'da                               |
| Yayın, güvenlik, lisans (YAY) | 6/10 | 19 (3/8/8) + 2  | YAY-01 eylemler SHA ile sabit değil + yazma yetkili token; YAY-02 test sürümü CI'yi beklemiyor; YAY-03 hangi exe hangi commit belli değil (0.0.44 ile 5 farklı exe)                 |

Y/O/D = Yüksek/Orta/Düşük; "+ n" = kanıtlanamayan şüphe (güven "olası"). Kritik bulgu hiçbir alanda yok.

**Matematik denetçisinin özet tablosu** (konu → sonuç): RBJ/peq doğru; 10 bant EQ sapma var (en kötü 1,11 dB, uç
bantlar tepe); BS.1770-4 doğru (48 kHz katsayıları 1e-16 içinde, −23 dBFS → −22,993 LUFS); normalize doğru;
sınırlayıcı sapma var (örnek tepesi ispatlı, örnekler arası değil); rubato doğru (geçirme ±0,0000 dB, alias ≤ −142
dB); bas/punch sapma var (alt oktav seviyesi −2,6…−12,1 dB oynar); beats/evaluate sapma var (Ellis önseli σ 1,0 vs
1,4; F-ölçüsü açgözlü); Foote doğru; ton doğru (KK 24/24, Camelot 24/24); Yönetmen doğru (≤ 3 nabız/sn 60–240 BPM
taramasıyla); visual_bridge doğru; ABX doğru (12/16 → p = 0,0384).

## 4. Çapraz karşılaştırma: iki denetim birbirini nerede doğruluyor, nerede ayrışıyor

### 4.1 Bağımsız denetimin habersizce yeniden bulduğu ilk denetim bulguları

Bu eşleşmeler iki denetimin birbirinden bağımsız olduğunu ve ilk denetimin isabetli olduğunu gösterir:

| İlk denetim  | Bağımsız denetim                      | Durum                                               |
| ------------ | ------------------------------------- | --------------------------------------------------- |
| C1, C2       | YAY-02, YAY-01 (yüksek)               | PR 4'te hazır                                       |
| C3           | YAY-05                                | PR 5'te hazır                                       |
| C4, C5       | YAY-04, YAY-06                        | PR 4'te hazır                                       |
| C6           | YAY-03 (yüksek)                       | açık; bağımsız denetim önemi yükseltti (Bölüm 5)    |
| C7, C8       | YAY-10, YAY-08                        | PR 4'te hazır                                       |
| C9, C10      | YAY-17, YAY-12, YAY-13                | PR 4'te hazır                                       |
| C11          | YAY-14, YAY-15, YAY-18, KUT-17/YAY-16 | kısmen PR 4–6; kalanı açık                          |
| U3           | ARA-02                                | #85 ile kapandı (denetim #85'ten önceki kodu gördü) |
| U4, U5       | ARA-03, ARA-06                        | açık                                                |
| U8 (kısayol) | ARA-08                                | PR 6'da hazır                                       |
| U10          | ARA-01 (yüksek)                       | açık; önemi yükseltildi (Bölüm 5)                   |
| K6           | KUT-06                                | #82 bozuk dosyayı kapsadı; okuma hatası yolu açık   |
| K7           | KUT-02 (yüksek)                       | karar bekliyor (Bölüm 5.3)                          |
| K11          | KUT-04, ARA-14                        | açık                                                |
| K3           | KUT-09                                | açık                                                |
| S4           | SES-03                                | karar bekliyor (`unsafe`)                           |
| S8, S11      | SES-08, SES-06/MAT-07                 | açık                                                |
| A9           | ANL-06, SES-02, KUT-08                | açık; üç denetçi bağımsızca aynı kilidi işaret etti |
| A12, A13     | KUT-15, ANL-03                        | açık; ANL-03 sayısal olarak ölçtü                   |
| A14          | ANL-15                                | açık                                                |
| A1/A4        | ANL-08, MAT-10                        | #85 ile vurgular nabzın alt kümesi; bkz. 4.3        |

### 4.2 Yalnızca bağımsız denetimin bulduğu bulgular (yeni)

Yüksek ve orta önemliler (tam liste raporlarda):

- **KUT-01 (yüksek, kesin):** var olan ama boş görünen kaynak klasör bir taramada bütün şarkıları ve tetikleyiciyle
  analizlerini siler (sürücü harfi başka diske gidince, bağlı olmayan bölüm, boşaltılmış bulut klasörü). İlk denetim
  yalnızca "okunamayan" durumu (K1) görmüştü. **Veri kaybı sınıfında; önce bu.**
- **SES-01 (yüksek, yüksek güven):** varsayılan aygıt değişince (kulaklık takılınca) akış eski aygıtta kalır; aygıt
  kopunca kurtarma kullanıcıya kalır. Gerçek cihazda ilk fark edilecek sorun.
- **ARA-01 (yüksek):** sahneler boşta ve duraklatılmışken de 60 kare/sn tam ekran gölgelendirici çizer;
  `failIfMajorPerformanceCaveat` ve kare süresi bekçisi yok; "1080p dahili GPU'da 60 fps" hedefi ölçülmüyor.
- **KUT-03 (orta):** analiz önbelleği ve günlükler Roaming AppData'da (gezici profil ve kota sorunu); Windows kuralı
  `%LOCALAPPDATA%`. Doğrulandı: `lib.rs` her yerde `app_data_dir()` kullanıyor.
- **KUT-05 (orta):** sürücü kökü eklenince `$RECYCLE.BIN`, gizli ve sistem klasörleri de taranıyor.
- **SES-04 (orta):** ilk ses için 0,6 s bekleme; önbelleksiz tam uzunluktaki şarkıda −4 dB varsayımından seviye
  kayması.
- **SES-05 (orta):** uzantı listesi çözücü yeteneğinden geniş: Opus çözücü yok, HE-AAC desteklenmiyor.
- **ANL-01, ANL-02 (orta, kesin, hesapla):** 176,4/192 kHz'te sabit 2048 örneklik FFT penceresi kare adımından kısa
  (sesin %30–36'sı analiz dışı); 44,1/48 kHz'te bile en alt bantlar birbirinin kopyası. Spektrum sürümü artışı
  gerektirir (bütün kütüphane yeniden analiz).
- **ANL-04 (orta):** tek küresel tempo; tempo değişen şarkıda yanlış BPM; testi yok.
- **ANL-07 (orta, kesin):** doğruluk kanıtı yalnızca sentetik sinyallerden; gerçek müzikle ölçüm yok (yol haritası
  da kabul ediyor).
- **MAT-01 (orta, simülasyonla):** sınırlayıcı örnek tepesine bakıyor; örnekler arası tepe 0 dBTP'yi aşabilir
  (+2,92 dBTP gösterildi). EBU R128 "−1 dBTP" uygulaması için 2–4× ara değerleme gerekir.
- **MAT-02 (orta):** 10 bant EQ'nun uç bantları raf değil tepe; 20 Hz'de sürgünün ~yarısı; en kötü sapma 1,11 dB
  (belge "~1 dB" diyor, test 1,2 kabul ediyor). Ya raf yap ya belgeyi düzelt.
- **MAT-03 (orta):** kulaklık düzeltmesinin ön kazanç sınırı sabit 96 kHz ve 20 Hz–20 kHz'te hesaplanıyor; 44,1
  kHz'te +0,25 dB eksik / −3,8 dB fazla kısma.
- **MAT-04 (orta):** alt oktav üreticisinin seviyesi giriş frekansına göre −2,6…−12,1 dB oynar; 40 Hz'de 3.
  harmonik −13 dB.
- **ARA-04 (orta):** oynatma denetimleri, başlık ve üst çubuk hiçbir hata sınırında değil; belge "her panel ve sahne"
  diyor.
- **ARA-05 (orta):** kütüphane tablosu `role="grid"` içinde hücre rolü yok; seçili satır ekran okuyucuya
  bildirilmez.
- **YAY-07 (orta):** `embedBootstrapper` WebView2 yoksa internetten indirir; "hiçbir zaman internete bağlanmak
  zorunda değildir" vaadiyle çelişir (karar: Bölüm 5.3).
- **YAY-09 (orta):** CI'daki "gerçek ses aygıtıyla deneme" adımı runner'da aygıt olmadığı için hep ATLANDI ile yeşil.
- **YAY-11 (orta):** `strip = true` ile çökme günlüğündeki yığın izi çözümsüz.
- **SES-11 (düşük):** README ve CLAUDE.md "crossfeed" var diyor; kodda yok (yol haritası maddesi).
- **MAT-11 (düşük, olası):** Equalizer APO "LS/HS" (Q yazılmayan raf) için Q = 0,707 varsayımı. Not: başka bir
  oturumun açık PR'ı #83 aynı noktayı "S = 0,9" diye düzeltiyor; iki kaynak bağımsızca aynı yere işaret etti.

### 4.3 Ayrışmalar ve nüanslar

- **Nabız garantisi (A4 ↔ ANL-08/MAT-10):** bağımsız denetçiler garantinin nabız ve vurgu için ayrı ayrı
  seyreltildiğini, birleşik kanalın 60–200 BPM'de en kötü tam 3 olay/sn'ye geldiğini ölçtü ("sınır aşılmıyor ama
  rastlantıyla"). #85 vurguları nabzın alt kümesi yaptı: birleşik kanal artık yapısal olarak nabız kanalıyla aynı.
  MAT-10'un "warmth drop anında 0,15 sıçrar" notu küçük ve açık.
- **Yarıda kesilen analiz (A2 ↔ ANL-16):** ilk denetim "saklanmasın" dedi, #82 öyle yaptı; bağımsız denetçi tersini
  gördü: bozuk kuyruklu dosya her açılışta baştan analiz edilir. İkisi de doğru; çözüm "eksik" bayrağıyla saklama
  (Y4).
- **Önem ayrışması:** C6/YAY-03 (commit izlenebilirliği) ve U10/ARA-01 (boşta çizim) ilk denetimde orta/düşüktü;
  bağımsız denetçiler "yüksek" dedi ve gerekçeleri ikna edici (doğrulama haftasında hata raporları derlemeye
  bağlanamıyor; dahili GPU'da pil ve ısı). Bu belge onların derecesini benimser.
- **Puanlar:** ilk denetim puan vermemişti; bağımsız denetim 6–8/10. En düşük puan yayın hattına (6/10): oradaki
  bulguların çoğu PR 4–5'te hazır; birleştikten sonra yeniden puanlanabilir.

## 5. Birleşik öncelik sırası (yeni iş)

İlk denetimin sırası (veri güvenliği → çalma → görsel → yayın → lisans → cihazda ölçüm → belgeler) korunur; bağımsız
denetimin bulguları bu sıraya yerleştirilir. Hepsi "bulunan hata" sınıfındadır (doğrulama haftası kuralıyla uyumlu);
spektrum sürümünü artıran ANL-01/02 ve tasarım kararı gerektirenler ayrıca işaretlendi.

### 5.1 Hemen (PR 7–9 adayları; her biri test-önce)

1. **KUT-01** — boş görünen kaynakta toplu silmeye eşik ve uyarı ("X şarkı bulunamadı; klasör taşınmış ya da disk
   değişmiş olabilir — kütüphaneden çıkarılmadı"); KUT-05 ile birlikte (`$RECYCLE.BIN`, gizli/sistem klasörleri
   atlanır ve "silinmedi" listesine girer). KUT-07'nin testleri (yanlış silme, iptal).
2. **SES-01** — varsayılan aygıt değişimini izleme ve kayıpta yeniden açma (`IMMNotificationClient` ya da yoklama).
   Gerçek cihazda doğrulanacak (DOGRULAMA'ya madde).
3. **SES-02 / KUT-08 / ANL-06 / A9** — şarkı açma ve sarmanın kilit dışına taşınması (ABX'te yapıldığı gibi
   `spawn_blocking`); önbellek kilidini oynatıcı kilidinden ayırmak.
4. **ARA-01 / U10** — boşta ve duraklatmada çizimi düşürmek (son kareyi tut, RAF'ı durdur),
   `failIfMajorPerformanceCaveat`, kare süresi bekçisi; ARA-03 (Enter hatası, küçük, kesin); ARA-04 (üst çubuğa
   hata sınırı).
5. **YAY-03 / C6** — commit kimliğini programa gömmek (Ayarlar > Program'da "Sürüm 0.0.48 · a1b2c3d") ve sürüm
   notlarına SHA-256 yazmak; YAY-09 (ses denemesi aygıt yoksa "atlandı" değil açıkça raporlansın); YAY-11 (ayrı
   sembol dosyası, `strip = "debuginfo"` yerine `split-debuginfo`).
6. **MAT-01** — sınırlayıcıya örnekler arası tepe (4× ara değerleme) ve −1 dBTP tavanı; **MAT-03** (kulaklık ön
   kazancı gerçek örnekleme hızıyla).

### 5.2 Sonra

- **ANL-01/02/17/18** — spektrum pencere boyutunu süreye bağlamak, bant tanımı; tek seferde, spektrum sürümü artar
  (bütün kütüphane yeniden analiz edilir; kullanıcıya söylenmeli).
- **ANL-03 / A13** — spektral kare okumasını pencere merkezine göre kaydırmak (±20 ms bütçesinin yarısı).
- **ANL-04 / A7 / Y5** — tempo değişimi: "değişken tempo" bayrağı, ortanca BPM, bölüm bölüm adım.
- **MAT-02, MAT-04** — EQ uç bantlarını raf yapmak ya da belgeyi düzeltmek; alt oktav seviyesini frekanstan
  bağımsızlaştırmak.
- **KUT-03** — büyük, yeniden üretilebilir veriyi `app_local_data_dir`'a taşımak (bir kerelik göçle);
  **KUT-06** (okuma hatasında üzerine yazmama); **KUT-04 / K11 / ARA-14** (aramaya `limit`, tarama sırasında
  yenileme sınırı).
- **ARA-05/06 / U5** — grid semantiği, odak yönetimi, sekmelerde ok tuşları; **ARA-07 / U9** — güvenli modun ve
  yedek çizimin parlama ölçümüne alınması.
- **SES-04, SES-05, SES-06 / S11, SES-07/09/13**, **A8 / ANL-16 / Y4**, **K12 / Y8**, **SES-11** (belge).

### 5.3 Proje sahibinin kararını bekleyenler

| Konu                                    | Seçenekler                                                                                                                                               | Öneri                                                                                                       |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Bozuk veritabanı (K7 / KUT-02)          | (a) dosyaya dokunma, hata göster; (b) `library.sqlite3.bozuk-<tarih>` adıyla kenara al, boş kur, kullanıcıya söyle                                       | **(b)**: kütüphane ve analiz yeniden üretilebilir veridir; yazılımcı olmayan kullanıcı elle dosya silmemeli |
| Ses iş parçacığı önceliği (S4 / SES-03) | (a) yapma; (b) MMCSS "Pro Audio" için yalnızca `wasapi.rs`'te gerekçeli `unsafe` (CLAUDE.md kuralı: sahibine bildirilerek)                               | **(b)**, Faz 3'te; "1 saat çalmada sıfır takılma" hedefi için gerekli                                       |
| WebView2 kurulumu (YAY-07)              | (a) `embedBootstrapper` kalsın, belgeye "WebView2 yoksa kurulum bir kez internet ister" yazılsın; (b) `offlineInstaller` (kurulum dosyası ~150 MB büyür) | **(a)** şimdilik; Windows 10/11'de WebView2 hazır gelir, program çalışırken ağa çıkmaz                      |
| Gerçek şarkılarla doğruluk (ANL-07)     | işaretleme aracıyla (ISARETLEME.md) 10–20 şarkı işaretlenip F-ölçüsü ölçülsün                                                                            | doğrulama haftasının parçası; sonuçlar DOGRULAMA'ya                                                         |

## 6. Sınırlar ve dürüstlük notu

- Bağımsız denetçiler aynı model ailesindendir; bağımsızlık bağlam ve bilgi düzeyindedir, kurumsal değil. Ortak
  kör noktaları olabilir (ör. gerçek aygıt davranışı, gerçek müzik).
- Bölüm B'nin bulguları denetçilerin kendi kanıtlarına dayanır. Bu belgeyi derleyen taraf şu bulguları ayrıca kodda
  doğruladı: KUT-01, KUT-03, KUT-13(b), ARA-02, ARA-08, YAY-01…04, YAY-06, YAY-08, YAY-10, YAY-12/13, MAT'ın "doğru"
  dediği formüller (ilk denetimle örtüşür). Doğrulanmayanlar denetçinin kanıtıyla sunulmuştur; her birinin dosya:satır
  adresi raporda vardır.
- Linux'ta yapıldı; WASAPI (SES-01, S3), kurulum dosyası ve Edge'de sahneler yalnızca kod okuması ve CI'daki Windows
  işleriyle sınandı. Gerçek kulaklık/hoparlörle doğrulama `DOGRULAMA.md` B10–B12, D10–D11'de proje sahibini bekliyor.
- Yayın denetçisinin "WASAPI testleri `main`'de hiç koşmuyor" (YAY-02) gözlemi PR 4'ten sonra da geçerlidir: Windows
  işi yalnızca PR'da çalışır; test sürümü PR'da yeşil görülen kodu derler. Ayrı bir `main` Windows işi istenirse
  eklenebilir (dakikalar maliyeti).

## 7. Ekler

- `docs/denetim-2/BRIEF.md` — denetçilere verilen ortak kural metni (bağımsızlık, yöntem, biçim).
- `docs/denetim-2/ses.md`, `analiz.md`, `kutuphane.md`, `arayuz.md`, `yayin.md`, `matematik.md` — tam raporlar.
- `docs/denetim-2/hesap/*.py` — matematik denetçisinin doğrulama betikleri (`python3 -I` ile yeniden üretilebilir:
  RBJ, 10 bant EQ, BS.1770, sınırlayıcı, rubato, bas, beats/director, Camelot/binom, peq hız).
- `docs/denetim-2/kapilar-ozet.tsv` — yayın denetçisinin bağımsız kalite kapısı koşusu (kapı, çıkış kodu, süre).
