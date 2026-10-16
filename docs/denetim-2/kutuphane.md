# Kütüphane, depolama, komutlar, ayarlar ve günlük — Bağımsız denetim raporu

Denetçi: bağımsız denetçi kütüphane/depolama. Kaynak: main @ 25f83f8. Tarih: 16 Ekim 2026.

## 1. Kapsam ve yöntem

İncelenen dosyalar (hepsi `(depo kökü)/src-tauri/`), satır sayılarıyla:

| Dosya                                                                       | Satır | İçerik                                              |
| --------------------------------------------------------------------------- | ----- | --------------------------------------------------- |
| `src/library/mod.rs`                                                        | 123   | Hata türleri, `path_covers`, `search_key`           |
| `src/library/db.rs`                                                         | 734   | SQLite erişimi, arama, güncelleme                   |
| `src/library/scan.rs`                                                       | 442   | Klasör yürüyüşü, paralel etiket okuma, silme kararı |
| `src/library/service.rs`                                                    | 513   | Arka plan tarama iş parçacığı, durum, sürükle-bırak |
| `src/storage.rs`                                                            | 881   | Şema, 7 göç adımı, önbellek onarımı, 14 test        |
| `src/commands.rs`                                                           | 768   | 42 Tauri komutu                                     |
| `src/lib.rs`                                                                | 164   | Açılış sırası, komut kaydı, kapanış                 |
| `src/settings.rs`                                                           | 229   | `settings.json` okuma/yazma                         |
| `src/diagnostics.rs`                                                        | 233   | `logs/lyraska.log`, çökme yakalayıcı                |
| `src/main.rs`, `build.rs`                                                   | 9     | Giriş noktası                                       |
| `capabilities/default.json`, `tauri.conf.json`, `Cargo.toml`, `clippy.toml` | —     | İzinler, CSP, lint ayarları                         |

Bağlam için okunanlar (bulgu üretmek için değil, kapsamdaki kodun çağırdıklarını doğrulamak için):
`analysis/cache.rs` (aynı veritabanını kullanan ikinci bağlantı, `FileStamp`), `analysis/annotation.rs`
(işaret dosyası yazımı), `audio/decode.rs` (`Decoder::open` ve `TrackInfo.path`), `audio/player.rs` ve
`audio/output/wasapi.rs` (günlüğün gerçek zamanlı yoldan çağrılıp çağrılmadığı), `src/lib/backend.ts` ve
`src/hooks/useLibrary.ts` (komutların arayüzden nasıl çağrıldığı), `docs/MIMARI.md`, `CLAUDE.md`,
`.github/ISSUE_TEMPLATE/hata-kaydi.yml`, bağımlılık kaynakları `rusqlite-0.40.2/src/error.rs` ve
`walkdir-2.5.0/src/dent.rs`.

Çalıştırılanlar: derleme/test yok (izin verilmedi). Hafif denetimler: `python3 -I` ile SQLite 3.45.1 üzerinde
göç 4'ün `GLOB '*[ÐÝÞðýþ]*'` kalıbının UTF-8 ile doğru çalıştığı ve bir işlem içindeki DDL'nin (`ALTER TABLE`)
`ROLLBACK` ile geri alındığı doğrulandı; test modülleri dışındaki `unwrap/expect/panic` kullanımları betikle
sayıldı.

Dayanılan standart ve kurallar: SQLite WAL/`synchronous`/`user_version` belgeleri, Windows uygulama veri
klasörü kuralları (Roaming ve Local AppData ayrımı), Tauri 2 güvenlik modeli (capabilities, CSP),
`CLAUDE.md`'deki proje kuralları (gerçek zamanlı yolda günlük yok, `unwrap` yasağı, göçler yalnızca sona eklenir).

## 2. Genel değerlendirme

**Puan: 7/10.** Veri katmanı dikkatle yazılmış: göçler tek bir `BEGIN IMMEDIATE` işlemi içinde, yarıda
kesilirse bütünüyle geri alınıyor; geçmişteki her sürümün bıraktığı dosya biçimi test ediliyor; SQL'de parametre
dışında kullanıcı verisi yok; `unwrap` yasağı kütüphane kodunda gerçekten uygulanmış (test modülleri dışında
yalnızca `#[cfg(test)]` yardımcılarında var). En zayıf yan, "kötü gün" senaryoları: var olan ama boş görünen bir
kaynak klasör bütün şarkıları ve saatlerce süren analizleri sessizce siler; bozulmuş bir veritabanı dosyasından
programın kendi başına çıkış yolu yok. Bir numaralı risk **KUT-01**: harici diskin harfi değişince ya da bir
bağlama noktası boş kalınca kaynağın tüm şarkılarının ve analizlerinin tek taramada silinmesi.

## 3. Bulgular (önem sırasıyla)

### [KUT-01] Var olan ama boş (ya da içeriği değişmiş) kaynak klasör bütün şarkıları ve analizlerini siler

- Önem: Yüksek
- Güven: kesin (kodla kanıtlandı)
- Yer: `src/library/scan.rs:141-142`, `:153`, `:227`, `:235-245`; `src/storage.rs:69-70`
- Kanıt: Tek koruma kökün var olmasıdır:
  ```rust
  if !(root.is_dir() || root.is_file() && is_audio(root)) {
      return Err(LibraryError::FolderMissing(folder.path.clone()));
  }
  ```
  Kök bir klasörse ve içinde hiç ses dosyası yoksa `files` boş, `unreadable` boş kalır;
  `removal_candidates` bilinen **bütün** yolları döndürür ve `apply_changes(folder.id, &batch, &removed)`
  hepsini siler. Tetikleyici `analyses_follow_tracks` (`storage.rs:69`) her silinen şarkının analizini de
  siler. Hiçbir "şarkı sayısı sıfıra düştü" eşiği yok.
- Etki: Gerçek hayattaki durumlar: (a) kaynak bir sürücü kökü (`E:\`) ve USB bellek takılınca `E:` harfini
  başka bir disk alır — klasör "vardır", müzik yoktur; (b) NTFS bağlama noktası klasörü (bölüm bağlı değilken
  boş klasör); (c) bulut eşitleme istemcisinin boşalttığı klasör. Programın her açılışında tam tarama
  çalıştığı için (`lib.rs:47`) kullanıcı fark etmeden kütüphane listesi ve **saatlerce süren analiz sonuçları**
  gider. Dosyalar silinmez ama ürünün temel vaadi olan "şarkıyı önceden bilme" verisi kaybolur ve yeniden
  hesaplanması büyük kütüphanede saatler sürer.
- Yeniden üretme / önerilen test: `scan.rs` testlerine `bos_gorunen_kok_sarkilari_silmez`: klasörü tara, 4
  şarkı eklensin; şarkıları klasörden taşı (klasör kalsın, boş); yeniden tara; şarkı sayısının korunmasını ve
  `problems` içinde uyarı olmasını bekle.
- Önerilen düzeltme: Silme kararına eşik koy: bilinen şarkıların tamamı (ya da örneğin > %50'si) "yok"
  görünüyorsa silme, `ScanSummary`'ye `suspicious_removals` ekle ve `problems` içinde "X şarkı bulunamadı;
  klasör taşınmış ya da disk değişmiş olabilir — kütüphaneden çıkarılmadı" yaz; kullanıcı isterse klasörü
  kaldırır. Alternatif: analizleri hemen silmeyen, "yetim" işaretleyip 30 gün sonra temizleyen bir yol.

### [KUT-02] Bozuk veritabanı dosyasından kurtulma yolu yok; kütüphane her açılışta kapalı kalır

- Önem: Yüksek
- Güven: kesin
- Yer: `src/library/db.rs:62-95`, `src/lib.rs:38-50`, `src/commands.rs:122-134`, `src/storage.rs:238-245`
- Kanıt: `Library::open` → `init` → `PRAGMA journal_mode = WAL …` ve `storage::migrate`. Dosya SQLite değilse
  ya da sayfaları bozuksa hata `LibraryError::Database` olarak döner; `lib.rs:48` yalnızca günlüğe yazar ve
  `LibraryState::new(Err(..))` ile saklar; her kütüphane komutu aynı metni döndürür. `is_schema_mismatch`
  bozulmayı (SQLITE_CORRUPT, SQLITE_NOTADB) bilerek dışarıda tutar (doğru), ama bunun ardından bir onarım
  adımı yok: `PRAGMA quick_check` yok, dosyayı `.bozuk` adıyla kenara alıp yeni dosya kurma yok (ayarlar için
  `settings.rs:95-96`'da bu var).
- Etki: Elektrik kesintisi, dolu disk ya da antivirüs müdahalesiyle dosya bozulduğunda kullanıcı her açılışta
  "Kütüphane veritabanı hatası: file is not a database" (İngilizce SQLite metni) görür; yazılımcı olmayan proje
  sahibi `%APPDATA%\io.github.gozlemesevdalisi.lyraska\library.sqlite3` dosyasını elle silmeden
  kurtulamaz. Kütüphane ve analiz **yeniden üretilebilir** veri olduğundan bu kapanma gereksiz.
- Yeniden üretme / önerilen test: `db.rs` testi `sqlite_olmayan_dosya_kenara_alinir_ve_yeni_kutuphane_acilir`:
  yola 1 KB rastgele bayt yaz, `Library::open` çağır, başarı ve `library.sqlite3.bozuk` dosyasının varlığını
  bekle.
- Önerilen düzeltme: `Library::open`'da hata `SQLITE_CORRUPT`/`SQLITE_NOTADB` ise dosyayı (ve `-wal`, `-shm`
  eşlerini) `library.sqlite3.bozuk-<tarih>` adıyla taşı, günlüğe yaz, boş dosyayla yeniden dene; arayüzde
  "Kütüphane dosyası bozuktu, yeniden kuruldu; klasörlerinizi yeniden ekleyin" mesajı göster. Açılışta
  `PRAGMA quick_check` (küçük dosyada hızlı) ile erken yakala.

### [KUT-03] Analiz önbelleği ve günlükler Roaming AppData'da: gigabaytlık, yeniden üretilebilir veri gezici profile yazılıyor

- Önem: Orta
- Güven: yüksek
- Yer: `src/lib.rs:34-35`, `:43`, `:59`, `:74`, `:82`; `src/commands.rs:337`; `src/library/mod.rs:11`
- Kanıt: Her kalıcı veri `app.path().app_data_dir()` altına yazılır; Tauri 2'de bu Windows'ta
  `%APPDATA%` (Roaming) klasörüdür — modül belgesi de bunu söyler (`mod.rs:11`). `analyses` tablosu şarkı
  başına 60 kare/sn × 32 bant seviyeleri, başlangıç gücü ve kroma blob'ları saklar
  (`cache.rs:248`, `spectrogram.rs:27-29`); delta + deflate sıkıştırmayla bile şarkı başına yüz KB
  mertebesinde, 10 000 şarkıda GB'lar.
- Etki: Etki alanı (domain) ve gezici profil kullanan bilgisayarlarda oturum açma/kapama yavaşlar, profil
  kotası dolar; OneDrive "Klasör Yedekleme" gibi araçlar Roaming'i izlemez ama kurumsal gezici profiller izler.
  Windows kuralı: makineye özgü, yeniden üretilebilir büyük veri `%LOCALAPPDATA%`'ya yazılır.
- Yeniden üretme / önerilen test: Bir lib.rs birim testi zor; en azından `docs/MIMARI.md` "Veri ve kalıcılık"
  tablosunda konumun gerekçesi yazılmalı.
- Önerilen düzeltme: `library.sqlite3`, `logs/` ve `kalibrasyon/` için `app_local_data_dir()` kullan;
  `settings.json` ve `isaretler/` (küçük, kullanıcıya ait) Roaming'de kalabilir. Geçişte eski konumdaki
  dosya varsa bir kez taşı (yoksa kullanıcı kütüphanesini "kaybetmiş" görür).

### [KUT-04] Arama komutu sınırsız çağrılıyor: tarama sürerken her 600 ms'de bütün kütüphane (50 000 satıra kadar) IPC'den geçiyor

- Önem: Orta
- Güven: kesin
- Yer: `src/lib/backend.ts:336-338`, `src/hooks/useLibrary.ts:21`, `:85-94`; `src/library/db.rs:14`,
  `src/library/service.rs:99-101`
- Kanıt: `searchLibrary(query)` komutu `limit` göndermez → `limit.unwrap_or(MAX_RESULTS)` = 50 000.
  Arama etkisi `status.trackCount` ve `analysisKey` değiştikçe yeniden çalışır; tarama sürerken durum her
  600 ms sorgulanır ve her 200 şarkılık toplu yazımda `trackCount` değişir. Her seferinde `TrackRow`
  (11 alan, tam yol dahil) JSON'a çevrilip WebView'a geçer.
- Etki: 20 000 şarkılık kütüphanede her yenileme yaklaşık 4–6 MB JSON ve ana iş parçacığında ayrıştırma;
  tarama boyunca saniyede birkaç kez. Arayüz takılır, görsel kare hızı düşer (1080p 60 fps hedefiyle çelişir).
  Ayrıca `library_status` her çağrıda `analyzed_count` için `tracks ⋈ analyses` JOIN'ini sayar
  (`db.rs:178-190`); 600 ms'de bir makul ama aynı dakikada yüzlerce kez tekrarlanır.
- Yeniden üretme / önerilen test: Vitest'te `useLibrary` için "tarama sürerken arama en fazla N kez
  çağrılır" testi; Rust'ta `search` için sayfalama (`offset`) testi.
- Önerilen düzeltme: Arayüzden `limit` gönder (listede görünen kadar + sayfalama ya da sanal liste); tarama
  sürerken arama yenilemesini `trackCount` yerine zamanla (ör. 2 s) sınırla; `library_status` için
  `analyzed` sayısını taramada/analizde artımlı tutulan bir sayaçtan ver.

### [KUT-05] Sürücü kökü eklenince Geri Dönüşüm Kutusu ve gizli/sistem klasörleri de taranıyor

- Önem: Orta
- Güven: yüksek
- Yer: `src/library/scan.rs:250-270`, `:273-279`
- Kanıt: `find_audio_files` yalnızca `follow_links(false)` ile yürür; gizli (`FILE_ATTRIBUTE_HIDDEN`),
  sistem ya da `$RECYCLE.BIN`, `System Volume Information`, `.`-ile-başlayan klasörler için hiçbir dışlama
  yok. `is_audio` yalnızca uzantıya bakar.
- Etki: Kullanıcı `D:\`'yi kaynak olarak eklerse (sürükle-bırak ile kolay) silip Geri Dönüşüm Kutusu'na
  attığı şarkılar `$RXXXXXX.mp3` adlarıyla kütüphanede görünür, analiz edilir, çalınır; kutu boşaltılınca
  KUT-01'deki silme çalışır. Ağ paylaşımlarında `.snapshot`, `@eaDir` (Synology küçük resimleri; içinde ses
  yoktur ama binlerce dosya vardır) gereksiz yürünür.
- Yeniden üretme / önerilen test: `scan.rs` testi `gizli_ve_sistem_klasorleri_atlanir`: `$RECYCLE.BIN`
  adlı alt klasöre şarkı koy, taramada sayılmadığını bekle.
- Önerilen düzeltme: `WalkDir::filter_entry` ile adı `$RECYCLE.BIN`, `System Volume Information`,
  `@eaDir`, `.`-ile-başlayan klasörleri ve Windows'ta gizli+sistem öznitelikli klasörleri atla
  (`std::os::windows::fs::MetadataExt::file_attributes`). Atlananları `unreadable` gibi "silinmedi"
  listesine ekle ki eski kayıtlar kaybolmasın.

### [KUT-06] Ayar dosyası geçici olarak okunamazsa sessizce varsayılana dönülüyor ve ilk kayıtta üzerine yazılıyor

- Önem: Orta
- Güven: kesin
- Yer: `src/settings.rs:43-58`, `:73-83`, `:134-147`
- Kanıt:
  ```rust
  let settings = match std::fs::read_to_string(path) {
      Ok(text) => parse_lenient(&text, path),
      Err(_) => Settings::default(),   // NotFound ile PermissionDenied/paylaşım ihlali ayrılmıyor, günlük yok
  };
  ```
  Ardından herhangi bir `update` (ör. ekolayzer kaydırıcısına dokunmak, `commands.rs:296`) `save` ile
  dosyanın üzerine varsayılan+tek değişiklik yazar.
- Etki: Dosya bir yedekleme/eşitleme aracı tarafından kilitliyken (Windows'ta paylaşım ihlali) açılan program
  ekolayzer, kulaklık profili (kullanıcının içe aktardığı AutoEq dosyası), güvenli mod ve gecikme ayarlarını
  kaybeder; ilk ayar değişikliğinde kayıp kalıcı olur. JSON bozuksa `.bozuk` yedeği alınırken okuma hatasında
  alınmıyor; iki yol tutarsız.
- Yeniden üretme / önerilen test: `settings.rs` testi `okunamayan_dosya_ustune_yazilmaz`: dosyayı salt
  klasörde/okuma izinsiz oluştur (Unix'te `chmod 000`), `open` sonrası `update` çağır, dosyanın
  değişmediğini ya da `.bozuk` yedeğinin alındığını bekle.
- Önerilen düzeltme: `ErrorKind::NotFound` dışındaki hatalarda günlüğe yaz ve ya `path: None` ile bellek
  modunda çalış (bu oturumda kaydetme) ya da ilk kayıttan önce eski dosyayı `.bozuk` adıyla yedekle.

### [KUT-07] Test boşlukları: yanlış silme, iptal, bozulma ve ağ koşulları ölçülmüyor

- Önem: Orta
- Güven: kesin
- Yer: `src/library/scan.rs:287-442`, `src/library/service.rs:279-513`, `src/storage.rs:286-881`,
  `src/settings.rs:149-229`
- Kanıt: Var olan testler mutlu yolu ve belgelenmiş eski dosya biçimlerini iyi kapsıyor. Olmayanlar:
  (a) kök var ama boş → silme (KUT-01); (b) `ScanState::cancel` çağrılınca yarım kalan toplu yazımın ve
  `removed` listesinin ne olduğu (`scan.rs:185-189`: işçiler durur, yazıcı yine `apply_changes(..., &removed)`
  çalıştırır — doğru ama test yok); (c) bozuk veritabanı dosyası (KUT-02); (d) sembolik bağ/bağlantı noktası
  döngüsü (`follow_links(false)` doğru, ama test yok); (e) `settings.update` eşzamanlı çağrı; (f) göçün
  gerçekten dosya tabanlı veritabanında yarıda kesilmesi (bellek içi `kutuphane_tablosunda_takilirsa…` var,
  süreç ölümü simülasyonu yok); (g) `pending`/`worker_running` yarış penceresi (`service.rs:229-234`) —
  mantık okumaya göre doğru, test yalnızca 20 ardışık istek deniyor.
- Etki: En riskli davranışlar (veri silme kararları) regresyona açık; `MIMARI.md`'nin "elle hatırlanması
  gereken kural yoktur" ilkesi bu alanda karşılanmıyor.
- Önerilen test: Yukarıdaki (a)–(e) için birer test; (a) ve (c) öncelikli.
- Önerilen düzeltme: KUT-01 ve KUT-02 düzeltmeleriyle birlikte yazılmalı.

### [KUT-08] Şarkı açma, oynatıcı kilidi altında ve `async` komut içinde bloklayıcı çalışıyor; görseller ve durum sorguları bekliyor

- Önem: Düşük
- Güven: kesin
- Yer: `src/commands.rs:152-157`, `:209-211`, `:668-670`; `src/audio/player.rs:622-681`
- Kanıt: `open_track` `async fn` içinde `player.lock()?.load(&path, true)` çağırır: dosya tanıma
  (`probe`), MP4 dolgu okuma, analiz önbelleği sorgusu ve WASAPI aygıtı açma (yüzlerce ms) `Mutex<Player>`
  tutulurken ve tokio çalışan iş parçacığında senkron yapılır. `visual_frame` (her ekran karesi) ve
  `playback_status` aynı kilidi bekler. Buna karşılık `abx_start` ve `annotation_evaluate` doğru biçimde
  `spawn_blocking` kullanır (`commands.rs:498`, `:626`).
- Etki: Şarkı geçişinde görsel sahne ve ilerleme çubuğu birkaç yüz ms donar; aygıt yavaş açılırsa (Bluetooth)
  saniyeye yaklaşır. Tokio çalışanı bloklandığı için aynı anda gelen diğer komutlar da gecikebilir.
- Yeniden üretme / önerilen test: `player.rs` için "load sürerken `visual_now` kilidi beklemez" testi
  (hazırlığı kilit dışında yapan yeni yol üzerinde).
- Önerilen düzeltme: `Session::start`'ın kilitten bağımsız hazırlık kısmını (`probe`, `Decoder::open`,
  `mp4_trim`) `spawn_blocking` içinde kilitsiz yap; kilidi yalnızca oturumu takarken al (ABX'te yapıldığı
  gibi).

### [KUT-09] Tam tarama klasör klasör ayrı "tarama" olarak koşuyor: durum bayrağı ve sayaçlar klasörler arasında sıfırlanıyor

- Önem: Düşük
- Güven: kesin
- Yer: `src/library/service.rs:262-276`, `src/library/scan.rs:92-99`, `:111-131`
- Kanıt: `run_scan` her klasör için `scan::scan_folders(library, std::slice::from_ref(folder), scan)`
  çağırır; `scan_folders` başta `set_scanning(true)`, sonda `set_scanning(false)` yapar; `begin` `found` ve
  `processed`'i sıfırlar. Çok klasörlü `scan_folders` toplama mantığı (`total`) hiç kullanılmaz.
- Etki: Arayüzde "taranıyor" göstergesi klasörler arasında kısa süre söner; `useLibrary` `scanning`
  false görünce 600 ms'lik ilerleme sorgusunu durdurur ve yeniden başlatır; ilerleme sayacı her klasörde
  sıfırdan başlar ("1200/1200" sonra "3/8000"). Toplam ilerleme kullanıcıya hiç gösterilemez.
- Önerilen düzeltme: `run_scan` tek `scan_folders` çağrısı yapsın; `scan_folders` içinde klasör başına hata
  toplasın (`try_for_each` yerine `filter_map`), `ScanProgress`'e toplam klasör/şarkı sayısı eklensin.

### [KUT-10] Kilit zehirlenmesi kullanıcıya "Query is not read-only" olarak gösteriliyor

- Önem: Düşük
- Güven: kesin
- Yer: `src/library/service.rs:82-86`, `src/library/scan.rs:281-285`; `rusqlite-0.40.2/src/error.rs:336`
- Kanıt: `self.library.lock().map_err(|_| LibraryError::Database(rusqlite::Error::InvalidQuery))`;
  rusqlite'ın bu hatanın `Display`'i `"Query is not read-only"`. Kullanıcı şunu görür:
  "Kütüphane veritabanı hatası: Query is not read-only".
- Etki: Bir iş parçacığı kilit tutarken çökerse (sürüm derlemesinde `panic = "abort"` olduğu için nadir ama
  `debug`/test derlemesinde olur) kullanıcı anlamsız İngilizce bir metin görür; hata kaydında yanlış yöne
  götürür.
- Önerilen düzeltme: `LibraryError::Lock` türü ekle ("Kütüphane beklenmedik bir hatayla kilitlendi; programı
  yeniden başlatın"), `cache.rs`'teki `CacheError::Lock` ile aynı.

### [KUT-11] Veritabanı klasörü oluşturulamazsa hata "Klasör okunamadı" olarak çıkıyor

- Önem: Düşük
- Güven: kesin
- Yer: `src/library/mod.rs:31-32`, `src/library/db.rs:63-65`
- Kanıt: `#[error("Klasör okunamadı: {0}")] Io(#[from] std::io::Error)`; `Library::open` içindeki
  `create_dir_all(dir)?` aynı türe dönüşür. Kullanıcı "Klasör okunamadı: Access is denied (os error 5)"
  görür; müzik klasörünü sorun sanır.
- Önerilen düzeltme: `Io` için bağlam alan bir varyant (`DataDir(PathBuf, io::Error)`) ya da `map_err` ile
  "Uygulama veri klasörü oluşturulamadı: …".

### [KUT-12] Günlük döndürme garantisi belgeyle uyuşmuyor; zaman damgası UTC

- Önem: Düşük
- Güven: kesin
- Yer: `src/diagnostics.rs:7-9`, `:86-102`, `:144-151`
- Kanıt: Belge "en fazla ~2 MB yer kaplar" der; `rotate_if_large` içinde `rename` başarısız olursa
  (`lyraska.1.log` başka bir program tarafından paylaşım-silme izni olmadan açıksa) sonuç yok sayılır ve
  `write_line` aynı dosyaya yazmayı sürdürür; test (`:213-231`) bunu kasıtlı doğrular. Zaman damgası
  `format_utc` ile UTC yazılır; Türkiye'de kullanıcının gördüğü saatle 3 saat fark eder.
- Etki: Sınır aşımı nadir ama olası (ör. kullanıcı `lyraska.1.log`'u Word ile açık bırakmış). UTC, hata
  kaydında "saat 21:05'te oldu" ile günlükteki `18:05` eşleşmez; proje sahibinin kendisi de bunu yaşar.
- Önerilen düzeltme: Yedekleme başarısızsa `lyraska.2.log` gibi ikinci ada dene ya da dosyayı kes
  (`set_len(0)`); zaman damgasına yerel saat farkını ekle (Windows'ta `GetTimeZoneInformation` yerine saat
  dilimi kütüphanesi istenmiyorsa en azından `UTC` sözcüğü kullanıcının gözüne çarpsın — şu an var; hata kaydı
  şablonuna "günlük saatleri UTC'dir" notu eklenmeli).

### [KUT-13] Komut girişleri doğrulanmıyor: sınırsız dosya okuma, göreli yollar, NaN işaretler

- Önem: Düşük
- Güven: kesin (davranış), etkisi olası
- Yer: `src/commands.rs:400-421`, `:467-479`; `src/library/service.rs:108-115`
- Kanıt: (a) `headphone_import(path: String)` → `std::fs::read_to_string(path)`: boyut sınırı yok; 2 GB'lık
  yanlış dosya seçilirse bellek dolar, ikili dosyada hata metni İngilizce
  "stream did not contain valid UTF-8" (AutoEq dosyaları birkaç yüz bayttır). (b) `add_folder` göreli yolu
  (`"."`) kabul eder: `path.exists()` doğru döner, `folders.path` tablosuna `"."` yazılır; çalışma dizini
  değişince KUT-01 tetiklenir. (c) `annotation_save(beats: Vec<f64>)` NaN/∞ denetlemez; `serde_json` NaN'ı
  `null` yazar, geri okuma `Vec<f64>` için başarısız olur → işaretler bir sonraki `load`'da kaybolur.
  Tehdit modeli: arayüz paketli ve CSP `default-src 'self'` olduğu için uzaktan kötüye kullanım yolu yok;
  bunlar sağlamlık bulgularıdır.
- Önerilen düzeltme: (a) 1 MB üstü dosyayı reddet, UTF-8 hatasını Türkçe çevir; (b) `path.is_absolute()`
  zorunlu kıl; (c) `beats`/`drops` için `is_finite()` ve `>= 0` süzgeci.

### [KUT-14] Büyük/küçük harf farklı aynı yol iki ayrı kayıt olabilir; analiz tetikleyiciyle silinir

- Önem: Düşük
- Güven: olası
- Yer: `src/storage.rs:37`, `:57`; `src/library/mod.rs:54-64`; `src/library/scan.rs:149-153`
- Kanıt: `tracks.path UNIQUE` ve `analyses.path PRIMARY KEY` bayt düzeyinde karşılaştırılır; `path_covers`
  Windows'ta harf duyarsızdır ama tarama `on_disk`/`known` eşlemesi (`scan.rs:149-163`) duyarlıdır. Tek
  şarkı kaynağı `d:\muzik\a.mp3` olarak eklenip sonra `D:\Muzik` klasörü eklenince `replace_folders` şarkıyı
  taşır; klasör taramasında yürüyüş `D:\Muzik\a.mp3` üretir → eski kayıt "silindi", analizi tetikleyiciyle
  gider, dosya yeniden eklenip yeniden analiz edilir.
- Etki: Nadir (Gezgin ve dosya penceresi gerçek harf biçimini verir); olursa tek şarkının analizi yeniden
  yapılır. Veri kaybı yok.
- Önerilen düzeltme: Yolları saklamadan önce Windows'ta `std::fs::canonicalize` ya da en azından sürücü
  harfini büyük harfe çevirip bileşenleri gerçek diskteki biçimiyle al; `search`/eşlemede `COLLATE NOCASE`
  **kullanılmamalı** (UTF-8 Türkçe harflerde yanlış).

### [KUT-15] Değişiklik tespiti saniye çözünürlüğünde mtime + boyuta dayanıyor

- Önem: Düşük
- Güven: kesin
- Yer: `src/analysis/cache.rs:132-141` (kütüphane aynı kuralı kullanır: `scan.rs:265`)
- Kanıt: `d.as_secs() as i64`; 1970 öncesi ya da okunamayan mtime `0`. Aynı saniye içinde aynı boyutta
  değiştirilen dosya (ör. etiket düzenleyicinin sabit dolgu alanına yazması + aynı saniyede kaydetme) atlanır.
  Analiz önbelleği de aynı damgayı kullandığı için yanlış şarkının analiziyle çalma kuramsal olarak olasıdır.
- Etki: Çok nadir; etiket düzenleyiciler genellikle boyutu değiştirir ya da farklı saniyeye düşer.
- Önerilen düzeltme: Damgaya nanosaniye (Windows 100 ns) ekle ya da `modified`'ı milisaniye sakla (yeni
  göç adımı gerekmez: aynı sütun, yeni ölçek yalnızca `TAGS_VERSION`/parça sürümleriyle yeniden damgalanır).

### [KUT-16] WAL + `synchronous = NORMAL`: elektrik kesintisinde son işlemler geri gider; `-wal` dosyası ve boş sayfalar geri alınmaz

- Önem: Düşük
- Güven: kesin
- Yer: `src/library/db.rs:89-91`, `src/analysis/cache.rs:173`
- Kanıt: SQLite belgelerine göre WAL + NORMAL'da işlemler **tutarlı** kalır ama güç kesilirse en son
  denetim noktası sonrası işlemler kaybolabilir — kütüphane için uygun bir seçim (yeniden tarama kurtarır).
  Ancak `wal_autocheckpoint` varsayılanda (1000 sayfa) kalıyor, `auto_vacuum` kapalı ve hiçbir yerde
  `VACUUM`/`PRAGMA optimize` yok; büyük klasör çıkarılınca analiz blob'larının boşalttığı sayfalar dosyada
  kalır (dosya hiç küçülmez).
- Etki: Performans ve disk alanı; veri bütünlüğü sorunu değil.
- Önerilen düzeltme: Kapanışta (`RunEvent::Exit`) `PRAGMA optimize` ve `wal_checkpoint(TRUNCATE)`; büyük
  silmelerden sonra arka planda `VACUUM` (ya da `auto_vacuum = INCREMENTAL` ilk kurulumda).

### [KUT-17] CSP'de `style-src 'unsafe-inline'`; izinler aksi halde dar

- Önem: Düşük
- Güven: kesin
- Yer: `src-tauri/tauri.conf.json` `app.security.csp`; `capabilities/default.json`
- Kanıt: `default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline';
connect-src ipc: http://ipc.localhost`. Satır içi stil izni React `style={{…}}` için gerekiyor olabilir
  ama CSS değişkenleriyle (`:root`) çoğu durum çözülür. İzinler `core:default` + `dialog:allow-open`
  ile en az ayrıcalık ilkesine yakın; `shell`, `fs`, `http` eklentisi yok (iyi).
- Etki: Uzak içerik yüklenmediği için pratik risk çok düşük; yine de Tauri'nin önerdiği en sıkı CSP'den
  bir adım geride.
- Önerilen düzeltme: Satır içi `style` kullanımlarını sınıf/`--css-var` ile değiştir ve `'unsafe-inline'`
  kaldır; mümkün değilse gerekçeyi `tauri.conf.json` yanına belgele.

### Şüpheler (kanıtlanamayan, güven "olası")

- **[KUT-Ş1] Bulut yer tutucu dosyalar taramada indiriliyor olabilir.** OneDrive "İsteğe Bağlı Dosyalar"
  yer tutucuları reparse point'tir ama Rust `is_symlink()` yalnızca SYMLINK/MOUNT_POINT etiketlerini sayar;
  `walkdir` onları düz dosya olarak verir ve `Decoder::open` okumaya başlayınca OneDrive dosyayı indirir
  (`scan.rs:194`). Müzik klasörü OneDrive'daysa ilk tarama bütün kütüphaneyi indirebilir. Windows'ta
  `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` (0x400000) öznitelikli dosyaları atlamak ya da uyarmak gerekir.
  Gerçek Windows ortamı olmadan doğrulanamadı.
- **[KUT-Ş2] Yavaş ağ yolunda `path.exists()`/`is_dir()` arayüzü uzun bekletir.** `add_folder`
  (`service.rs:110-115`) ve `scan_folder` (`scan.rs:141`) kopuk SMB yolunda 20–60 s bloklanabilir;
  `add_folder` `async` komutta tokio çalışanını tutar. Ölçülemedi.

## 4. Güçlü yanlar

- **Göçler gerçekten geri alınabilir ve eşzamanlılığa dayanıklı** (`storage.rs:181-230`): `BEGIN IMMEDIATE`
  ile yazma kilidi baştan alınır, adımlar `SAVEPOINT` içinde koşar, hata olursa dosyaya dokunulmaz;
  `kutuphane_tablosunda_takilirsa_dosyaya_dokunulmaz` ve `ayni_dosyayi_iki_baglanti_birlikte_acabilir`
  testleri bunu ölçer. SQLite'ın DDL'yi işlem içinde geri alabildiği python ile doğrulandı.
- **Geçmişin her dosya biçimi test ediliyor** (`every_past_file`, `storage.rs:571-615`): 0.0.5–0.0.25'in
  üç ayrı şeması ve "yarım göçle güncellenmiş" dosyalar dahil 14 varyant aynı `shape`'e ulaşıyor ve hiçbiri
  analiz silmiyor. Bu, bu büyüklükte bir hobi projesinde nadir görülen bir titizlik.
- **SQL enjeksiyonu yok**: kullanıcı verisi yalnızca `?N` parametreleriyle gidiyor; `LIKE` kalıbı
  `escape_like` ile kaçırılıyor ve testi var (`db.rs:517-519`). `format!` ile kurulan tek dinamik parça
  sabit sütun adları ve sayısal `LIMIT`.
- **Türkçe arama doğru kurulmuş**: `İ/ı/I/i` tek harfe, aksanlar sadeye indirgeniyor (`mod.rs:69-96`);
  SQLite `LIKE`'ın yalnızca ASCII'de harf duyarsız olduğu sorunu her iki tarafı önceden katlayarak aşılmış.
  Göç 4'ün `GLOB` kalıbının UTF-8 çok baytlı harflerle çalıştığı doğrulandı.
- **`unwrap`/`panic` yasağı uygulanıyor**: test modülleri dışında kalan tek kullanımlar `#[cfg(test)]`
  yardımcılarında (`db.rs:77,83`, `cache.rs:188`); `Cargo.toml` lint'leri ve `clippy.toml` testlerde
  serbest bırakıyor. Sürümde `panic = "abort"` ile çökme kancası uyumlu: kanca abort'tan önce çalışır ve
  `[ÇÖKME]` satırını yazar.
- **Günlük gerçek zamanlı yoldan çağrılmıyor**: `diagnostics::error` çağrıları ses çıkış iş parçacığında
  yalnızca döngü bittikten sonra (`wasapi.rs:148`) ve çözücü iş parçacığında (`player.rs:1287,1298`); WASAPI
  döngüsünün içinde yok.
- **Yarış koşullarına karşı düşünülmüş tarama servisi**: `worker_running` bayrağını bırakıp bekleyen isteği
  yeniden denetleme (`service.rs:229-234`), kaynak tarama sırasında çıkarılırsa yabancı anahtar hatası
  yerine sessiz bitiş (`scan.rs:212-216`), okunamayan alt klasörün altındakileri silmeme
  (`removal_candidates`). Hepsi testli.
- **Ayarlar atomik yazılıyor** (`settings.rs:134-147`: geçici dosya + `sync_all` + `rename`); alan alan
  hoşgörülü okuma (`parse_lenient`) ekolayzer ile kulaklık profilini birbirine bağımlı kılmıyor.
- **Hata kaydı şablonu gizliliği söylüyor** (`hata-kaydi.yml:87-90`: günlükte yol ve kullanıcı adı
  geçebileceği açıkça yazılı; şarkıların kendisi yok).

## 5. Denetlenemeyenler ve sınırlar

- Derleme ve test çalıştırma izni yoktu; bulgular kod okumasına ve SQLite davranışının python ile
  doğrulanmasına dayanır. `ayni_dosyayi_iki_baglanti_birlikte_acabilir` gibi zamanlamaya bağlı testlerin
  CI'da kararlı olduğu varsayıldı.
- Windows'a özgü davranışlar (Roaming/Local AppData konumu, reparse point türleri, `explorer /select`,
  dosya paylaşım kilitleri, mapped drive davranışı) Linux'ta doğrulanamadı; KUT-03 ve şüpheler bu nedenle
  "yüksek/olası" güvenle verildi.
- 10 000+ şarkılık gerçek kütüphaneyle performans ölçümü yapılamadı; KUT-04 kod yolundan türetilmiş
  hesaptır (satır sayısı × alan sayısı × yenileme sıklığı).
- `analysis/cache.rs`, `analysis/background.rs` ve oynatıcı yalnızca kapsamdaki kodun çağırdığı kadar
  okundu; onların kendi hataları bu raporun dışındadır.
- Tauri `core:default` izin kümesinin içeriği (`gen/schemas`) depoda üretilmiş durumda yoktu; Tauri 2
  belgelerindeki varsayılan içerik esas alındı.

## 6. Önerilen öncelik sırası

1. **KUT-01** — boş görünen kaynakta toplu silmeye eşik/uyarı (veri kaybını kesen en küçük değişiklik).
2. **KUT-02** — bozuk veritabanını kenara alıp yeniden kurma (kullanıcıyı elle müdahaleden kurtarır).
3. **KUT-07** — KUT-01/02 ile birlikte yazılacak testler (yanlış silme, bozulma, iptal).
4. **KUT-04** — aramaya `limit`/sayfalama ve tarama sırasında yenileme sınırı (büyük kütüphanede takılma).
5. **KUT-05** — `$RECYCLE.BIN`, gizli/sistem klasörlerini atlama.
6. **KUT-03** — büyük, yeniden üretilebilir veriyi `app_local_data_dir`'a taşıma (bir kerelik göçle).
7. **KUT-06** — ayar dosyası okuma hatasında üzerine yazmama.
8. **KUT-08, KUT-09** — şarkı açmayı kilit dışına alma; tam taramanın tek tarama olarak koşması.
9. **KUT-10, KUT-11, KUT-12, KUT-13** — hata metinleri, günlük döndürme/saat, giriş doğrulama.
10. **KUT-14, KUT-15, KUT-16, KUT-17** — iyileştirme fırsatları; bir bakım oturumunda topluca.
