# Yayın, CI, tedarik zinciri, lisans uyumu ve kalite kapıları — Bağımsız denetim raporu

Denetçi: bağımsız denetçi (yayın ve tedarik zinciri). Kaynak: main @ 25f83f8. Tarih: 16 Ekim 2026.

## 1. Kapsam ve yöntem

**İncelenen dosyalar** (hepsi `(depo kökü)/` altında, satır satır okundu; toplam ~3 500 satır + iki
kilit dosyası ~9 100 satır):

- CI ve yayın: `.github/workflows/ci.yml` (166 satır), `.github/workflows/test-surumu.yml` (97),
  `.github/ISSUE_TEMPLATE/hata-kaydi.yml`, `config.yml`
- Tauri: `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/build.rs`,
  `src-tauri/.cargo/config.toml`, `src-tauri/clippy.toml`
- Bağımlılık ve lisans: `src-tauri/Cargo.toml`, `Cargo.lock` (479 paket), `deny.toml`, `package.json`,
  `package-lock.json` (274 paket), `LICENSE`, `docs/LISANSLAR.md`, `public/lisanslar/`
- Betikler: `scripts/check-commands.mjs`, `check-licenses.mjs`, `check-no-audio.mjs`, `check-scenes.mjs`
- Araç ayarları: `.gitignore`, `.prettierrc.json`, `.prettierignore`, `eslint.config.js`, `tsconfig.json`,
  `vite.config.ts`, `index.html`
- Kod (IPC yüzeyi, günlük, sürüm, platform kodu): `src-tauri/src/lib.rs`, `commands.rs`, `diagnostics.rs`,
  `examples/ses_denemesi.rs`, `src/lib/backend.ts`, `src/lib/errorReporting.ts`
- Belgeler: `README.md`, `CLAUDE.md`, `docs/MIMARI.md`, `docs/ROADMAP.md` (yalnızca ilgili bölümler)

**Standartlar ve ölçütler:** GitHub Actions güvenlik iyi uygulamaları (en az ayrıcalık, eylemlerin SHA ile
sabitlenmesi, `GITHUB_TOKEN` kapsamı, cache ve yarış koşulları), SLSA/tedarik zinciri izlenebilirliği, SPDX lisans
tanımlayıcıları ve GPL-3.0 uyumu (MIT/BSD/Apache-2.0/MPL-2.0 bildirim yükümlülükleri), OFL-1.1, Tauri 2 güvenlik
modeli (CSP, capabilities), Microsoft WebView2 dağıtım seçenekleri (Tauri belgeleri ve `tauri-utils` kaynak kodu).

**Çalıştırılanlar:** `cargo metadata`/`cargo tree` (bağımlılık sayımı, lisans dağılımı, `build.rs` çalıştıran
crate'ler, ağ crate'lerinin kökeni), `grep` (unsafe, marka adları, ağ erişimi, XSS yuvaları), `git log`
(sürüm–commit ilişkisi) ve bölüm 7'deki 14 kalite kapısı komutu (bir kez, süreleriyle;
`CARGO_TARGET_DIR=(geçici derleme klasörü)`). Disk kısıtı (6,7 GB boş) nedeniyle Rust derlemesi
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` ile çalıştırıldı; bu ayarlar
yalnızca hata ayıklama bilgisini kapatır, lint ve test sonuçlarını değiştirmez.

**Okunmayanlar (bağımsızlık):** `docs/DENETIM.md`, `docs/devlog/`, GitHub issue/PR'lar, diğer çalışma kopyaları.

## 2. Genel değerlendirme

**Puan: 6/10.** Depo, bu büyüklükte bir açık kaynak projeden beklenenin üstünde bir kalite kapısı disiplinine sahip:
iki kilit dosyası da `--locked`/`npm ci` ile zorlanıyor, Rust'ta `unsafe` yasak ve kodda hiç yok, Tauri izinleri en
alt düzeyde (`core:default` + `dialog:allow-open`), Windows hedefinde ağa çıkan tek bir crate bile yok, lisans
denetimi iki katmanlı ve belgeli, Rust↔arayüz tip sözleşmesi üretilip CI'da karşılaştırılıyor, sahneler gerçek
Edge'de ölçülüyor. 14 kalite kapısının hepsi yerelde geçti. Zayıf yan **yayın hattı**: proje sahibinin kurup
denediği sabit linkli kurulum dosyası hiçbir testi beklemeden, SHA ile sabitlenmemiş üçüncü taraf bir eylemle
(`tauri-apps/tauri-action@v0`), `contents: write` yetkisiyle üretiliyor ve hangi commit'ten çıktığı dosyadan
anlaşılamıyor (aynı sürüm numarasıyla 5 farklı kurulum dosyası yayımlandı). Bir numaralı risk, tedarik zinciri:
mutable bir eylem etiketi ele geçirilirse sahibinin indirip çalıştırdığı exe sessizce değiştirilebilir.

En güçlü yan: kapılar gerçek ve otomatik (ölü kural yok, ts-rs/komut sözleşmesi dahil). En zayıf yan: yayın akışının
CI'dan bağımsız, atomik olmayan ve izlenemez olması. Bir numaralı risk: sabitlenmemiş eylemler + yazma yetkili
token → sabit linkteki kurulum dosyasının bütünlüğü.

## 3. Bulgular (önem sırasıyla)

### [YAY-01] Üçüncü taraf eylemler SHA ile sabitlenmemiş; yayın işinde yazma yetkili token mutable bir eyleme veriliyor

- Önem: Yüksek
- Güven: kesin
- Yer: `.github/workflows/test-surumu.yml:16-17, 29-43`; `.github/workflows/ci.yml:27, 54, 68, 71, 94, 137, 140, 157, 161`
- Kanıt: Bütün eylemler hareketli etiketle çağrılıyor: `actions/checkout@v5`, `actions/setup-node@v5`,
  `dtolnay/rust-toolchain@stable` (bir dal adı), `Swatinem/rust-cache@v2`, `EmbarkStudios/cargo-deny-action@v2`,
  `tauri-apps/tauri-action@v0` (0.x majör: kırıcı değişiklik de aynı etikete gelir), `actions/upload-artifact@v4`.
  Yayın iş akışında `permissions: contents: write` iş akışı düzeyinde ve
  `uses: tauri-apps/tauri-action@v0` adımına `GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}` veriliyor (satır 41-43).
  Üretilen exe doğrudan sabit linke (`releases/download/test-surumu/Lyraska-Kurulum.exe`) gidiyor ve proje
  sahibi bu dosyayı kurup çalıştırıyor (CLAUDE.md "Test için sahibine her zaman sabit indirme linki verilir").
- Etki: Mutable bir etiket, eylemin deposu ele geçirilirse (2025'teki `tj-actions/changed-files` olayı gibi)
  bir sonraki `main` push'unda derleme ortamında keyfi kod çalışır: kurulum dosyasına zararlı kod eklenebilir
  ve `contents: write` token'ıyla depo içeriği/Release değiştirilebilir. Hedef kitle (yazılımcı olmayan sahip,
  SmartScreen uyarısını "Yine de çalıştır" ile geçmeye alışmış) saldırının etkisini artırır.
- Yeniden üretme / önerilen test: `grep -n "uses:" .github/workflows/*.yml | grep -v "@[0-9a-f]\{40\}"` → 20/20 satır
  sabitlenmemiş. GitHub'ın "Require actions to be pinned to a full-length commit SHA" depo ayarı ile zorlanabilir.
- Önerilen düzeltme: Her `uses:` satırını tam SHA ile sabitleyin ve sürümü yorumda tutun
  (`uses: tauri-apps/tauri-action@<sha> # v0.5.x`); Dependabot `github-actions` ekosistemiyle güncelleyin.
  Yayın işinde `permissions` iş düzeyine indirilsin; `tauri-action`'a token verilmesin (yalnızca derliyor,
  Release'i `gh` adımı yapıyor) ya da `tauri-action` yerine doğrudan `npm run tauri build` çalıştırılsın (üçüncü
  taraf eylem sayısı bir azalır).

### [YAY-02] Test sürümü hiçbir testi beklemiyor; Windows testleri `main` üzerinde hiç çalışmıyor

- Önem: Yüksek
- Güven: kesin
- Yer: `.github/workflows/test-surumu.yml:6-9, 24-43`; `.github/workflows/ci.yml:126-129`
- Kanıt: `test-surumu.yml` `on: push: branches: [main]` ile tetikleniyor; `jobs.yayinla` içinde tek bir adım
  dizisi var: checkout → toolchain → `npm ci` → `tauri-action` (derleme) → Release. `needs:` yok, `workflow_run`
  yok, tek bir test ya da lint adımı yok. CI'daki Windows işi ise yalnızca PR ve elle tetiklemede çalışıyor:
  `if: github.event_name == 'pull_request' || github.event_name == 'workflow_dispatch'` (ci.yml:129). Yani
  `main`'e düşen commit için Windows'ta Clippy, `cargo test` ve `ses_denemesi` hiç koşmuyor; CI'nın Linux işleri
  yayınla **paralel** koşuyor, yayın onların sonucunu beklemiyor.
- Etki: (a) PR'daki CI, PR'ın geçici birleşme commit'ini (`refs/pull/N/merge`) test eder; squash/merge sonrası
  `main`'deki commit farklıdır ve yayın bunu test etmeden derler. (b) Dal koruması olmadan ya da yönetici
  atlatmasıyla `main`'e doğrudan push, test edilmemiş bir exe'yi sabit linke koyar. (c) CI kırmızıyken bile sabit
  link yenilenir; sahibi "doğrulama haftası"nda kırmızı bir derlemeyi test edebilir.
- Yeniden üretme / önerilen test: `main`'e kasıtlı derleme hatası içeren (yalnızca testte kırılan) bir commit
  push edin: CI kırmızı olur, Test sürümü yine de yeşil biter ve dosyayı yükler.
- Önerilen düzeltme: En küçük düzeltme: `test-surumu.yml`'yi `on: workflow_run: workflows: [CI], types:
[completed], branches: [main]` ile tetikleyip `if: github.event.workflow_run.conclusion == 'success'` koşulu
  ekleyin ve `ref: ${{ github.event.workflow_run.head_sha }}` ile aynı commit'i derleyin. Ayrıca ci.yml'deki
  Windows işinin `if` koşuluna `github.ref == 'refs/heads/main'` push'unu ekleyin (WASAPI kodu `main`'de de
  denetlensin). Dal koruması: "Require status checks" ile dört CI işini zorunlu kılın.

### [YAY-03] Kurulum dosyası hangi commit'ten çıktığı anlaşılamıyor: aynı sürüm numarasıyla 5 farklı exe yayımlandı

- Önem: Yüksek
- Güven: kesin
- Yer: `.github/workflows/test-surumu.yml:21-22, 45-51, 79-81`; `package.json:4`; `src-tauri/src/commands.rs:55-59`;
  `.github/ISSUE_TEMPLATE/hata-kaydi.yml:66-71`
- Kanıt: `git log` ile her commit'teki `package.json` sürümü:
  `1263b8c 0.0.44`, `5905aa9 0.0.44`, `294bd80 0.0.44`, `d2d5b4d 0.0.44`, `39a16ba 0.0.44` → beş ayrı `main`
  güncellemesi, beş ayrı `Lyraska-Kurulum.exe`, hepsi "0.0.44". Toplam 75 commit'e karşılık `package.json`
  46 kez değişmiş. Exe'ye gömülen tek kimlik sürümdür (`tauri-build` `FileVersion`/`ProductVersion`,
  `commands.rs:58` `env!("CARGO_PKG_VERSION")`); commit kimliği yalnızca Release notlarında var
  (`short_sha`, satır 57-64) ve notlar her yayında üzerine yazılıyor; `test-surumu` etiketi `git tag -f` ile
  taşınıyor (satır 80-81), geçmişi yok. SHA-256 toplamı yayımlanmıyor. Hata kaydı formu "Program sürümü"
  soruyor (satır 66-71): kullanıcı "0.0.44" yazar, hangi derleme olduğu bilinemez.
- Etki: Proje sahibinin bildirdiği bir hata hangi commit'teki koda ait, "bu düzeltme eline ulaşmış mıydı?"
  sorusu yanıtsız kalır; doğrulama haftasının bütün geri bildirimi bu belirsizlikle çalışıyor. Ayrıca indirilen
  dosyanın bütünlüğünü doğrulamanın yolu yok (imza da yok).
- Yeniden üretme / önerilen test: Yukarıdaki `git log` döngüsü; `AppInfo` testi (`commands.rs:687`)
  `assert_eq!(AppInfo::current().version, env!("CARGO_PKG_VERSION"))` biçiminde kendini kendisiyle
  karşılaştırıyor, derlemeyi ayırt eden bir alan yok.
- Önerilen düzeltme: `build.rs`'te `git rev-parse --short HEAD` (ya da CI'da `GITHUB_SHA`) ile
  `cargo:rustc-env=LYRASKA_COMMIT=…` üretip `AppInfo`'ya `commit` alanı ekleyin; karşılama ekranı ve günlük
  açılış satırı "0.0.46 (25f83f8)" yazsın; Release notlarına `sha256sum` ekleyin; her `main` yayınında
  `v0.0.46+25f83f8` gibi kalıcı bir etiket de atın (sabit `test-surumu` etiketi kalabilir).

### [YAY-04] Yayın adımları atomik değil; iptal ve eşzamanlı indirme yarışları sabit linki kırabilir

- Önem: Orta
- Güven: kesin
- Yer: `.github/workflows/test-surumu.yml:11-14, 74-96`
- Kanıt: `concurrency: cancel-in-progress: true` (satır 13-14). Son adımın sırası: `git push -f` etiket (80-81)
  → `gh release edit … --notes-file` (84) → `gh release upload … --clobber` (85) → eski adlı dosyaları silme döngüsü
  (87-92). `--clobber`, aynı adlı mevcut dosyayı **önce siler, sonra yükler**; yükleme süresince (~10 MB,
  saniyeler–dakikalar) sabit link 404 döner. Yeni bir `main` push'u bu pencerede gelirse iş iptal edilir: etiket
  ve notlar yeni commit'i gösterir, dosya eski ya da hiç yoktur; bir sonraki başarılı yayına kadar (Windows
  derlemesi, onlarca dakika) link kırık kalır.
- Etki: Proje sahibi "linki açtım, dosya yok" ya da "notlar X commit'i diyor ama program eski" durumuyla
  karşılaşır; hata raporları yanlış derlemeye bağlanır.
- Yeniden üretme / önerilen test: İki commit'i ardışık push edin; ilki yükleme adımındayken ikinci iş
  başladığında ilk iptal edilir. Yükleme sırasında linki `curl -I` ile yoklayın (404).
- Önerilen düzeltme: `cancel-in-progress: false` (yayınlar sırayla bitsin) ya da en azından son adımı tek
  parça yapın: dosyayı önce geçici adla yükleyin (`Lyraska-Kurulum.exe.yeni`), ardından GitHub API ile tek bir
  `PATCH /releases/assets/{id}` çağrısıyla adını değiştirin (eski dosyayı sonra silin); notları dosyadan
  **sonra** güncelleyin.

### [YAY-05] Üçüncü taraf lisans bildirimleri (MIT/Apache/MPL/BSD) kurulum paketine girmiyor

- Önem: Orta
- Güven: kesin
- Yer: `src-tauri/tauri.conf.json:33-35`; `public/lisanslar/` (tek dosya); `dist/assets/index-*.js`
- Kanıt: Windows hedefinde çözümlenen 301 crate'in lisans dağılımı (`cargo metadata`): 143 "MIT OR Apache-2.0",
  52 MIT, 27 "Apache-2.0 OR MIT", 21 MPL-2.0 (başta `symphonia-*`: çözücünün tamamı), 18 Unicode-3.0, 2
  BSD-3-Clause, … Arayüz paketinde React/ReactDOM (MIT) ve `@tauri-apps/api` (MIT/Apache-2.0) var. Pakete giren
  tek üçüncü taraf lisans metni `public/lisanslar/Inter-OFL-1.1.txt`; `dist/assets/*.js` içinde `@license` ya da
  "Copyright (c)" satırı **0** (derleyici yasal yorumları da düşürmüş). NSIS kurulumunda yalnızca
  `licenseFile: ../LICENSE` (GPL metni) gösteriliyor. MIT: "The above copyright notice and this permission
  notice shall be included in all copies or substantial portions"; Apache-2.0 §4; MPL-2.0 §3.3 (Larger Work'te
  MPL kaynağına erişim ve bildirim) — hiçbiri karşılanmıyor. `docs/LISANSLAR.md` kod bağımlılıklarını kapsam
  dışı bırakıp "CI denetler" diyor; CI yalnızca _yasak_ lisans arıyor, bildirim üretmiyor.
- Etki: GPL-3.0 projesinin kendi lisansı sağlam, ama ikili dağıtım yüzlerce izin lisansının bildirim koşulunu
  ihlal ediyor; v1.0 ve olası mağaza dağıtımı öncesi düzeltilmesi gereken uyum borcu.
- Yeniden üretme / önerilen test: `grep -c "Copyright" dist/assets/*.js` → 0; kurulum sonrası program klasöründe
  üçüncü taraf bildirim dosyası yok.
- Önerilen düzeltme: `cargo about generate` (EmbarkStudios, cargo-deny ile aynı aile) ile Rust bildirimlerini,
  npm için küçük bir betikle (6 üretim paketi) arayüz bildirimlerini `public/lisanslar/UCUNCU-TARAF.html`
  olarak üretin; CI'da üretilenin depodakiyle aynı olduğunu denetleyin (ts-rs adımı gibi); Ayarlar panelinde
  "Lisanslar" bağlantısı verin.

### [YAY-06] Güvenlik uyarıları (advisories) hiçbir yerde denetlenmiyor; Dependabot yok

- Önem: Orta
- Güven: kesin
- Yer: `src-tauri/deny.toml:110-112`; `.github/workflows/ci.yml:93-97`; `.github/` (dependabot.yml yok)
- Kanıt: `deny.toml` `[advisories]` bölümünde "Güvenlik uyarıları ayrı bir iş olarak izlenir" yazıyor, ama CI'daki
  tek cargo-deny adımı `command: check licenses bans sources` (ci.yml:97) — `advisories` yok; başka iş akışı,
  zamanlanmış tarama (`schedule:`), `.github/dependabot.yml` ya da `npm audit` adımı yok. Bu denetimde elle
  çalıştırıldı (bölüm 7): `cargo deny check advisories` → **KALDI** (RUSTSEC-2024-0370, `proc-macro-error`
  bakımsız, `gtk` üzerinden yalnızca Linux hedefi; güvenlik açığı sınıfında uyarı yok); `npm audit --omit=dev` → 0
  açık. Yani bugün gerçek bir açık yok, ama bunu söyleyebilen tek şey bu denetimin elle çalıştırmasıdır.
- Etki: Bilinen bir güvenlik açığı (ör. WebView/wry, sqlite, zip çözücüler) `Cargo.lock`'ta aylarca kalabilir;
  kimse haberdar olmaz. Program çevrimdışı olduğu için saldırı yüzeyi küçük (kötü niyetli ses dosyası: symphonia
  çözücüleri, kapak resmi) ama sıfır değil.
- Yeniden üretme / önerilen test: `grep -n advisories .github/workflows/*.yml` → sonuç yok.
- Önerilen düzeltme: ci.yml'deki cargo-deny adımına `advisories` ekleyin (kırmızı yapmak istenmiyorsa ayrı,
  `continue-on-error: true` bir adım + haftalık `schedule:` iş akışı); `.github/dependabot.yml` ile `cargo`,
  `npm`, `github-actions` ekosistemlerini haftalık izleyin; `npm audit --omit=dev --audit-level=high` adımı.

### [YAY-07] `embedBootstrapper` internet ister: "hiçbir zaman internete bağlanmak zorunda değildir" vaadiyle çelişki

- Önem: Orta
- Güven: kesin
- Yer: `src-tauri/tauri.conf.json:44`; `README.md:19`; `CLAUDE.md` "Ürün" bölümü
- Kanıt: `"webviewInstallMode": { "type": "embedBootstrapper" }`. `tauri-utils-2.10.1/src/config.rs:974-980`
  (kaynak kod belgesi): "`EmbedBootstrapper` — Embed the bootstrapper and run it. **Requires an internet
  connection.** Increases the installer size by around 1.8MB". Çevrimdışı seçenek aynı dosyada:
  "`OfflineInstaller` — Does not require an internet connection. Increases the installer size by around
  127MB". README: "Fully offline — no accounts, no telemetry, no network required."
- Etki: WebView2 Evergreen çalışma zamanı yüklü olmayan bir Windows 10'da (ya da runtime'ı kaldırılmış bir
  makinede) kurulum internetsiz tamamlanamaz; `silent: true` varsayılanıyla kullanıcı neden kurulamadığını da
  görmez. Windows 11 ve güncel Windows 10'da runtime genellikle hazır olduğu için çoğu kullanıcı etkilenmez;
  ama vaat "zorunda değildir" biçiminde mutlak.
- Yeniden üretme / önerilen test: Ağ bağdaştırıcısı kapalı, WebView2 kaldırılmış bir Windows 10 sanal makinede
  kurulum dosyasını çalıştırın.
- Önerilen düzeltme: Karar sahibine sunulsun: (a) `offlineInstaller` (+127 MB; vaat tam karşılanır) ya da
  (b) `embedBootstrapper` kalır, README ve Release notlarına "WebView2 yüklü değilse kurulum bir kez internete
  bağlanır; program kendisi hiç bağlanmaz" notu eklenir ve `silent: false` ile kullanıcı ne olduğunu görür.
  Öneri: (b) şimdi, (a) v1.0 için ayrıca "çevrimdışı kurulum" varyantı.

### [YAY-08] Yayımcı ve telif alanları yanlış: exe'de CompanyName "github", LegalCopyright "GPL-3.0"

- Önem: Orta
- Güven: kesin
- Yer: `src-tauri/tauri.conf.json:5, 33`; `LICENSE`; `src-tauri/Cargo.toml:5`
- Kanıt: `bundle.publisher` yok. `tauri-build-2.7.1/src/lib.rs:836-845`:
  `config.bundle.publisher.unwrap_or_else(|| config.identifier.split('.').nth(1) …)` → `"io.github.gozlemesevdalisi.lyraska"`
  için `"github"`; `res.set("CompanyName", &company_name)`. `tauri-utils` config.rs:1589: "publisher … Defaults
  to the second element in the identifier string … HKCU\Software\<publisher>\<product name>". Satır 848:
  `res.set("LegalCopyright", copyright)` → `"GPL-3.0"` (bir lisans adı, telif bildirimi değil). `LICENSE` salt GPL
  metni; depoda hiçbir yerde "Copyright (C) <yıl> <sahip>" satırı yok (`authors = ["Lyraska geliştiricileri"]`).
- Etki: Windows "Uygulamalar ve özellikler" ve dosya özelliklerinde yayımcı "github" görünür (yanıltıcı; GitHub
  markasını yayımcı gibi gösterir), kayıt defteri anahtarı `HKCU\Software\github\Lyraska` olur; telif alanı
  anlamsız. Telif sahibi adı olmayan GPL dağıtımı, lisansın uygulanabilirliğini zayıflatır (GPL "How to Apply"
  ekindeki bildirim yok).
- Yeniden üretme / önerilen test: Windows'ta `Lyraska.exe` → Özellikler → Ayrıntılar: Şirket "github", Telif
  hakkı "GPL-3.0". Ya da `tauri-build` koduyla hesap (yukarıda).
- Önerilen düzeltme: `tauri.conf.json`'a `"publisher": "Lyraska"` (ya da sahibin seçtiği ad) ve
  `"copyright": "© 2026 Lyraska geliştiricileri"`; `LICENSE` üstüne ya da `README`'ye telif satırı; `Cargo.toml`
  `authors` ile aynı ad.

### [YAY-09] CI'daki "Gerçek ses aygıtıyla deneme" adımı barındırılan runner'da aygıt olmadığı için her zaman "ATLANDI" ile yeşil çıkar

- Önem: Orta
- Güven: yüksek (kod yolu kesin; GitHub Windows runner'larında ses uç noktası olmadığı yaygın bilinen durum)
- Yer: `src-tauri/examples/ses_denemesi.rs:34-39, 61-67`; `.github/workflows/ci.yml:153-155`; `docs/MIMARI.md:187`
- Kanıt: `Err(Outcome::NoDevice) => { println!("ATLANDI: … ses çıkışı yok"); ExitCode::SUCCESS }`;
  `AudioError::NoOutputDevice | AudioError::OutputUnavailable` → `NoDevice`. GitHub'ın `windows-latest`
  görüntüsünde ses çıkış aygıtı bulunmaz (sahne denetimi yorumunda da "Ekran kartsız CI makinesi" kabulü var,
  `check-scenes.mjs:287-288`). Adım `if-no-device` gibi bir koruma ya da çıktı denetimi yapmıyor.
- Etki: CLAUDE.md ve MIMARI'nin "gerçek aygıt davranışı `ses_denemesi` ile denenir / Windows CI" güvencesi CI'da
  içi boş: WASAPI çıkışı aygıtla hiç çalıştırılmadan yeşil görünür; gerçek aygıt denemesi fiilen yalnızca proje
  sahibinin elindeki kurulumla yapılıyor.
- Yeniden üretme / önerilen test: CI günlüğünde adımın çıktısı "ATLANDI" olmalı. Öneri: adım çıktısını
  `Select-String BAŞARILI` ile denetleyin; aygıt yoksa adımı `continue-on-error` yerine **açıkça "atlandı"**
  olarak işaretleyin (`echo "::warning::ses aygıtı yok"`), belgelerde "CI'da aygıt yok" yazsın.
- Önerilen düzeltme: Runner'a sanal ses aygıtı kurmak kırılgan; dürüst düzeltme belgeleri ve adım adını
  düzeltmek ("WASAPI derleme ve aygıtsız yol denemesi") ve gerçek aygıt denemesini sahibinin elle çalıştırdığı
  bir komut olarak `DOGRULAMA.md`'ye eklemektir.

### [YAY-10] Sürüm tutarlılığı denetlenmiyor; CHANGELOG ve sürüm etiketi yok

- Önem: Orta
- Güven: kesin
- Yer: `package.json:4`, `src-tauri/Cargo.toml:3`, `src-tauri/tauri.conf.json:4`; `CLAUDE.md` "Sürüm numarası
  package.json içindedir; src-tauri/Cargo.toml ile aynı tutulur"
- Kanıt: Sürüm iki yerde elle yazılı (`0.0.46` × 2); `tauri.conf.json` `"version": "../package.json"` ile
  birini okuyor. Arayüz sürümü `vite.config.ts:163` `__APP_VERSION__` ile package.json'dan, çekirdek sürümü
  `commands.rs:58` ve `diagnostics.rs:41` `CARGO_PKG_VERSION` ile Cargo.toml'dan geliyor. Eşitliği denetleyen
  test/betik yok (`grep -rn "package.json" src-tauri/src scripts` → yalnızca lisans betiği). `tauri-build` de
  karşılaştırmıyor (`tauri-build-2.7.1/src/lib.rs` sürümü yalnızca kaynak resource'a yazar). Depoda
  `CHANGELOG*` yok; `git tag -l` boş (yalnızca CI'nın taşıdığı `test-surumu` etiketi uzakta).
- Etki: Biri unutulursa kurulum dosyası "0.0.47", karşılama ekranı ve günlük "0.0.46" der; hata kayıtları
  çelişir. Sürüm geçmişi yalnızca commit başlıklarında.
- Yeniden üretme / önerilen test: `Cargo.toml`'da sürümü değiştirip `npm run build && cargo test` → hiçbir kapı
  kırmaz.
- Önerilen düzeltme: `scripts/check-commands.mjs` yanına 10 satırlık `check-version.mjs`
  (package.json ↔ Cargo.toml ↔ Cargo.lock `lyraska` kaydı) ekleyip CI'ya koyun; ya da sürümü tek kaynaktan
  üretin (`build.rs` package.json'u okuyup `cargo:rustc-env` versin). Her sürüm bump'ında `vX.Y.Z` etiketi ve
  kısa `CHANGELOG.md`.

### [YAY-11] Sürüm derlemesinde `strip = true`: çökme günlüğündeki yığın izi çözümsüz adreslerden ibaret

- Önem: Orta
- Güven: yüksek
- Yer: `src-tauri/Cargo.toml:33-38`; `src-tauri/src/diagnostics.rs:118-141`
- Kanıt: `[profile.release] … panic = "abort"`, `strip = true` (hem semboller hem hata ayıklama bilgisi
  silinir; `debug` ayarlanmamış → zaten 0). Panik kancası `std::backtrace::Backtrace::force_capture()` çıktısını
  günlüğe yazıyor (satır 132-138) ve hata kaydı formu bu günlüğü istiyor. Sembolsüz ikilide `Backtrace` çerçeveleri
  `<unknown>` ya da salt adres olur; `panic = "abort"` kancanın çalışmasını engellemez ama Windows'ta PDB yoksa
  çözümleme yapılamaz.
- Etki: Çökme bildirimi geldiğinde "ÇÖKME … (dosya:satır)" satırı (panic `location`) işe yarar, yığın izi
  yaramaz; dolaylı panikler (kütüphane içinden) yerini gösteremez. Sürüm/commit de olmadığı için (YAY-03)
  adreslerle eşleştirecek PDB de yok.
- Yeniden üretme / önerilen test: `npm run tauri build` sonrası bilinçli bir panik tetikleyip `lyraska.log`'a
  bakın; çerçeveler sembolsüz.
- Önerilen düzeltme: `strip = "debuginfo"` yerine `split-debuginfo = "packed"` + `debug = "line-tables-only"`
  ile PDB üretip CI artifact'ı olarak (Release'e değil) saklayın; ya da en azından yığın izini yazmaktan vazgeçip
  günlüğü sadeleştirin. PDB'nin commit ile eşleşmesi için YAY-03 ön koşul.

### [YAY-12] Derleme araç zinciri sürümleri sabit değil (Rust `stable`, Node `22`); `rust-version` hiç denenmiyor

- Önem: Düşük
- Güven: kesin
- Yer: `.github/workflows/ci.yml:68, 137`; `test-surumu.yml:34`; `src-tauri/Cargo.toml:9`; `package.json` (engines yok)
- Kanıt: `dtolnay/rust-toolchain@stable` → her gün farklı bir derleyici olabilir; `rust-toolchain.toml` yok;
  `Cargo.toml` `rust-version = "1.85"` iddiası hiçbir işte `1.85` ile denenmiyor. `setup-node@v5` `node-version: 22`
  (yalnızca majör). `package.json`'da `engines` yok. Bu denetim 1.97.0 ile yapıldı.
- Etki: Yeni bir stable'daki yeni Clippy lint'i `-D warnings` ile CI'yı kod değişmeden kırar; aynı commit iki
  farklı günde farklı ikili üretir (yeniden üretilebilirlik yok); MSRV iddiası boş kalabilir.
- Önerilen düzeltme: `rust-toolchain.toml` (`channel = "1.9x.y"`) ekleyip eylemde `@stable` yerine onu
  kullanın (`dtolnay/rust-toolchain@master` + dosya, SHA sabitli); ayda bir elle güncelleyin. MSRV'yi ya ayrı bir
  `cargo +1.85 check` işiyle doğrulayın ya da alanı kaldırın. `engines.node: "22.x"` ve `node-version-file`.

### [YAY-13] CI hijyeni: zaman aşımı yok, gereksiz kimlik bilgisi/token, her dalda çift çalışma

- Önem: Düşük
- Güven: kesin
- Yer: `.github/workflows/ci.yml:6-10, 27, 54, 132, 156-159`; `test-surumu.yml`
- Kanıt: Hiçbir işte `timeout-minutes` yok (varsayılan 6 saat; Windows sahne denetimi adım başına 180 sn'lik
  beklemelerle uzun). `actions/checkout` `persist-credentials` varsayılanı (true) CI işlerinde kapatılmamış
  (yalnızca yayın işi `git push` için gerekiyor). PR'daki `tauri-action` adımına `GITHUB_TOKEN` veriliyor
  (ci.yml:158-159) ama `tagName` yok, token kullanılmıyor. `on.push.branches: ["**"]` + `pull_request` → PR
  açık bir dal her push'ta iki kez (push + PR) derleniyor; `concurrency` grubu `event_name` içerdiği için biri
  diğerini iptal etmiyor. Olumlu: `pull_request_target` yok, `permissions: contents: read`, cache yalnızca
  `Swatinem/rust-cache` ve `setup-node cache: npm` (dal kapsamlı, zehirlenme yolu görülmedi).
- Etki: Asılı kalan runner dakikaları, gereksiz token kapsamı, çift kaynak tüketimi.
- Önerilen düzeltme: Her işe `timeout-minutes` (Linux 30, Windows 90); CI'da `with: persist-credentials: false`;
  PR işinden `GITHUB_TOKEN` satırını silin; `push` tetikleyicisini `main`'e daraltıp PR'lara `pull_request` bırakın.

### [YAY-14] Ses dosyası denetimi: uzantı listesi eksik, izinli klasör sınırsız, `.gitignore` ile uyumsuz

- Önem: Düşük
- Güven: kesin
- Yer: `scripts/check-no-audio.mjs:5-12`; `.gitignore:21-44`
- Kanıt: Düzenli ifade 25 uzantı; eksikler: `mpc`, `tta`, `ac3`, `dts`, `amr`, `au`, `spx`, `mp2`, `mkv`, `mov`,
  `avi`, `3gp`, `tak`, `ofr`, `shn`, `wvc`, `sf2/sf3` (ses bankası). Betik `mid|midi` arıyor, `.gitignore` bunları
  içermiyor (iki liste elle senkron). İzinli klasör `src-tauri/tests/data/` için ek koşul yok: oraya konan
  herhangi bir MP3 (gerçek şarkı dahil) geçer; `README.md` "sentetik" diyor ama boyut/süre/sağlama denetimi yok.
  Yalnızca `git ls-files` (izlenen dosyalar) bakılıyor — doğru; ancak uzantısız/yeniden adlandırılmış ikili
  (ör. `.bin`) ya da arşiv içindeki ses yakalanmaz (içerik imzası denetimi yok).
- Etki: Telif riski küçük ama kural "asla" diyor; en olası kaçak yolu izinli klasöre büyük bir dosya koymak.
- Önerilen düzeltme: İzinli klasörde dosya başına üst sınır (ör. 200 KB) ve toplam sınır; `file`/sihirli bayt
  denetimi (ID3/`fLaC`/`OggS`/`ftyp`) uzantıdan bağımsız; uzantı listesini tek yerde tutup `.gitignore`'u
  betikten üretin.

### [YAY-15] npm lisans denetimi yasak listesi: GPL-3.0'la uyumsuz ama "yasak" olmayan lisanslar geçer

- Önem: Düşük
- Güven: kesin
- Yer: `scripts/check-licenses.mjs:11-23, 56-63`
- Kanıt: Yalnızca `FORBIDDEN` kalıpları (AGPL, SSPL, BUSL, Commons Clause, Elastic, CC-BY-NC, PolyForm-NC,
  Prosperity, UNLICENSED) reddediliyor; bilinmeyen/`SEE LICENSE IN` hata. Bunun dışında her şey geçer:
  `CC-BY-SA-4.0`, `EPL-1.0`, `CDDL-1.0`, `Artistic-1.0`, `JSON`, `LicenseRef-Custom`, `OSL-3.0` — GPL-3.0 ile
  uyumsuz ya da ticari kısıtlı olabilen lisanslar. Rust tarafı (`deny.toml`) tersine **izin listesi** kullanıyor;
  iki katman farklı felsefede. Olumlu: 274 paketin tamamı (dev dahil) taranıyor; "A OR B" doğru işleniyor;
  "AND" muhafazakâr davranıyor.
- Etki: Üretim paketi 6 taneyken risk düşük; dev bağımlılığı eklenirken sessiz uyumsuzluk mümkün.
- Önerilen düzeltme: `deny.toml`'daki izin listesini betiğe taşıyın (tek kaynak), listede olmayan lisans → hata;
  üretim (`--omit=dev`) ve geliştirme ayrımını çıktıda gösterin.

### [YAY-16] CSP `style-src 'unsafe-inline'` büyük olasılıkla gereksiz; `base-uri` yok

- Önem: Düşük
- Güven: olası
- Yer: `src-tauri/tauri.conf.json:25`
- Kanıt: `default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; connect-src ipc:
http://ipc.localhost`. Arayüzde 19 `style={{…}}` React özniteliği var; React bunları CSSOM (`element.style`)
  üzerinden uygular, CSP `style-src` bunu kısıtlamaz; `<style>` bloğu ya da `innerHTML`/`dangerouslySetInnerHTML`
  yok (grep: 0). Tauri kendi enjekte ettiği stiller için CSP'ye hash/nonce ekler. `base-uri` `default-src`'den
  devralınmaz. Olumlu: `script-src` default'tan `'self'`, `withGlobalTauri` kapalı (varsayılan), uzak içerik
  yok, capabilities yalnızca `core:default` + `dialog:allow-open`, `shell`/`opener`/`fs` izni yok.
- Etki: Etiketlerden gelen metin React tarafından kaçışlandığı için bugün sömürülebilir bir yol görülmedi;
  `'unsafe-inline'` ileride bir XSS'in stil tabanlı veri sızdırma yüzeyini açık tutar.
- Önerilen düzeltme: `'unsafe-inline'`ı kaldırıp gerçek Windows WebView2'de 5 sahne ve çekmeceyi deneyin
  (konsolda CSP ihlali yoksa kalıcı yapın); `base-uri 'self'; object-src 'none'` ekleyin; `app.security.freezePrototype: true`.

### [YAY-17] Betikler (`scripts/*.mjs`) lint ve tür denetimi dışında; ts-rs eski dosya denetimi yok

- Önem: Düşük
- Güven: kesin
- Yer: `eslint.config.js:9`; `tsconfig.json:19`; `.github/workflows/ci.yml:85-92`
- Kanıt: ESLint `files: ["**/*.{ts,tsx,js}"]` — `.mjs` yok; `tsconfig.include` `src`, `vite.config.ts`,
  `eslint.config.js` — `scripts/` yok. Kalite kapılarının kendisi (4 betik, ~600 satır) denetimsiz. CI'nın ts-rs
  adımı `git status --porcelain -- src/lib/bindings` ile yalnızca değişen/yeni dosyayı yakalar; ts-rs silmediği
  için Rust'tan kaldırılan bir tipin `.ts` dosyası sonsuza dek kalır (bugün 37 dosya = 37 export, 3'ü `rename`
  ile; tutarlı).
- Etki: Kapı betiğindeki bir yazım hatası CI'yı sessizce "her zaman geçer" yapabilir; eski binding'ler
  arayüzü yanıltır.
- Önerilen düzeltme: ESLint glob'una `mjs` ekleyin; tsconfig'e `scripts` için `checkJs` ya da ayrı
  `tsconfig.scripts.json`; ts-rs adımında dosyayı silip yeniden üretin (`rm -rf src/lib/bindings && cargo test`),
  sonra `git status` (silinenler de görünür).

### [YAY-18] Belge–kod uyumsuzlukları (README, CLAUDE.md, MIMARI.md)

- Önem: Düşük
- Güven: kesin
- Yer: `README.md:3-4, 65-70`; `CLAUDE.md` komut tablosu ve "Görsel güvenlik"; `docs/MIMARI.md:187`;
  `src-tauri/src/commands.rs:59`
- Kanıt (doğrulanan 12 iddia; ✔ doğru, ✘ yanlış/eskimiş):
  1. ✘ README: "Phase 1 is in progress … visuals and the library come next" — `docs/ROADMAP.md:23` "Faz 1 …
     tamamlandı"; `library/` modülü ve 5 sahne var. `commands.rs:59` `phase: "Faz 1"` sabit metin.
  2. ✘ README "Checks" bloğu lisans, ses, komut sözleşmesi, biçim ve `--locked` adımlarını içermiyor; CI içeriyor.
  3. ✘ CLAUDE.md komut tablosunda `cargo clippy`/`cargo test` `--locked` olmadan; CI `--locked` ile.
  4. ✘ CLAUDE.md "Faz 2'de bu kural bir 'epilepsi güvenli modu' ile denetlenebilir hale gelir" — `visual_safe_get/set`
     komutları ve `useVisualSafe` zaten var (lib.rs:132-133).
  5. ✘ MIMARI.md:187 "Windows'a özel kod … Windows CI" — Windows işi `main` push'unda çalışmıyor (YAY-02).
  6. ✘ MIMARI/CLAUDE "gerçek aygıt davranışı `ses_denemesi` ile denenir" — CI'da aygıt yok (YAY-09).
  7. ✔ CLAUDE.md "CI iki denetim yapar: `cargo deny check licenses bans sources` ve `npm run check:licenses`" — ci.yml:41-42, 93-97.
  8. ✔ CLAUDE.md "`unsafe` yasaktır (`unsafe_code = "deny"`)" — Cargo.toml:41; `grep -rn unsafe src-tauri/src` → 0.
  9. ✔ CLAUDE.md "sürüm derlemesi panikte kapanır (`panic = "abort"`)" — Cargo.toml:37.
  10. ✔ CLAUDE.md "CI üretilenin depodakiyle aynı olduğunu … `npm run check:commands` … denetler" — ci.yml:45-46, 85-92.
  11. ✔ MIMARI "Kurallar ve onları denetleyenler" tablosundaki 24 Rust test adı ve 5 TS test dosyası kaynakta var
      (hepsi grep ile bulundu: `ses_yolu_bellek_ayirmaz`, `ebu_3341_durum_*`, `her_eski_dosya_…`, `MIN_ALIAS_REJECTION_DB: 140.0` vb.).
  12. ✔ CLAUDE.md "Veri tipleri `src/lib/bindings/` altına üretilir" — `.cargo/config.toml` `TS_RS_EXPORT_DIR`.
- Etki: Yeni katkıcı ve proje sahibi için yanıltıcı; "Faz 1 sürüyor" ifadesi indirme sayfasındaki beklentiyi
  düşürüyor.
- Önerilen düzeltme: README'yi Faz 1 sonrasına göre güncelleyin (sahneler, kütüphane, kör test, ton); "Checks"
  bloğunu CI ile birebir yapın; CLAUDE.md tablosuna `--locked`; `AppInfo.phase`'i kaldırın ya da ROADMAP'ten türetin.

### [YAY-19] Güvenlik bildirimi politikası (SECURITY.md) ve CODEOWNERS yok; dal koruması doğrulanamadı

- Önem: Düşük
- Güven: kesin (dosya yokluğu), olası (dal koruması)
- Yer: depo kökü, `.github/`
- Kanıt: `SECURITY.md`, `CODEOWNERS`, `CONTRIBUTING.md` yok; hata kaydı formu güvenlik açığı için ayrı kanal
  sunmuyor (`config.yml` yalnızca `blank_issues_enabled: true`). Dal koruması GitHub ayarıdır; bu denetimde
  erişilmedi.
- Etki: Bir güvenlik açığı bulan kişi herkese açık issue açar; `main`'e doğrudan push olup olmadığı belirsiz
  (YAY-02'nin etkisini belirler).
- Önerilen düzeltme: Kısa `SECURITY.md` (özel bildirim: GitHub "Report a vulnerability"); dal korumasında
  "Require PR + status checks" (CI'nın 4 işi) ve "Require signed commits" isteğe bağlı.

### Şüpheler (kanıtlanamayan)

- **WebView2 Evergreen çalışma zamanı** Microsoft Edge Update servisiyle kendini günceller; bu, programın değil
  Windows bileşeninin ağ trafiğidir, ama "program çalışırken makineden hiç paket çıkmaz" beklentisi tam
  karşılanmaz. Güven: olası (ürün dışı; belgelerde not edilmesi önerilir).
- `tauri-apps/tauri-action` derlemeyi `--locked` olmadan çalıştırır; `Cargo.lock` manifestle uyumsuz kalsa
  yayın derlemesi kilidi sessizce günceller. CI `--locked` ile aynı commit'i denetlediği için pratikte yakalanır
  (YAY-02 düzeltilince kesinleşir). Güven: olası.

## 4. Güçlü yanlar

- **İzin yüzeyi gerçekten dar:** `capabilities/default.json` yalnızca `core:default` ve `dialog:allow-open`;
  `fs`, `shell`, `opener`, `http`, `updater` eklentisi yok (`Cargo.toml:20-31`, 13 doğrudan bağımlılık).
  `withGlobalTauri` ve devtools kapalı (varsayılan). CSP tanımlı; `script-src` yalnızca `'self'`.
- **Çevrimdışı iddiası kodla tutarlı:** Windows hedefinde çözümlenen 301 crate arasında ağ istemcisi yok;
  `reqwest`/`hyper`/`url` `Cargo.lock`'ta yalnızca Tauri'nin **Android/iOS** hedefleri için
  (`tauri-2.12.1/Cargo.toml:321`, `cargo tree -i reqwest` → Windows'ta "nothing to print"). Arayüzde `fetch`,
  `XMLHttpRequest`, `WebSocket`, `sendBeacon`, dış `<script>`/`url(http…)` yok (grep: 0). Günlük yalnızca yerel
  dosyaya yazar (`diagnostics.rs:1-12`) ve içeriği (yollar, kullanıcı adı) hata kaydı formunda dürüstçe
  açıklanıyor (`hata-kaydi.yml:86-91`); arayüz hataları tekrar süzgeciyle günlüğü şişirmiyor (`errorReporting.ts`).
- **`unsafe` yok ve denetleniyor:** `unsafe_code = "deny"` + kaynakta 0 eşleşme; `unwrap/expect/panic` lint'leri
  açık, testlerde `clippy.toml` ile serbest.
- **Kilit disiplini:** `Cargo.lock` v4 ve `package-lock.json` v3 depoda; CI `cargo … --locked` ve `npm ci`;
  `deny.toml` `wildcards = "deny"`, `unknown-registry/unknown-git = "deny"`; git bağımlılığı yok (`cargo
metadata`: kaynaklar yalnızca `registry` ve `path`).
- **Lisans kuralı iki katmanlı ve belgeli:** Rust'ta izin listesi (GPL-3.0 uyumlu set, `confidence-threshold 0.9`,
  3 hedef), npm'de yasak listesi + bilinmeyen = hata; OFL-1.1 kararı tarih ve onayla `docs/LISANSLAR.md`'de,
  lisans metni pakette (`dist/lisanslar/Inter-OFL-1.1.txt` derlemede doğrulandı). Araştırma varlıkları (LAME,
  FluidR3, PyTorch) "programa girmez" ayrımıyla kayıtlı. Marka yasağı: kaynak ve stillerde Pioneer vb. yok
  (grep yalnızca kural metninde ve `iTunSMPB` etiket adında — teknik zorunluluk).
- **Sözleşme kapıları özgün ve etkili:** `check-commands.mjs` 42 komut / 43 çağrıyı ad **ve argüman adı**
  düzeyinde eşliyor (Tauri'nin snake→camel kuralı dahil); ts-rs çıktısı CI'da diff'leniyor; `check-scenes.mjs`
  sahneleri gerçek Edge/D3D11'de ve WebGL kapalı yedekte sanal zamanla, WCAG parlama pencere boyutunda ölçüyor.
- **Yerelde 14/14 kapı yeşil** (bölüm 7), 272 TS testi ve Rust testlerinin tamamı geçti; tek `#[ignore]` testin
  gerekçesi yazılı (ayar taraması aracı).
- **Yayın notları dürüst:** imzasız olduğu, SmartScreen uyarısı ve yönetici izni istemediği açıkça yazılıyor;
  `installMode: currentUser` bunu doğruluyor.

## 5. Denetlenemeyenler ve sınırlar

- Windows'a özel kod (WASAPI, `explorer /select`) ve NSIS kurulum dosyası Linux'ta derlenip çalıştırılamadı;
  yayımcı/telif alanları `tauri-build` kaynak kodundan **hesapla** doğrulandı, gerçek exe incelenmedi.
- GitHub tarafı ayarlar (dal koruması, eylem izinleri, Dependabot uyarıları, mevcut Release/etiket durumu) ve
  gerçek CI günlükleri bağımsızlık kuralı gereği okunmadı; `git tag -l` yerel kopyada boş.
- `windows-latest` runner'ında ses aygıtı olmadığı kabulü genel bilgiye dayanır (YAY-09 güveni "yüksek").
- `npm run check:scenes` (Edge gerektirir) ve `npm run tauri build` çalıştırılmadı (görev kapsamı dışı; disk kısıtı).
- Rust derlemesi disk kısıtı nedeniyle hata ayıklama bilgisi kapalı yapıldı; test ve lint sonuçları bundan
  etkilenmez, derleme süreleri CI ile birebir karşılaştırılamaz.
- Cargo.lock'taki 479 paketin tek tek bakım durumu (son yayın tarihi, bakımcı sayısı) çevrimdışı ortamda
  değerlendirilemedi; `cargo deny check advisories` çıktısı (bölüm 7) bu boşluğun bir kısmını kapatır.

## 6. Önerilen öncelik sırası

1. **YAY-01** eylemleri SHA ile sabitle, yayın işinden gereksiz token'ı kaldır (bir saatlik iş, en büyük risk).
2. **YAY-02** yayını CI'nın başarısına bağla (`workflow_run`), Windows işini `main`'de de çalıştır.
3. **YAY-03** commit kimliğini exe/karşılama ekranı/günlüğe göm, SHA-256 yayımla — doğrulama haftası geri
   bildirimi için hemen gerekli.
4. **YAY-04** yayın adımını atomik yap (`cancel-in-progress: false` en küçük adım).
5. **YAY-06** `advisories` + Dependabot.
6. **YAY-10** sürüm tutarlılık betiği (10 satır) ve etiketleme.
7. **YAY-08** publisher/copyright alanları (iki satır).
8. **YAY-07** WebView2 kurulum modu kararı ve belgeleme.
9. **YAY-05** üçüncü taraf bildirimleri (`cargo about`) — v1.0 öncesi zorunlu.
10. **YAY-09, YAY-18** belgeleri gerçekle hizala; **YAY-11** PDB saklama; **YAY-12, 13, 14, 15, 16, 17, 19** bakım.

## 7. Kalite kapıları (bir kez çalıştırıldı)

Ortam: Linux x86_64, 4 çekirdek, rustc/cargo 1.97.0, cargo-deny 0.20.2, Node 22.22.0, npm paketleri mevcut
`node_modules` ile (CI'daki `npm ci` adımı burada tekrarlanmadı). Çıktılar
`docs/denetim-2/kapilar-ozet.tsv` (özet; tam günlükler depoya alınmadı).

| #   | Komut                                                | Sonuç     | Süre                               | Not                                                                                                                                                                                                                                                                                                                                                       |
| --- | ---------------------------------------------------- | --------- | ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `npm run format:check`                               | GEÇTİ     | 11,3 s                             | Prettier, uyarı yok                                                                                                                                                                                                                                                                                                                                       |
| 2   | `npm run lint`                                       | GEÇTİ     | 17,1 s                             | ESLint, 0 uyarı                                                                                                                                                                                                                                                                                                                                           |
| 3   | `npm run typecheck`                                  | GEÇTİ     | 11,6 s                             | `tsc --noEmit`, strict                                                                                                                                                                                                                                                                                                                                    |
| 4   | `npx vitest run`                                     | GEÇTİ     | 54,2 s                             | 36 dosya, **272 test geçti**, 0 atlanan/todo (vitest süresi 52,0 s; %48 jsdom ortamı)                                                                                                                                                                                                                                                                     |
| 5   | `npm run build`                                      | GEÇTİ     | 11,1 s                             | `dist/` 804 KB; JS 371 KB (gzip 118 KB), CSS 45 KB, 7 Inter woff2, `lisanslar/Inter-OFL-1.1.txt` kopyalandı                                                                                                                                                                                                                                               |
| 6   | `npm run check:commands`                             | GEÇTİ     | 0,3 s                              | 42 komut, arayüzde 43 çağrı; adlar ve argümanlar eşleşiyor                                                                                                                                                                                                                                                                                                |
| 7   | `npm run check:licenses`                             | GEÇTİ     | 0,3 s                              | 274 paket incelendi (dev dahil), yasak lisans yok                                                                                                                                                                                                                                                                                                         |
| 8   | `npm run check:audio`                                | GEÇTİ     | 0,3 s                              | İzlenen dosyalar tarandı; yalnızca `tests/data/` sentetik 4 dosya                                                                                                                                                                                                                                                                                         |
| 9   | `cargo fmt --check`                                  | GEÇTİ     | 0,5 s                              |                                                                                                                                                                                                                                                                                                                                                           |
| 10  | `cargo clippy --all-targets --locked -- -D warnings` | GEÇTİ     | 238,4 s (3 dk 58 s derleme)        | 0 uyarı                                                                                                                                                                                                                                                                                                                                                   |
| 11  | `cargo test --locked`                                | GEÇTİ     | 303,6 s (testlerin kendisi 14,4 s) | **328 geçti** (324 birim + 4 `tests/formats.rs`), 0 başarısız, **1 ignored**: `audio::eq::tests::ayar_taramasi` — gerekçe kaynakta: "ayar keşfi için: `cargo test --release -- --ignored --nocapture ayar_taramasi`" (parametre tarama aracı, denetim değil). Doc-test 0.                                                                                 |
| 12  | `cargo deny check licenses bans sources`             | GEÇTİ     | 1,5 s                              | "bans ok, licenses ok, sources ok" (3 hedef)                                                                                                                                                                                                                                                                                                              |
| 13  | `cargo deny check advisories`                        | **KALDI** | 2,1 s                              | `error[unmaintained]` RUSTSEC-2024-0370: `proc-macro-error` bakımsız; yol `gtk 0.18.2` → yalnızca **Linux** hedefi, Windows ikilisinde yok (`cargo tree --target x86_64-pc-windows-msvc -i proc-macro-error` → boş). **Güvenlik açığı (vulnerability) sınıfında uyarı yok.** CI bu komutu hiç çalıştırmadığı için (YAY-06) sonuç bugüne dek görünmüyordu. |
| 14  | `npm audit --omit=dev`                               | GEÇTİ     | 1,0 s                              | 0 açık (6 üretim paketi)                                                                                                                                                                                                                                                                                                                                  |

**Özet: 13/14 geçti, 1 kaldı** (advisories; Windows'u etkilemeyen bakımsızlık uyarısı, yine de CI'ya eklenirse
`deny.toml [advisories] ignore` kaydı gerekçesiyle yazılmalı ya da gtk'yi getiren Linux hedefi grafikten çıkarılmalı).

**Depo kirlenmesi:** `git status --porcelain` testlerden sonra yalnızca `?? node_modules` (önceden var olan
sembolik bağ) ve `?? target-denetim/` (görevde istenen derleme klasörü, 1,7 GB, `.gitignore`'da değil) gösterdi.
`src/lib/bindings/` **değişmedi**: ts-rs üretimi depodakiyle birebir aynı (CI'daki "Arayüz tipleri güncel mi"
adımı yerelde de yeşil). `dist/` `.gitignore` kapsamında. `git checkout -- .` gerekmedi.
