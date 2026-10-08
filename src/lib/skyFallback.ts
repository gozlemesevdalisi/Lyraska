import { mixColors, parseHexColor, SKY_THEMES, LYRA_LINES, type SkyState } from "./sky";
import { lyraPositions } from "./skyRenderer";

/**
 * "Gece göğü"nün yedek çizimi (Canvas 2D). Ekran kartı çizimi (WebGL2) açılamayan
 * bilgisayarlarda kullanılır: aynı gök, aynı Lyra takımyıldızı (gerçek gök
 * konumları, Vega en parlak), müziğe göre güçlenen üç ışık perdesi ve uzak tepeler.
 * Gölgelendiricideki kadar ayrıntılı değildir ama aynı durumu (`SkyState`) çizer;
 * parlaklık aynı hız sınırlı değerlerden gelir (epilepsi güvenliği korunur).
 */
export interface SkyFallback {
  draw(state: SkyState): void;
}

type Rgb = [number, number, number];

/** Perde şeridinin genişliği (piksel): küçük tutulur, perde akıcı görünsün. */
const STRIP_PX = 3;
const STAR_COUNT = 140;
/**
 * Perdelerin en yüksek opaklığı: ekran kartı çizimiyle aynı parlaklıkta kalacak
 * biçimde Chromium'da ölçülerek seçildi (ekranın ortasında en yüksek ~0,04 bağıl
 * parlaklık artışı; parlama eşiği 0,10).
 */
const AURORA_ALPHA = 0.25;

/** Perdenin alt kenarının ekrandaki yüksekliği (0 = alt, 1 = üst); gölgelendiricinin sadeleşmiş hâli. */
export function auroraCenter(
  x: number,
  curtain: number,
  flowTime: number,
  tension: number,
  bloom: number,
): number {
  const fi = curtain;
  const wave =
    0.06 * Math.sin(x * 1.3 + flowTime * 0.11 * (1 + 0.2 * fi) + fi * 2.1) +
    0.035 * Math.sin(x * 3.1 - flowTime * 0.07 * (1 + 0.3 * fi) + fi * 5.3);
  return 0.5 + 0.08 * fi + wave - 0.12 * tension + 0.05 * bloom;
}

/** Lyra'nın yıldızları ekran pikselinde (sol üst köşe başlangıç). */
export function lyraScreenStars(
  width: number,
  height: number,
): { x: number; y: number; brightness: number }[] {
  const positions = lyraPositions(width / Math.max(1, height));
  const out: { x: number; y: number; brightness: number }[] = [];
  for (let i = 0; i < positions.length; i += 3) {
    out.push({
      x: positions[i]! * height,
      y: (1 - positions[i + 1]!) * height,
      brightness: positions[i + 2]!,
    });
  }
  return out;
}

function context2d(canvas: HTMLCanvasElement): CanvasRenderingContext2D | null {
  try {
    return canvas.getContext("2d");
  } catch {
    return null;
  }
}

/** Tuvale 2D yedek çiziciyi kurar; 2D bağlam da alınamazsa `null`. */
export function createSkyFallback(canvas: HTMLCanvasElement): SkyFallback | null {
  const g = context2d(canvas);
  if (!g) return null;

  const style = window.getComputedStyle(canvas);
  const color = (name: string, fallback: Rgb) =>
    parseHexColor(style.getPropertyValue(name), fallback);
  const zenith = color("--sky-zenith", [0.01, 0.016, 0.043]);
  const horizon = color("--sky-horizon", [0.04, 0.1, 0.16]);
  const star = color("--sky-star", [0.91, 0.94, 1]);
  const ground = color("--sky-ground", [0.004, 0.008, 0.012]);
  const themes = Array.from({ length: SKY_THEMES }, (_, i) => ({
    low: color(`--sky-theme-${i}-low`, [0.27, 0.94, 0.65]),
    high: color(`--sky-theme-${i}-high`, [0.49, 0.36, 1]),
  }));
  // Yıldızlar sabit (her açılışta aynı yerde): basit sözde rastgele dizi.
  let seed = 7;
  const random = () => {
    seed = (seed * 16807) % 2147483647;
    return seed / 2147483647;
  };
  const stars = Array.from({ length: STAR_COUNT }, () => ({
    x: random(),
    y: random() * 0.85,
    size: 0.5 + random() * 0.9,
    phase: random() * Math.PI * 2,
    base: 0.25 + random() * 0.6,
  }));
  const ridge = Array.from(
    { length: 64 },
    (_, i) => 0.06 + 0.1 * random() * (0.5 + 0.5 * Math.sin(i * 0.7)),
  );

  const css = ([r, gr, b]: Rgb, alpha = 1) =>
    `rgba(${Math.round(r * 255)}, ${Math.round(gr * 255)}, ${Math.round(b * 255)}, ${alpha})`;

  return {
    draw(state) {
      const pixel = Math.min(window.devicePixelRatio || 1, 2);
      const width = Math.max(1, Math.round(canvas.clientWidth * pixel));
      const height = Math.max(1, Math.round(canvas.clientHeight * pixel));
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }
      g.setTransform(1, 0, 0, 1, 0, 0);
      g.globalCompositeOperation = "source-over";

      // Gök: ufuktan tepeye.
      const sky = g.createLinearGradient(0, height, 0, 0);
      sky.addColorStop(0.05, css(horizon));
      sky.addColorStop(0.95, css(zenith));
      g.fillStyle = sky;
      g.fillRect(0, 0, width, height);

      // Yıldızlar (yavaş kırpışma; parlamaz).
      for (const s of stars) {
        const twinkle = 0.85 + 0.15 * Math.sin(state.twinkleTime * 1.3 + s.phase);
        g.fillStyle = css(star, s.base * twinkle * 0.8);
        g.fillRect(s.x * width, s.y * height, s.size * pixel, s.size * pixel);
      }

      // Lyra: soluk çizgiler ve parlak yıldızlar.
      const lyra = lyraScreenStars(width, height);
      g.strokeStyle = css(star, 0.12);
      g.lineWidth = pixel;
      g.beginPath();
      for (const [a, b] of LYRA_LINES) {
        g.moveTo(lyra[a]!.x, lyra[a]!.y);
        g.lineTo(lyra[b]!.x, lyra[b]!.y);
      }
      g.stroke();
      for (const s of lyra) {
        const glow = g.createRadialGradient(s.x, s.y, 0, s.x, s.y, (2 + 6 * s.brightness) * pixel);
        glow.addColorStop(0, css(star, 0.6 + 0.4 * s.brightness));
        glow.addColorStop(0.25, css(star, 0.35 * s.brightness));
        glow.addColorStop(1, css(star, 0));
        g.fillStyle = glow;
        g.fillRect(s.x - 8 * pixel, s.y - 8 * pixel, 16 * pixel, 16 * pixel);
      }

      // Kuzey ışıkları: üç perde, şeritler hâlinde (ışıklar üst üste eklenir).
      const { from, to, mix } = state.palette;
      const a = themes[from % themes.length]!;
      const b = themes[to % themes.length]!;
      const low = mixColors(a.low, b.low, mix);
      const high = mixColors(a.high, b.high, mix);
      g.globalCompositeOperation = "lighter";
      const aspect = width / height;
      const strip = STRIP_PX * pixel;
      for (let i = 0; i < 3; i++) {
        const strength = 0.25 + 0.75 * state.energies[i]!;
        const alpha = AURORA_ALPHA * strength * (1 - 0.22 * i) * (1 + 0.15 * state.bloom);
        const tail = (0.16 + 0.04 * i) * (1 - 0.45 * state.tension) * (1 + 0.12 * state.bloom);
        // Perde yerel biriminde (1 birim = perde uzunluğu, yukarı eksi): alt kenarda
        // (0) tam renk, yukarı doğru üstel sönüm, alt kenarın hemen altında keskin bitiş.
        const curtain = g.createLinearGradient(0, -3, 0, 0.25);
        curtain.addColorStop(0, css(high, 0));
        curtain.addColorStop(0.615, css(mixColors(low, high, 0.6), alpha * 0.37));
        curtain.addColorStop(0.923, css(low, alpha));
        curtain.addColorStop(1, css(low, 0));
        g.fillStyle = curtain;
        const tailPx = tail * height;
        for (let x = 0; x < width; x += strip) {
          const units = (x / width) * aspect * (0.85 + 0.2 * i) + i * 3.7;
          const center = auroraCenter(units, i, state.flowTime, state.tension, state.bloom);
          // Perdenin dikey ışınları: şeritten şeride değişen yoğunluk.
          const rays = 0.55 + 0.45 * Math.sin(units * 23 + state.flowTime * 0.4 + i * 1.7);
          g.globalAlpha = Math.max(0, Math.min(1, rays));
          g.setTransform(1, 0, 0, tailPx, x, (1 - center) * height);
          g.fillRect(0, -3, strip + 0.5, 3.25);
        }
        g.setTransform(1, 0, 0, 1, 0, 0);
      }
      g.globalAlpha = 1;
      g.globalCompositeOperation = "source-over";

      // Uzak tepeler.
      g.fillStyle = css(ground);
      g.beginPath();
      g.moveTo(0, height);
      ridge.forEach((r, i) => g.lineTo((i / (ridge.length - 1)) * width, (1 - r) * height));
      g.lineTo(width, height);
      g.closePath();
      g.fill();
    },
  };
}
