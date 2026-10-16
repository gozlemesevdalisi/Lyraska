# Arayüz (React + TypeScript + WebGL2) — Bağımsız denetim raporu

Denetçi: bağımsız denetçi arayüz. Kaynak: main @ 25f83f8. Tarih: 16 Ekim 2026.

## 1. Kapsam ve yöntem

**İncelenen dosyalar** (`(depo kökü)/`): `src/` altındaki her şey — 20 bileşen (`components/`),
20 hook (`hooks/`), 34 saf yardımcı/çizici (`lib/`), üretilmiş 30 tip dosyası (`lib/bindings/`, yalnızca
tutarlılık için), `styles/global.css` (3 180 satır), `main.tsx`, `App.tsx`, 36 test dosyası; ayrıca
`index.html`, `vite.config.ts`, `tsconfig.json`, `eslint.config.js`, `package.json`,
`src-tauri/tauri.conf.json` (CSP), `scripts/check-scenes.mjs`, `public/lisanslar/`. Toplam ≈ 19 100 satır
TS/TSX/CSS; `check-scenes.mjs` 508 satır. Okunabilir belgeler: `CLAUDE.md`, `docs/MIMARI.md`,
`docs/ROADMAP.md`, `README.md`.

**Standartlar:** WCAG 2.2 (1.4.3 kontrast, 2.1.1 klavye, 2.4.3 odak sırası, 2.4.7/2.4.11 odak görünürlüğü,
2.5.8 hedef boyutu, 2.3.1/2.3.3 ışık hassasiyeti, 4.1.2 ad/rol/değer), WAI-ARIA 1.2 (grid, tablist, slider
örüntüleri), ISO 9241-391 (parlama ve düzenli desen), React 19 kuralları (çizim sırasında durum
güncelleme, effect bağımlılıkları), WebGL2 kaynak yönetimi, Tauri 2 IPC maliyeti.

**Çalıştırılanlar** (`CARGO_TARGET_DIR` gerekmedi, yalnızca Node araçları):

- `npx tsc --noEmit` → hatasız (çıkış 0).
- `npx eslint .` → hatasız (çıkış 0).
- `npx vitest run` → 36 dosya, **272 test geçti**, 83,5 sn.
- `python3` ile WCAG kontrast oranları ve kuramsal parlama hızları hesaplandı (bulgularda verildi).
- `npm run build` ve `npm run check:scenes` çalıştırılmadı (izin yok; tarayıcı ölçümleri Bölüm 5'te).

**Yöntem:** her dosya satır satır okundu; hook'lar arası sözleşmeler (kim kime hangi işlevi veriyor, hangi
`useEffect` hangi bağımlılıkla yeniden kuruluyor) çıkarıldı; her bulgu dosya:satır ile kanıtlandı.
Kanıtlanamayan şüpheler Bölüm 3'ün sonunda ayrı başlıkta, güven "olası" ile verildi.

## 2. Genel değerlendirme

**Puan: 7/10.** Arayüz, bu boyuttaki bir hobi/açık kaynak projesi için belirgin biçimde olgun: katı
TypeScript (`strict`, `noUncheckedIndexedAccess`), tek bir `!` dışında zorlama yok, Rust'tan üretilen tipler
elle düzenlenmiyor, lint/tür/test üçlüsü temiz, her panel bir hata sınırında, IPC istekleri "yanıt gelmeden
yenisi gönderilmez" kuralıyla korunuyor, büyük liste sanallaştırılmış, ve ışık hassasiyeti için iki katmanlı
bir güvence var (modelde hız sınırı + gerçek tarayıcıda ölçüm). En güçlü yan budur: epilepsi güvenliği
sloganda kalmamış, matematiği ve CI ölçümü var. En zayıf yan, görsel katmanın **kaynak disiplini**: sahne,
şarkı çalmıyorken ve pencere boştayken bile saniyede 60 kez tam ekran gölgelendirici çizer, aynı görsel veri
her karede 2–3 ayrı IPC akışıyla istenir, tuval boyutu her karede yerleşim okunarak ölçülür; zayıf ya da
yazılımsal ekran kartı için hiçbir koruma yok. Bir numaralı risk: **dahili ya da yazılımsal GPU'lu bilgisayarda
programın ağır ve pil tüketen bir uygulamaya dönüşmesi** — bu, "1080p dahili GPU'da 60 fps" hedefinin hiçbir
yerde ölçülmemesiyle birleşiyor. Erişilebilirlikte iyi niyet görünür (ARIA adları, odak halkaları,
`prefers-reduced-motion`), ancak kütüphane tablosunun ARIA yapısı ve odak yönetimi (çekmece kapanınca odağın
kaybolması, görünmez odak göstergesi) klavye kullanıcısı için eksik.

## 3. Bulgular (önem sırasıyla)

### [ARA-01] Sahneler boşta ve duraklatılmışken de 60 kare/sn tam ekran çizer; zayıf/yazılımsal GPU'ya karşı koruma yok

- Önem: Yüksek
- Güven: kesin (boşta çizim kodla kanıtlı); yazılımsal GPU'daki etkisi yüksek (CI betiğinin kendi notuyla)
- Yer: `src/hooks/useVisualFeed.ts:34-61`, `src/components/SkyScene.tsx:68-82`,
  `src/components/HighwayScene.tsx:41-54`, `src/lib/gl.ts:13-19`, `src/lib/gl.ts:132-137`,
  `src/lib/skyFallback.ts:106-195`, `scripts/check-scenes.mjs:158-163`
- Kanıt:
  - `useVisualFeed` döngüsü `playing`'den bağımsız her karede döner ve sahneyi çağırır:
    `raf = window.requestAnimationFrame(loop)` (satır 36) … `tick.current(playing && latest ? … : null, dt)`
    (satır 60). `SkyScene.onTick` her çağrıda `stepSky` + `renderer.current?.draw(sky.current)` (satır 78)
    yapar; durgunluk denetimi yok (VU ve spektrum sahnelerinde `still/idle` denetimi var, gök ve otoyolda yok).
  - Gök gölgelendiricisi piksel başına 4 perde × çok sayıda gürültü örneği, 6 yıldız, göl manzarasında
    ekranın %24'ü için `sky()`nin ikinci kez çağrılması (`skyRenderer.ts:323`) içerir; 1080p sınırında bu her
    karede ~2 M piksel × ağır iş.
  - `getWebGl2` yalnızca `{ antialias: false, alpha: false, depth: false }` ister (satır 15);
    `failIfMajorPerformanceCaveat` yok → yazılımsal (SwiftShader/WARP) WebGL2 de "başarılı" sayılır ve 2D
    yedek hiç devreye girmez. `gl.ts:135`: "dahili ekran kartında da her kare zamanında çizilir" — ölçülmemiş
    bir varsayım.
  - CI betiğinin kendi notu (`check-scenes.mjs:158-163`): ekran kartsız Windows makinede "tek bir ekran
    görüntüsü 10–60 sn sürebiliyor" — yani yazılımsal çizimde bu gölgelendirici kare başına saniyeler alıyor.
  - 2D yedek çizim de aynı döngüde her karede çalışır: 3 perde × (genişlik / 3 px) şerit için `setTransform` +
    `fillRect` (1920 px'te ~1 900 çağrı) + 140 yıldız + 6 radyal gradyan, kare başına (satır 126-183).
- Etki: Pencere açık ama hiçbir şey çalmıyorken bile GPU/CPU sürekli meşgul: dizüstünde pil ve fan; ekran
  kartı zayıf ya da sanal makine/uzak masaüstü gibi yazılımsal çizim ortamlarında arayüzün tamamı
  (düğmeler dahil) saniyede birkaç kareye düşer. "1 saat çalmada sıfır takılma" ve "1080p dahili GPU'da
  60 fps" hedefleri hiçbir testle ölçülmüyor.
- Yeniden üretme / önerilen test: (1) Programı aç, şarkı açma, Görev Yöneticisi'nde GPU %'sini izle
  (gece göğü/otoyol seçili). (2) Edge'i `--disable-gpu` ile `check:scenes` çalıştır ve kare süresini ölç.
  Önerilen test: `useVisualFeed` için "çalmıyorken ve durum değişmemişken `onTick` çizim yapmaz" birim testi;
  `check:scenes`'e kare süresi ölçümü (ör. `advance` sırasında 16,7 ms üstü kare oranı) eklenmesi.
- Önerilen düzeltme: (a) Gök ve otoyolda çalmıyorken kare hızını düşür (ör. 15–20 fps) ya da durum değişmeyince
  çizimi atla; (b) `getContext`'e `failIfMajorPerformanceCaveat: true` ver, başarısızsa 2D yedek/düşük
  çözünürlük; (c) kare süresi bekçisi: ardışık karelerde 25 ms üstü sürede `canvasScale`'i 0,5'e indir;
  (d) 2D yedekte şerit genişliğini `pixel`'den bağımsız 4–6 px'e çıkar ve 30 fps'e sınırla.

### [ARA-02] Aynı görsel veri her karede 2–3 ayrı IPC akışıyla istenir (gizli Senkron paneli ve Gece sürüşü)

- Önem: Orta
- Güven: kesin
- Yer: `src/components/SyncPanel.tsx:19-37`, `src/components/PlayerScreen.tsx:381-390`,
  `src/components/DriveScene.tsx:143,149`, `src/components/HighwayScene.tsx:54`, `src/hooks/useVisualFeed.ts:39-57`
- Kanıt: Çekmecedeki bütün paneller sürekli bağlıdır (`hidden` ile gizlenir, PlayerScreen 335. satır yorumu).
  `SyncPanel` içindeki `BeatIndicator` kendi `useVisualFeed(playing, onTick)` döngüsünü kurar (SyncPanel:29);
  panel görünmese de `playing` iken her karede `getVisualFrame()` çağırır. `DriveScene` hem kendi
  `useVisualFeed` (143) hem içine koyduğu `<HighwayScene>`'in `useVisualFeed`'i (HighwayScene:54) ile iki akış
  daha açar. Her akışın kendi "tek istek uçuşta" kilidi var ama akışlar birbirinden habersiz: Gece sürüşünde
  saniyede 3 × ~60 = ~180 `visual_frame` isteği, diğer sahnelerde ~120.
- Etki: Tauri IPC'de her istek JSON serileştirme + ana iş parçacığında ayrıştırma; `VisualFrame` 32 bant +
  yönetmen notu ≈ 1 KB. Gereksiz 2–3 kat IPC yükü; çekirdeğin `visual_bridge` kilidini 2–3 kat sık tutar;
  dizüstünde CPU/pil. Görsel bir hata üretmez ama "60 fps" bütçesinden pay yer.
- Yeniden üretme / önerilen test: `components.test.tsx` içinde `getVisualFrame` çağrı sayısını 0,5 sn
  boyunca say: gök sahnesinde ~kare sayısı kadar olmalı, şu an ~2×; gece sürüşünde ~3×.
- Önerilen düzeltme: Görsel veriyi tek bir sağlayıcıda (PlayerScreen düzeyinde bir hook/ref ya da küçük bir
  abone deseni) iste, sahneler ve `BeatIndicator` ona abone olsun; `DriveScene` yolu kendi karesini
  `HighwayScene`'e prop olarak geçsin. Gizli paneldeki `BeatIndicator`, yalnızca panel görünürken dinlesin.

### [ARA-03] Kütüphane: arama sonucu küçülünce eski seçim indeksi kalır; Enter `undefined` şarkıyı çalmaya çalışır (TypeError)

- Önem: Orta
- Güven: kesin
- Yer: `src/components/LibraryPanel.tsx:49`, `:90-93`, `:295`; `src/components/PlayerScreen.tsx:126-131`
- Kanıt: `const [selected, setSelected] = useState(-1)` (49) yalnızca `select()` (sınırlar) ve satıra
  tıklamayla (`onClick={() => setSelected(index)}`, 295) değişir; `tracks` değişince sıfırlanmaz. Enter dalı:
  `else if (event.key === "Enter" && selected >= 0) { … onPlay(tracks[selected]!, tracks); }` (90-93).
  `tracks.length` seçimden küçükse `tracks[selected]` `undefined`'dır; `playFromLibrary(track, list)`
  `track.path`'e erişir (PlayerScreen 130) → `TypeError: Cannot read properties of undefined (reading 'path')`.
- Etki: Kullanıcı 50. şarkıya tıklar, arama kutusuna yazar (3 sonuç kalır), listeye odaklanıp Enter'a basar:
  hiçbir şey çalmaz, hata günlüğüne bir TypeError düşer. Olay işleyicisindeki hata olduğu için ErrorBoundary
  yakalamaz; sessiz başarısızlık. `aria-selected` da hiçbir satırda doğru olmaz.
- Yeniden üretme / önerilen test: `components.test.tsx`'e: 5 şarkı → 5. satıra tıkla → aramayla 1 sonuca
  düşür → listeye `Enter` → `backend.open` çağrılmamalı ve hata fırlamamalı (ya da ilk satır çalmalı).
- Önerilen düzeltme: `tracks` değişince `selected`'ı sınırla ya da sıfırla (ör. çizim sırasında
  `const current = Math.min(selected, tracks.length - 1)` kullan); Enter dalında `tracks[selected]` yoksa çık.

### [ARA-04] Oynatma denetimleri, başlık ve üst çubuk hiçbir hata sınırında değil; belge "her panel ve sahne" diyor

- Önem: Orta
- Güven: kesin
- Yer: `src/components/PlayerScreen.tsx:242-266` (topbar), `:269-279` (uyarı), `:281-295` (`LiveChrome`),
  `:216-240` (yalnızca sahne sınırlı), `:342-419` (yalnızca paneller sınırlı); `CLAUDE.md` ("her panel ve
  sahne bir ErrorBoundary içinde: hata yalnızca o bölümü kapatır")
- Kanıt: `LiveChrome` (NowPlaying, InfoStack, Dock, SeekBar + SongMapTrack) ve `header.topbar` doğrudan
  `<main>` içinde çizilir; ErrorBoundary yalnızca `SceneStage` ve altı çekmece panelini sarar. `LiveChrome`
  her karede yeniden çizilir ve `keyAt`, `dropCountdown`, `timelineLayout`, `formatLabel`, `coverGradient`
  gibi veri bağımlı hesaplar yapar (InfoStack 44-53, SeekBar 118-146).
- Etki: Bu bölümlerden birinde çizim hatası (ör. çekirdekten beklenmedik bir alan) olursa tüm PlayerScreen
  düşer; kök sınır "Lyraska bir hatayla karşılaştı… Yeniden başlat" gösterir, sahne ve bütün paneller
  kaybolur. En sık kullanılan bölüm (çal/duraklat, sarma) en az korunan bölüm.
- Yeniden üretme / önerilen test: `ErrorBoundary.test.tsx` benzeri: `InfoStack` içine hata fırlatan sahte
  bir alt bileşen enjekte edilince sahne ve çekmece ayakta kalmalı.
- Önerilen düzeltme: `LiveChrome`'u (ve üst çubuğu) kendi `ErrorBoundary`'sine al; yedek görünüm olarak en
  azından çal/duraklat/durdur düğmelerini basit biçimde göster. `CLAUDE.md`'deki cümleyi gerçek kapsama uydur.

### [ARA-05] Kütüphane tablosu: `role="grid"` içinde hücre rolü yok, seçili satır ekran okuyucuya bildirilmez, odak göstergesi görünmez

- Önem: Orta
- Güven: kesin (ARIA yapısı ve CSS kodla; kullanıcı etkisi standarttan)
- Yer: `src/components/LibraryPanel.tsx:244-277`, `:288-331`; `src/styles/global.css:2015-2024`, `:42`
- Kanıt: Kapsayıcı `role="grid" aria-rowcount` (244); başlık satırı `role="row"` + `role="columnheader"`
  (245-270) doğru. Veri satırları `role="row" aria-rowindex aria-selected` (290-293) ama hücreler yalnızca
  `<span className="library__no|library__cell|library__time">` — `role="gridcell"` yok (WAI-ARIA: `row`'un
  çocukları `gridcell/rowheader/columnheader` olmalı). Klavye odağı kapsayıcı `div.library__list`
  (`tabIndex={0}`, 273) üzerinde; satırlar odaklanamaz ve `aria-activedescendant` verilmez → ekran okuyucu
  ok tuşuyla gezinirken hangi satırın seçildiğini duymaz. Odak göstergesi:
  `.library__list { outline: none }` ve `.library__list:focus-visible { box-shadow: inset 0 0 0 1px var(--vfd-ghost) }`
  (2019-2024); `--vfd-ghost: rgba(108, 246, 223, 0.075)` (42) → %7,5 opak 1 px çizgi, pratikte görünmez
  (WCAG 2.4.7 / 2.4.11). Satır seçilmeden önce (ilk Tab'da) klavye kullanıcısı nerede olduğunu göremez.
- Etki: Ekran okuyucu kullanıcısı için kütüphane "boş bir grid"; klavye kullanıcısı için odak kaybolur.
  WCAG 4.1.2 ve 2.4.7 ihlali.
- Yeniden üretme / önerilen test: NVDA ile listeye Tab → ok tuşları: hiçbir satır okunmaz. Testing Library
  ile `getAllByRole("gridcell")` boş döner (şu an); `aria-activedescendant` seçili satırın id'sine eşit olmalı.
- Önerilen düzeltme: Hücrelere `role="gridcell"`; her satıra `id`, kapsayıcıya
  `aria-activedescendant={selected>=0 ? rowId : undefined}`; odak halkasını en az `2px solid var(--ink)`
  (diğer düğmelerdeki gibi) yap. Alternatif: `role="listbox"/"option"` daha basit ve bu etkileşime daha uygun.

### [ARA-06] Odak yönetimi: çekmece kapanınca odak kaybolur; sekmelerde ok tuşu gezinmesi yok; odak panele taşınmaz

- Önem: Orta
- Güven: kesin (kod); etkisi yüksek
- Yer: `src/components/PlayerScreen.tsx:297-333`, `src/hooks/useDrawer.ts:38-64`
- Kanıt: Çekmece `<aside … aria-hidden={!drawer.open} inert={!drawer.open}>` (301-303). Esc ya da "Paneli
  kapat" ile `close()` çağrılınca odak çekmecenin içindeki bir düğmede kalır; öğe `inert` olunca tarayıcı odağı
  `body`'ye düşürür. Odağı açan düğmeye (`chromeButton`, 194-206) geri taşıyan bir kod yok. Açılışta da odak
  panele taşınmaz (`toggle()` yalnızca `setOpen(true)`). Sekmeler `role="tablist"` + `role="tab"` (306-323)
  ama `onKeyDown` yok: ARIA sekme örüntüsünün ←/→ ile gezinmesi ve `tabIndex=-1` dolaşımı eksik; sekmede
  ←/→ basılınca hiçbir şey olmaz (oynatıcı kısayolu `button` için susturulur, usePlayer 267-268).
- Etki: Klavye kullanıcısı her Esc'ten sonra belgenin başından Tab'lamaya başlar (WCAG 2.4.3). Ekran
  okuyucu "sekme listesi" duyurur ama alışılmış ok tuşları çalışmaz.
- Yeniden üretme / önerilen test: Kütüphane düğmesine Tab + Enter → panel içinde bir düğmeye Tab → Esc →
  `document.activeElement` `body` olur (olmalı: Kütüphane düğmesi).
- Önerilen düzeltme: `close()`'da odağı tetikleyen düğmeye (ref) döndür; `toggle()` açılışta ilk sekmeye
  odakla; sekmelerde ←/→/Home/End ile `select()` ve roving `tabIndex`.

### [ARA-07] Parlama güvencesinin ölçmediği yollar: proxy parlaklık, yalnızca durağan uçlar, güvenli mod/azaltılmış hareket/yedek çizim/fps dışarıda; bir yorum kodla çelişiyor

- Önem: Orta
- Güven: kesin (ne ölçüldüğü kodla); kalan riskin büyüklüğü olası
- Yer: `src/lib/sky.test.ts:72-92,191-209`, `src/lib/highway.test.ts:177-203`, `src/lib/stageLight.test.ts:111-131`,
  `scripts/check-scenes.mjs:318-347`, `:64-78`, `src/lib/stageLight.ts:108-111`, `src/components/SettingsPanel.tsx:74-77`
- Kanıt:
  - Birim testleri `countFlashes`'ı **model** değerlere uygular: `auroraLuminance(s.energies, s.bloom)`,
    `highwayLuminance(glow, bloom)`, `stageLuminance(light)`; bunlar `AURORA_MAX_LUMINANCE × …` gibi üst sınır
    formülleridir, ekrandaki gerçek parlaklık değil. Kuramsal sınır (hesap): gök enerjisinin 0→1 geçişi
    parlaklığı 0,175 değiştirir, 0,10 için 0,571 birim gerekir, hız 1,2/sn → geçiş 0,476 sn → **en çok
    1,05 parlama/sn** (güvenli modda 0,52); sahne ışığı 1,2/sn; otoyol 1,47/sn. Yani modelde sınır sağlam.
  - `check:scenes` gerçek pikselleri ölçer ama yalnızca iki durağan uç arasında: `level` 0 ve 1 ile iki ayrı
    sayfa, 2 500 ms sanal süre sonra tek ekran görüntüsü (satır 321-338). Sahte yönetmende `anticipation: 0,
release: 0.4` sabit (74-75): gerilim (perdeler ufka çekilir), açılım 0→1, ölçü başı kabarması
    (`--stage-spread`), tema geçişi (2 sn) ve `pulse` ile akış hızlanması ölçüme girmez. Güvenli mod,
    `prefers-reduced-motion`, 2D yedek çizimin parlama sınırı (yalnızca "çiziliyor mu" denetlenir, 446-450),
    sahne geçiş animasyonu ve kare hızı ölçülmez.
  - `countFlashes` WCAG'nin alan (%25 / 10°) ve "koyu taraf < 0,80" koşullarını uygulamaz (muhafazakâr, iyi) ama
    kırmızı parlama ölçütünü de bilmez; VU'nun doygun kırmızı lambası (`--vu-lamp-on: #ff5a36`) alanı küçük
    (`r=4` / 240×140) olduğu için ölçüt dışı — bu gerekçe hiçbir yerde yazılı değil.
  - `stageLight.ts:108` yorumu "güvenli modda vurgu gelmez" der; kod `fresh = !reducedMotion && accent > …`
    (110) — `safe` parametresi kabarmayı etkilemez. Ayarlar metni (SettingsPanel 75-76) "saniyede en fazla
    bir vuruş" vaat eder; arayüz tarafında bu yalnızca otoyol lambalarında (`SAFE_LAMP_RATE`) uygulanır; göğün
    `pulse` akışı ve halenin kabarması her vurguyu izler. Vaat, çekirdekteki yönetmenin güvenli modu bilmesine
    bağlı (arayüzden doğrulanamadı, bkz. Bölüm 5).
- Etki: Güvenlik iddiası büyük olasılıkla doğru (üst sınır matematiği ve durağan uç ölçümü güçlü), ancak
  iddianın bir kısmı ölçülmemiş; yorum/kod uyumsuzluğu gelecekte yanlış güven verir.
- Yeniden üretme / önerilen test: `check:scenes`'e üçüncü bir ölçüm ekle: sanal zamanda 10 sn boyunca her
  250 ms bir görüntü, ardışık kareler arası en kötü pencere farkı ve `countFlashes` ile parlama/sn; sahte
  yönetmende `anticipation` 0→1→0 ve `release` 1→0 gerçek bir drop gibi değişsin; aynı ölçüm `visual_safe_get:
true` ve `--force-prefers-reduced-motion` ile. Yedek çizim için de parlama sınırı.
- Önerilen düzeltme: Yorumu koda uydur (ya kabarmayı `safe` ile de kapat ya yorumu sil); kırmızı lamba için
  alan gerekçesini `vu.ts`'ye yaz; yukarıdaki dinamik ölçümü CI'a ekle.

### [ARA-08] Kısayol listesi "1 – 4 Sahne seç" diyor; beş sahne var

- Önem: Düşük
- Güven: kesin
- Yer: `src/components/SettingsPanel.tsx:21`, `src/lib/scene.ts:5-11,18`, `src/components/PlayerScreen.tsx:223`
- Kanıt: `["1 – 4", "Sahne seç"]`; `SCENES` beş öğe (sky, highway, vu, spectrum, drive); `sceneForKey`
  `/^[1-9]$/` ile 5'i de kabul eder; sahne hata metni doğru "(1–5)" yazar.
- Etki: Kullanıcı 5. sahneye (Gece sürüşü) klavye kısayolu olduğunu öğrenemez; belge/kod uyumsuzluğu.
- Yeniden üretme / önerilen test: `SHORTCUTS`'taki sahne satırının `SCENES.length`'ten türetilmesi ve bir
  testle doğrulanması.
- Önerilen düzeltme: `["1 – " + SCENES.length, "Sahne seç"]`.

### [ARA-09] Soluk metinler ışıklı sahne üstündeki cam katmanlarda 4,5:1'in altına düşer

- Önem: Düşük
- Güven: yüksek (hesap; gerçek arka plan sahneye göre değişir)
- Yer: `src/styles/global.css:22,27,30` (`--text-muted`, `--ink-3`, `--glass-fill`), `:1497` (`PEAK`), `:49`
- Kanıt (WCAG 1.4.3 hesabı, python3): `--ink-3 rgba(244,247,251,.48)` saf `--bg` üstünde 4,68:1, ufuk rengi
  üstünde 4,61:1 (sınırda geçer); cam (`--glass-fill` %46) perdelerle aydınlanmış bir gök (≈ rgb 40,90,110)
  üstündeyken **3,92:1**; `--text-muted` aynı koşulda **4,12:1**. Bu renkler 11–14 px yardımcı metinlerde
  (ör. `.timeline__legend` 11 px, `.now__hint`, kütüphane soluk sütunları). `PEAK` etiketi `#6b6f75` /
  `#111214` 3,71:1 (7 px, süs niteliğinde).
- Etki: Sahne parlakken ve çekmece kapalıyken küçük yardımcı yazılar düşük görüşlü kullanıcı için zor okunur.
- Yeniden üretme / önerilen test: Kulüp temasında (tema 2, en parlak perde) gök sahnesinde `.now__hint`'in
  kontrastını Edge DevTools ile ölç.
- Önerilen düzeltme: `--ink-3`'ü 0,58'e, `--text-muted`'ı 0,6'ya çıkar ya da cam katman zeminini
  `--glass-strong`'a yaklaştır; 11 px yardımcı metinleri 12 px'e çıkar.

### [ARA-10] Klasör çipindeki "çıkar" düğmesi 18×18 px (WCAG 2.5.8 en az 24×24)

- Önem: Düşük
- Güven: kesin
- Yer: `src/styles/global.css:1929-1952`, `src/components/LibraryPanel.tsx:163-171`
- Kanıt: `.chip { height: 24px; … }`, `.chip button { width: 18px; height: 18px; }` — çipin içinde ve
  komşusuyla aralığı 2 px. Dokunmatik ekranlı Windows dizüstünde yanlışlıkla klasör çıkarma riski.
- Etki: Hedef boyutu ölçütü ihlali; yanlış dokunuşla klasör kütüphaneden çıkar (dosyalar silinmez, ama liste
  gider, yeniden tarama gerekir).
- Önerilen düzeltme: Düğmeyi 24×24 yap (çip yüksekliği 28), ya da tıklanabilir alanı `::before` ile genişlet.

### [ARA-11] Tuval boyutu her karede yerleşim okunarak ölçülür (zorunlu reflow)

- Önem: Düşük
- Güven: yüksek
- Yer: `src/lib/gl.ts:150-163`, `src/lib/skyRenderer.ts:365`, `src/lib/highwayRenderer.ts:228`,
  `src/lib/skyFallback.ts:108-110`
- Kanıt: `fitCanvas` her `draw`'da `canvas.clientWidth/clientHeight` ve `window.devicePixelRatio` okur. Aynı
  karede `LiveChrome` React ile yeniden çizildiği için (konum metni, `clipPath`, `scaleX`) stil kirli olur;
  `clientWidth` okuması tarayıcıyı eşzamanlı yerleşime zorlar — kare başına 1 (Gece sürüşünde 2) zorunlu reflow.
- Etki: Ölçülebilir ama küçük kare süresi kaybı; 60 fps bütçesinde gereksiz pay.
- Önerilen düzeltme: `ResizeObserver` (`devicePixelContentBoxSize`) ile boyutu olayla izle; `draw` yalnızca
  saklanan değeri kullansın.

### [ARA-12] WebGL bağlamı `dispose`'da bırakılmaz; sahne/manzara değişiminde bağlam birikir

- Önem: Düşük
- Güven: yüksek
- Yer: `src/lib/skyRenderer.ts:390-398`, `src/lib/highwayRenderer.ts:248-256`, `src/components/PlayerScreen.tsx:218`
- Kanıt: `dispose()` program ve VAO'yu siler, bağlamı değil (`WEBGL_lose_context.loseContext()` yok). Sahne
  ya da gök manzarası değişince `ErrorBoundary key={`${scene}-${skyLook.look}`}` tüm sahneyi yeniden
  bağlar: yeni `<canvas>` + yeni bağlam. Chromium en çok ~16 canlı WebGL bağlamına izin verir; çöp toplayıcı
  yetişmezse "Too many active WebGL contexts, oldest context lost" ile en eski bağlam (hâlâ görünen olabilir)
  düşürülür. 1↔2 tuşlarına hızlı basmak bunu üretebilir.
- Etki: Nadir ama gerçek: hızlı sahne gezintisinde sahnenin kararması ya da konsol uyarıları; bellek
  (tam ekran arka tampon) gereksiz tutulur.
- Önerilen düzeltme: `dispose()` sonunda `ctx.getExtension("WEBGL_lose_context")?.loseContext()`.

### [ARA-13] Sürükle-bırak dinleyicisi her panel değişiminde sökülüp yeniden kurulur (kısa kör pencere)

- Önem: Düşük
- Güven: kesin
- Yer: `src/hooks/useDropToLibrary.ts:25-38,47-62`, `src/components/PlayerScreen.tsx:119-134`,
  `src/hooks/useDrawer.ts:38-41`
- Kanıt: Dinleyici etkisi `[handleDropped]`'a bağlı; `handleDropped` → `onTracks` → `playAndReveal` →
  `closeDrawer` → `useDrawer.close` `[panel, onLeave]`'e bağlı. Sekme her değişince zincir yeni kimlik alır,
  etkinin temizleyicisi `unlisten()` çağırır, yeni `onDragDrop` (dinamik `import` + `onDragDropEvent`) kurulur.
  Bu asenkron aralıkta bırakılan dosya kaybolur; `cancelled` bayrağı yalnızca sıralamayı korur.
- Etki: Çok seyrek: kullanıcı sekme değiştirdiği anda dosya bırakırsa bırakma sessizce yitirilir.
- Önerilen düzeltme: `handleDropped`'ı `useRef`'te tut, etki `[]` ile tek kez kurulsun.

### [ARA-14] Kütüphane listesi: her tazelemede tüm sonuç kümesi IPC'den geçer (10 000 şarkıda tarama/analiz boyunca onlarca kez)

- Önem: Düşük
- Güven: kesin (arayüz tarafı); çekirdeğin sayfalama yapmadığı varsayımı
- Yer: `src/hooks/useLibrary.ts:83-94,96-110`, `src/lib/analysis.ts:28-32`, `src/components/LibraryPanel.tsx:96`
- Kanıt: Arama etkisi `[…, status.trackCount, analysisKey]` değişince `searchLibrary(query)` ile **tam listeyi**
  alır; çizim sanal ama veri değil. Tarama sırasında durum 600 ms'de bir sorulur ve `trackCount` her artışta
  tetikler; analizde her %2'de (`total/50`) bir. 10 000 şarkı × ~150 B ≈ 1,5 MB JSON, tarama boyunca onlarca,
  analiz boyunca 50 kez.
- Etki: Büyük kütüphanede ilk tarama sırasında arayüz takılmaları (JSON ayrıştırma ana iş parçacığında);
  bellekte 10 000 nesnenin her seferinde yeniden oluşması.
- Önerilen düzeltme: Tarama sürerken tazelemeyi en fazla 2 sn'de bire indir; uzun vadede çekirdekte
  sayfalı/aralıklı arama (`offset/limit`) ve yalnızca görünen aralığın istenmesi.

### [ARA-15] Küçük erişilebilirlik tutarsızlıkları: canlı bölge her vuruşu okur; görsel olarak gizlenen bilgi kartları erişilebilirlik ağacında kalır; iki durumlu düğmeler çift duyuru

- Önem: Düşük
- Güven: kesin
- Yer: `src/components/MarkerPanel.tsx:142`, `src/styles/global.css` (`.app.has-drawer .info { opacity: 0;
pointer-events: none }`), `src/components/SettingsPanel.tsx:65-73,83-91,98-106`
- Kanıt: `.marker__counts` `aria-live="polite"` — her Boşluk basışında "N beat · ~X BPM" yeniden okunur (sn'de
  2 kez). Çekmece açıkken `.info` yalnızca CSS ile gizlenir, `aria-hidden` yok. Ayar düğmeleri `aria-pressed`
  - metinde "Açık/Kapalı": ekran okuyucu "basılı, Açık" der.
- Önerilen düzeltme: `aria-live`'ı yalnızca özet satırına (sayı) uygula ya da kaldır; `.info`'ya
  `aria-hidden={drawer.open}`; ayarlarda `role="switch" aria-checked` kullan (ekolayzerde zaten öyle).

### [ARA-16] Test ve tip hijyeni: sahte kare üretilen tiple uyumsuz; jsx-a11y lint yok; 2D yedek, bağlam kaybı ve güvenli modun sahneye ulaşması birim testlerde yok

- Önem: Düşük
- Güven: kesin
- Yer: `src/hooks/useVisualFeed.test.ts:369-380`, `eslint.config.js:7-19`, `src/components/components.test.tsx:1190-1205`,
  `src/lib/skyRenderer.ts:350-360`
- Kanıt: Test sabiti `barBeat: null` alanı taşır (`VisualFrame`'de yok; `beat.barBeat` içinde) ve
  `as unknown as VisualFrame` ile tipten kaçar — sahte veri gerçek sözleşmeden sapmış. ESLint yalnızca
  `react-hooks`; `eslint-plugin-jsx-a11y` yok (ARA-05/15 türü hatalar derlemede yakalanmıyor). "Güvenli mod"
  testi düğmeyi ve çekirdek çağrısını doğrular, `safe`'in `stepSky/stepStageLight`'a ulaştığını değil. WebGL
  `webglcontextlost/restored` yolu (skyRenderer 350-360) hiçbir testte tetiklenmiyor. 2D yedek çizim jsdom'da
  çalışmaz (`getContext("2d")` yok); yalnızca CI'daki Edge işi (`SCENE_EXPECT=fallback`) görür — iyi, ama
  parlama sınırı orada ölçülmez (ARA-07).
- Önerilen düzeltme: Sahte kareyi `satisfies VisualFrame` ile yaz; `jsx-a11y` ekle; `SceneStage`'e `safe`
  prop'unun sahneye geçtiğini gösteren bir test; `canvas.dispatchEvent(new Event("webglcontextlost"))` ile
  kayıp/geri yükleme testi.

### [ARA-17] Kapak önbelleği `data:` adreslerini sınırsız boyutla 24 adet tutar

- Önem: Düşük
- Güven: kesin
- Yer: `src/hooks/useCover.ts:5-15`
- Kanıt: `CACHE_LIMIT = 24`, değerler base64 `data:` dizgileri; gömülü kapaklar 1–3 MB olabilir → 24 × 3 MB
  ≈ 70 MB dizgi bellek. Ayrıca her `<img src=data:>` tarayıcıda ayrıca kodlanır.
- Önerilen düzeltme: Sınırı toplam bayta göre koy (ör. 20 MB) ya da çekirdekten küçültülmüş kapak iste;
  `blob:` URL + `revokeObjectURL`.

### Şüpheler (kanıtlanamadı; güven: olası)

- **[ARA-Ş1] `song_map` / `song_key` yola bağlı değil.** `useSongMap(trackPath)` çekirdeğe yol göndermez
  (`getSongMap()`, useSongMap:21) ve ilk dolu yanıtı alınca sorgulamayı durdurur (24). Boşluksuz geçişte
  çekirdek bir an önceki şarkının haritasını döndürürse harita yeni şarkıya kilitlenir ve şarkı boyunca
  yanlış kalır. Çekirdek tarafı okunmadı; test: geçiş anında `song_map`'in hangi şarkıya ait olduğu.
- **[ARA-Ş2] Komut yanıtından sonra gelen eski durum sorgusu.** `usePlayer` durum sorgusunu yalnızca sökülürken
  iptal eder (`cancelled`, 233-248); `toggle` yanıtından hemen sonra, komuttan önce gönderilmiş bir sorgu
  "paused" ile gelirse `applyStatus(next,false)` düğme simgesini bir sorgu süresi (≤ 250 ms) geri çevirir.
  Gözle görülür titreme olasılığı düşük; sıra numarası (ABX'teki gibi `issued/applied`) ile kesin önlenir.
- **[ARA-Ş3] Düzenli desen (ISO 9241-391).** Spektrum sahnesi tam ekranda 47 × 10 düzenli nokta ızgarasıdır;
  standart yüksek karşıtlıklı, 5'ten çok çizgi çifti içeren düzenli desenleri de risk sayar. Sönük noktalar
  %7,5 opak ve yananlar %62 opak VFD renginde olduğundan karşıtlık düşüktür; yine de yazılı bir
  değerlendirme yok.
- **[ARA-Ş4] `prefers-reduced-motion` altında yol akışı sürer.** `stepHighway` azaltılmış harekette hızı
  %25'e indirir ama durdurmaz (highway.ts 151-158); WCAG 2.3.3 "hareket animasyonu kapatılabilir" için
  "yavaş" yeterli sayılmayabilir. Tasarım kararı olarak belgelenmiş, kullanıcı araştırması yok.

## 4. Güçlü yanlar

- **Parlama güvenliği iki katmanlı:** modelde hız sınırı (`ENERGY_SLEW_PER_SECOND = 1.2`, `slew()`), hesapla
  en çok ~1,05 parlama/sn (gök) — eşik 3'ün üçte biri; gerçek tarayıcıda WCAG pencere boyutunda piksel ölçümü
  (`check-scenes.mjs:261-308`, integral görüntüyle en kötü 341×256 pencere). Vuruşlar parlaklığa değil
  harekete bağlanmış (`sky.ts:13-16`), CSS'te parlaklığı yalnızca `--stage-glow` değiştirir (1357-1359).
- **IPC disiplini:** `usePlayer` (233-245), `useVisualFeed` (39-57), `useAbx` (121-136), `useEqualizer` (88-109)
  hepsinde "yanıt gelmeden yenisi gönderilmez"; ekolayzerde araya gelen değişikliklerden yalnızca sonuncusu
  gönderilir; ABX'te sıra numarasıyla eski yanıt atılır (78-83). Testler bunu ölçer (`usePlayer.test.ts:150-165`).
- **Çalma sırası yarışları düşünülmüş ve test edilmiş:** #51/#52/#74 senaryoları `usePlayback.test.ts`'de
  (açılış sürerken ikinci seçim, bozuk dosya, boşluksuz geçişte sıranın ilerlemesi).
- **Tip güvenliği:** `strict` + `noUncheckedIndexedAccess`; depoda tek `!` (`marks.ts:65`, güvenli); `as`
  yalnızca CSS değişkeni ve `Object.fromEntries` gibi zorunlu yerlerde; Rust → TS tipleri üretilir ve CI ile
  denetlenir (`CLAUDE.md`), `sceneCheck.test.ts` sahte çekirdeğin alan eksikliğini derlemede yakalar.
- **Hata işleme:** altı panel ve sahne ayrı `ErrorBoundary`'de, hata bileşen yoluyla günlüğe gider
  (`ErrorBoundary.tsx:32-38`); yakalanmamış istisna ve `unhandledrejection` tekrar süzgeciyle günlüğe yazılır
  (`errorReporting.ts:29-41`, saniyede 60 satır taşmasına karşı). WebGL derleme/bağlam hataları sürücü
  mesajıyla tuvale ve günlüğe işlenir (`gl.ts:81-127`), bağlam kaybı/geri yüklemesi ele alınır.
- **Performans bilinci:** `useLivePosition` ile her karede yalnızca konum gösteren bileşenler çizilir ve bu
  testle ölçülür (`components.test.tsx:315-341`); liste sanallaştırılmış (`virtual.ts`); tuval 1080p piksel
  sınırında (`canvasScale`); `DotMatrix` binlerce `<circle>` yerine tek `<path>`.
- **Güvenlik/gizlilik:** `dangerouslySetInnerHTML`, `innerHTML`, dış bağlantı, `window.open` yok (grep ile);
  CSP `default-src 'self'`, görseller yalnızca `data:/blob:`, bağlantı yalnızca `ipc:`; telemetri yok.
- **Yerelleştirme:** sayılar `Intl.NumberFormat("tr-TR")`, gerçek eksi işareti (`−`), `toLocaleUpperCase("tr")`;
  kısayol metinleri (Boşluk, ←→ 5 sn, Ctrl+O, Esc) kodla uyumlu (ARA-08 dışında).
- **Azaltılmış hareket:** JS (`usePrefersReducedMotion` 7 bileşende) ve CSS (`@media` 5 blok) birlikte;
  yıldız kırpışması, kaset, kayan yazı, sahne geçişi durur.

## 5. Denetlenemeyenler ve sınırlar

- `npm run build` ve `npm run check:scenes` çalıştırılmadı (izin yok): WebGL gölgelendiricilerinin Edge/ANGLE
  üstünde derlenmesi, gerçek parlaklık ölçümleri, 2D yedek görünümü ve paket boyutu doğrulanmadı. Bulgular
  CI betiğinin okunmasına dayanır.
- Gerçek Windows/WebView2 yok: kare hızı, GPU kullanımı, dokunmatik davranış, ekran okuyucu (NVDA) çıktısı,
  `inert` ve `color-mix()`/`@property` desteği gözlenmedi (WebView2 Chromium ≥ 120 varsayıldı).
- Rust çekirdeği kapsam dışı: Görsel Yönetmen'in güvenli modda nabzı 1/sn'ye indirdiği iddiası (ARA-07),
  `song_map`'in geçiş anındaki davranışı (ARA-Ş1), `library_search`'ün sayfalama yapıp yapmadığı (ARA-14)
  arayüzden doğrulanamadı.
- Kontrast hesapları tipik arka planlar için yapıldı; sahne içeriği sürekli değiştiği için gerçek değer ana
  göre farklı olabilir.
- Testler bir kez, paylaşımlı CPU'da çalıştırıldı; kararlılık (flaky) ölçülmedi.

## 6. Önerilen öncelik sırası

1. **ARA-01** — boşta/duraklatmada çizimi düşür, `failIfMajorPerformanceCaveat` + kare süresi bekçisi.
2. **ARA-03** — kütüphane Enter hatası (küçük, kesin, kullanıcı görür).
3. **ARA-02** — tek görsel veri sağlayıcısı (IPC yükünü 2–3 kat azaltır; ARA-01 ile birlikte).
4. **ARA-04** — `LiveChrome`/üst çubuk için hata sınırı ve belge düzeltmesi.
5. **ARA-05, ARA-06** — grid semantiği, odak göstergesi, odak dönüşü, sekme ok tuşları.
6. **ARA-07** — dinamik parlama ölçümü (CI), yorum/kod uyumu, güvenli mod ölçümü.
7. **ARA-08, ARA-16** — kısayol metni; `jsx-a11y`, `satisfies` ile sahte veri, bağlam kaybı testi.
8. **ARA-11, ARA-12, ARA-13** — `ResizeObserver`, `loseContext`, sürükle-bırak dinleyicisi `useRef`.
9. **ARA-09, ARA-10, ARA-14, ARA-15, ARA-17** — kontrast, hedef boyutu, liste tazeleme, canlı bölge, kapak önbelleği.
