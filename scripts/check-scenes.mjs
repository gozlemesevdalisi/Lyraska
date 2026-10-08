// Sahne denetimi: derlenmiş arayüzü gerçek bir tarayıcıda açar, "Gece göğü" ve
// "Gece otoyolu" sahnelerini sahte müzik verisiyle çalıştırır ve ölçer:
//
// - Ekran kartı çizimi (WebGL2) açıldı mı, gölgelendiriciler derlendi mi?
// - Konsolda hata var mı?
// - Görüntü gerçekten çiziliyor mu (düz/karanlık değil) ve zamanla değişiyor mu?
// - Gece göğünde sağ üstteki Lyra takımyıldızının yıldızları görünüyor mu?
//
// CI'da Windows'ta Microsoft Edge ile çalışır: programın kullandığı WebView2 de Edge
// motorudur ve gölgelendiriciler Windows'ta (ANGLE → Direct3D) Linux'takinden farklı
// derlenir. Yerelde: SCENE_BROWSER_PATH=/yol/chromium npm run check:scenes
//
// Ortam değişkenleri:
//   SCENE_BROWSER_CHANNEL  ör. "msedge" (kurulu tarayıcı)
//   SCENE_BROWSER_PATH     tarayıcı dosyası (kanal verilmediyse)
//   SCENE_BROWSER_ARGS     ek tarayıcı argümanları (boşlukla ayrılmış)
//   SCENE_EXPECT           "webgl" (varsayılan) ya da "fallback": WebGL kapalı bir
//                          tarayıcıda gece göğünün 2D yedek çizimle (Lyra dahil) göründüğü denetlenir

import { chromium } from "playwright-core";
import { preview } from "vite";

const PORT = 4179;
const EXPECT_FALLBACK = process.env.SCENE_EXPECT === "fallback";
const URL = `http://localhost:${PORT}`;

/**
 * Tarayıcıda Tauri yerine geçen sahte çekirdek: 128 BPM'lik bir şarkı çalıyor. `level` verilirse bütün bantlar ve
 * VU seviyeleri sabit o düzeyde (parlama sınırı ölçümü için: 0 = sessiz, 1 = tam).
 */
const fakeCore = (level = null) => `
  const LEVEL = ${level === null ? "null" : level};
  window.isTauri = true;
  const started = performance.now();
  const track = { path: "C:/Müzik/deneme.flac", fileName: "deneme", title: "Deneme", artist: "Lyraska",
    codec: "flac", sampleRate: 44100, channels: 2, durationSecs: 240 };
  const status = { state: "playing", track, positionSecs: 60, underruns: 0, error: null, bpm: 128, output: null };
  window.__TAURI_INTERNALS__ = { transformCallback: () => 0, invoke: async (cmd) => {
    const t = (performance.now() - started) / 1000;
    const beats = t * 128 / 60;
    switch (cmd) {
      case "app_info": return { name: "Lyraska", version: "denetim", phase: "", audioEngine: "", analysis: "",
        visualBridge: "", supportedExtensions: ["flac"] };
      case "library_status": return { folders: [], trackCount: 0, analyzed: 0,
        scan: { scanning: false, found: 0, processed: 0, current: null }, problems: [] };
      case "library_search": return [];
      case "equalizer_get": return { enabled: true, gainsDb: Array(10).fill(0),
        bandsHz: [31.25, 62.5, 125, 250, 500, 1000, 2000, 4000, 8000, 16000], maxGainDb: 12, preampDb: 0,
        curveHz: [20, 20000], curveDb: [0, 0] };
      case "headphone_get": return { enabled: false, profile: null, curveHz: [], curveDb: [] };
      case "visual_safe_get": return false;
      case "audio_delay_get": return 0;
      case "song_map": case "annotation_get": return null;
      case "visual_frame": return {
        positionSecs: 60 + t,
        bands: Array.from({ length: 32 }, (_, i) =>
          LEVEL ?? Math.max(0, 0.85 - i * 0.015 + 0.12 * Math.sin(i + t * 4))),
        rmsDb: LEVEL === null ? [-12, -12] : LEVEL ? [0, 0] : [-60, -60],
        peakDb: LEVEL === null ? [-3, -3] : LEVEL ? [0, 0] : [-60, -60],
        vuReferenceDb: -14, energy: 0.8, section: 1,
        beat: { bpm: 128, index: Math.floor(beats), phase: beats % 1, barBeat: 1 + (Math.floor(beats) % 4), meter: 4 },
        director: {
          atmosphere: { section: 1, theme: 1, mood: 0.9, warmth: 0.6 },
          rhythm: { pulse: 0.5, accent: 0.2, beatPhase: beats % 1, barPhase: (beats / 4) % 1,
            anticipation: 0, release: 0.4 },
          texture: { detail: 0.6, motion: 0.6 },
        },
      };
      default: return status;
    }
  }};
`;

/** Tuvalin görüntüsünü çözüp parlaklık ölçer (bağıl parlaklık, 0..1). */
function measure(png) {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const c = document.createElement("canvas");
      c.width = img.width;
      c.height = img.height;
      const g = c.getContext("2d");
      g.drawImage(img, 0, 0);
      const d = g.getImageData(0, 0, c.width, c.height).data;
      const lin = (v) => {
        v /= 255;
        return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
      };
      const W = c.width;
      const H = c.height;
      const lum = new Float32Array(W * H);
      for (let i = 0; i < W * H; i++) {
        lum[i] = 0.2126 * lin(d[i * 4]) + 0.7152 * lin(d[i * 4 + 1]) + 0.0722 * lin(d[i * 4 + 2]);
      }
      const region = (x0, y0, x1, y1) => {
        let sum = 0;
        let max = 0;
        let n = 0;
        for (let y = Math.floor(y0 * H); y < Math.floor(y1 * H); y++) {
          for (let x = Math.floor(x0 * W); x < Math.floor(x1 * W); x++) {
            const l = lum[y * W + x];
            sum += l;
            max = Math.max(max, l);
            n++;
          }
        }
        return { mean: n ? sum / n : 0, max };
      };
      let sum = 0;
      let sq = 0;
      for (const l of lum) {
        sum += l;
        sq += l * l;
      }
      const mean = sum / lum.length;
      resolve({
        width: W,
        height: H,
        mean,
        spread: Math.sqrt(Math.max(0, sq / lum.length - mean * mean)),
        // Ekranın üstü sağ çeyreği (Lyra) ve ortası (kuzey ışıkları / ufuk).
        topRight: region(0.7, 0, 1, 0.45),
        middle: region(0.1, 0.25, 0.9, 0.6),
        pixels: Array.from(lum.filter((_, i) => i % 97 === 0)),
      });
    };
    img.src = "data:image/png;base64," + png;
  });
}

/**
 * İki görüntü arasındaki en büyük ortalama parlaklık farkı, WCAG'nin parlama alanı
 * büyüklüğündeki (1024×768 ekranda 341×256 piksel) en kötü pencerede.
 */
async function worstWindowDifference([before, after]) {
  const load = async (data) => {
    const img = new Image();
    img.src = "data:image/png;base64," + data;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = img.width;
    c.height = img.height;
    const g = c.getContext("2d");
    g.drawImage(img, 0, 0);
    return g.getImageData(0, 0, c.width, c.height);
  };
  const a = await load(before);
  const b = await load(after);
  const lin = (v) => {
    v /= 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  const lum = (d, i) => 0.2126 * lin(d[i]) + 0.7152 * lin(d[i + 1]) + 0.0722 * lin(d[i + 2]);
  const W = a.width;
  const H = a.height;
  const sum = new Float64Array((W + 1) * (H + 1));
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const i = (y * W + x) * 4;
      sum[(y + 1) * (W + 1) + x + 1] =
        lum(b.data, i) -
        lum(a.data, i) +
        sum[y * (W + 1) + x + 1] +
        sum[(y + 1) * (W + 1) + x] -
        sum[y * (W + 1) + x];
    }
  }
  const bw = Math.min(341, W);
  const bh = Math.min(256, H);
  let worst = 0;
  for (let y = 0; y + bh <= H; y += 4) {
    for (let x = 0; x + bw <= W; x += 4) {
      const s =
        sum[(y + bh) * (W + 1) + x + bw] -
        sum[y * (W + 1) + x + bw] -
        sum[(y + bh) * (W + 1) + x] +
        sum[y * (W + 1) + x];
      worst = Math.max(worst, Math.abs(s) / (bw * bh));
    }
  }
  return worst;
}

/** WCAG 2.3.1: bağıl parlaklıkta %10'dan küçük değişim parlama sayılmaz. */
const FLASH_DELTA = 0.1;

/**
 * Parlama sınırı: sahne müziğin en sessiz hâlinden en yüksek hâline geçerken bile
 * ekrandaki parlaklık farkı parlama eşiğinin altındaysa, hangi ritimde olursa olsun
 * parlama üretemez. (Gece göğü ve otoyolda ayrıca değişim hızı da sınırlıdır.)
 */
async function checkFlashBound(browser, scene) {
  const shots = [];
  for (const level of [0, 1]) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
    await page.addInitScript(
      `try { localStorage.setItem("lyraska.scene", "${scene.id}"); } catch {}\n${fakeCore(level)}`,
    );
    await page.goto(URL);
    await page.waitForTimeout(800);
    await page.mouse.click(5, 5);
    await page.keyboard.press("Space");
    await page.waitForTimeout(3000); // hız sınırlı değerler yerine otursun
    shots.push((await page.locator(".display").screenshot()).toString("base64"));
    await page.close();
  }
  const page = await browser.newPage();
  const worst = await page.evaluate(worstWindowDifference, shots);
  await page.close();
  console.log(`${scene.name}: sessiz → tam ses en büyük parlaklık farkı ${worst.toFixed(4)}`);
  return worst < FLASH_DELTA
    ? []
    : [`${scene.name}: parlaklık farkı ${worst.toFixed(3)} ≥ ${FLASH_DELTA} (parlama riski)`];
}

const SCENES = [
  {
    id: "sky",
    name: "Gece göğü",
    // Lyra'nın yıldızları: sağ üstte parlak noktalar (Vega en parlak).
    extra: (m) =>
      m.topRight.max > 0.35
        ? null
        : `Lyra yıldızları görünmüyor (en parlak ${m.topRight.max.toFixed(3)})`,
  },
  { id: "highway", name: "Gece otoyolu", extra: () => null },
];

/** Parlama sınırı ölçülen sahneler (hepsi). */
const ALL_SCENES = [
  { id: "spectrum", name: "Nokta matris spektrum" },
  { id: "vu", name: "VU ibreleri" },
  ...SCENES,
];

async function checkScene(browser, scene) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
  const problems = [];
  page.on("console", (m) => {
    // Yedek çizim denetiminde WebGL'in açılamadığı bildirimi beklenen bir şeydir.
    const expected = EXPECT_FALLBACK && m.text().includes("ekran kartında çizilemedi");
    if (m.type() === "error" && !expected) problems.push(`konsol: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`sayfa hatası: ${e}`));
  await page.addInitScript(
    `try { localStorage.setItem("lyraska.scene", "${scene.id}"); } catch {}\n${fakeCore()}`,
  );
  await page.goto(URL);
  await page.waitForTimeout(800);
  await page.mouse.click(5, 5);
  await page.keyboard.press("Space"); // çal
  await page.waitForTimeout(2500);

  const canvas = page.locator("canvas:not([hidden])").first();
  const state = await canvas.evaluate((c) => ({
    webgl: c.dataset.webgl,
    fallback: c.dataset.fallback ?? null,
    error: c.dataset.webglError ?? null,
    gpu: c.dataset.gpu ?? null,
  }));
  const first = await page.evaluate(measure, (await canvas.screenshot()).toString("base64"));
  await page.waitForTimeout(600);
  const second = await page.evaluate(measure, (await canvas.screenshot()).toString("base64"));
  // Hareket: örneklenen noktaların ne kadarı belirgin biçimde değişti (şerit çizgileri,
  // ışık perdeleri; gökyüzünün büyük kısmı durgundur).
  let moved = 0;
  for (let i = 0; i < first.pixels.length; i++) {
    if (Math.abs(first.pixels[i] - second.pixels[i]) > 0.005) moved++;
  }
  const change = moved / first.pixels.length;

  if (EXPECT_FALLBACK) {
    if (state.fallback !== "2d") problems.push("2D yedek çizim devreye girmedi");
  } else if (state.webgl !== "on") {
    problems.push(`WebGL2 çizimi açılmadı: ${state.error ?? "neden bilinmiyor"}`);
  }
  if (first.spread < 0.01) problems.push(`görüntü düz/boş (sapma ${first.spread.toFixed(4)})`);
  if (change < 0.003) {
    problems.push(`görüntü hareket etmiyor (değişen nokta %${(change * 100).toFixed(2)})`);
  }
  const extra = scene.extra(first);
  if (extra) problems.push(extra);

  console.log(
    `${scene.name}: WebGL ${state.webgl}${state.fallback ? ` (yedek: ${state.fallback})` : ""} · ekran kartı: ${state.gpu ?? "-"} · ${first.width}×${first.height} · ` +
      `ortalama ${first.mean.toFixed(4)} · sapma ${first.spread.toFixed(4)} · ` +
      `sağ üst en parlak ${first.topRight.max.toFixed(3)} · orta ${first.middle.mean.toFixed(4)} · ` +
      `değişen nokta %${(change * 100).toFixed(1)}`,
  );
  await page.close();
  return problems.map((p) => `${scene.name}: ${p}`);
}

const server = await preview({ preview: { port: PORT, strictPort: true }, logLevel: "warn" });
const channel = process.env.SCENE_BROWSER_CHANNEL;
const executablePath = channel ? undefined : process.env.SCENE_BROWSER_PATH;
const args = (process.env.SCENE_BROWSER_ARGS ?? "").split(/\s+/).filter(Boolean);
const browser = await chromium.launch({ channel, executablePath, args });
console.log(`Tarayıcı: ${browser.version()} ${args.length ? `(${args.join(" ")})` : ""}`);
let problems = [];
try {
  // Yedek çizim yalnızca gece göğünde var (Lyra orada).
  const scenes = EXPECT_FALLBACK ? SCENES.filter((s) => s.id === "sky") : SCENES;
  for (const scene of scenes) problems = problems.concat(await checkScene(browser, scene));
  if (!EXPECT_FALLBACK) {
    for (const scene of ALL_SCENES)
      problems = problems.concat(await checkFlashBound(browser, scene));
  }
} finally {
  await browser.close();
  await new Promise((resolve) => server.httpServer.close(resolve));
}
if (problems.length > 0) {
  console.error("\nSahne denetimi BAŞARISIZ:\n" + problems.map((p) => `  - ${p}`).join("\n"));
  process.exit(1);
}
console.log(
  EXPECT_FALLBACK
    ? "\nSahne denetimi tamam: WebGL olmadan da gece göğü ve Lyra görünüyor."
    : "\nSahne denetimi tamam: sahneler ekran kartında çiziliyor.",
);
