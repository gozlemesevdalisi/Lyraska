/**
 * "Gece göğü" sahnesi için saf hesaplar: kuzey ışıklarını süren enerjiler,
 * hareket hızı, renk ayrıştırma ve Lyra takımyıldızı.
 *
 * Görsel Yönetmen bağlıysa (`DirectorInput`): bölüm teması perdelerin rengini,
 * ruh hâli parlaklığını belirler; droptan önce perdeler toplanıp ufka çekilir
 * (gerilim), drop'ta genişleyip yükselir (açılım); ölçü başlarında perdede bir
 * dalga akar.
 *
 * Epilepsi güvenliği: ekranın parlaklığını değiştiren her değer (perde
 * enerjileri ve açılım) saniyede en fazla `ENERGY_SLEW_PER_SECOND` kadar değişir
 * (güvenli modda yarısı). Müziğin vuruşları parlaklığa değil, ışıkların akış
 * hızına ve şekline yansır; bunlar parlama üretmez.
 */

/** Sırasıyla bas, orta ve tiz perdenin enerjisi (0..1). */
export type Energies = [number, number, number];

export interface SkyState {
  /** Gösterilen perde enerjileri (parlaklık; hız sınırlı). */
  energies: Energies;
  /** Bas vuruşlarına hızlı tepki veren seviye (yalnızca akış hızını sürer). */
  pulse: number;
  /** Işıkların akış zamanı (saniye; hıza göre ilerler). */
  flowTime: number;
  /** Yıldızların göz kırpma zamanı (saniye). */
  twinkleTime: number;
  /** Droptan önceki gerilim (0..1; yavaş). */
  tension: number;
  /** Drop açılımı (0..1; parlaklığı etkiler, hız sınırlı). */
  bloom: number;
  /** Renk teması geçişi: eski tema, yeni tema ve karışım (0 → 1). */
  palette: Palette;
  /** Son ölçü başından beri geçen süre (saniye; perdedeki dalga için). */
  rippleTime: number;
  /** Son karede okunan ölçü vurgusu (yeni vurguyu ayırt etmek için). */
  lastAccent: number;
}

export const SKY_AT_REST: SkyState = {
  energies: [0, 0, 0],
  pulse: 0,
  flowTime: 0,
  twinkleTime: 0,
  tension: 0,
  bloom: 0,
  palette: { from: 0, to: 0, mix: 1 },
  rippleTime: 100,
  lastAccent: 0,
};

/** Gökyüzünün Görsel Yönetmen'den kullandığı değerler (`DirectorFrame`'in alt kümesi). */
export interface DirectorInput {
  mood: number;
  theme: number;
  pulse: number;
  accent: number;
  anticipation: number;
  release: number;
}

/** Tema renk paleti sayısı (`--sky-theme-N-*` değişkenleri). */
export const SKY_THEMES = 4;
/** Tema geçişi süresi (saniye). */
const PALETTE_SECONDS = 2;
/** Gerilimin değişim hızı (tam ölçek / saniye). */
const TENSION_SLEW_PER_SECOND = 0.8;
/** Açılımın perde parlaklığına katkısı ve parlaklık payı. */
const BLOOM_ENERGY = 0.35;
export const BLOOM_LUMINANCE = 0.15;

/** Perde enerjisinin en büyük değişim hızı (tam ölçek / saniye). */
export const ENERGY_SLEW_PER_SECOND = 1.2;
/** Enerjinin yükselişte ve düşüşte yumuşatma süreleri (saniye). */
const ENERGY_ATTACK_SECONDS = 0.12;
const ENERGY_RELEASE_SECONDS = 0.7;
const PULSE_ATTACK_SECONDS = 0.05;
const PULSE_RELEASE_SECONDS = 0.35;

/** Akış hızı: boştayken `IDLE_SPEED`, en güçlü vuruşta `IDLE_SPEED + PULSE_SPEED`. */
const IDLE_SPEED = 0.35;
const PULSE_SPEED = 1.3;
/** "Animasyonları azalt" açıkken hareket bu oranda yavaşlar. */
const REDUCED_MOTION_SPEED = 0.25;

/**
 * Perdenin ekrana katabileceği bağıl parlaklık için güvenli üst sınır (parlama
 * testinde kullanılır). Chromium'da ölçüldü: perdeler sönükten en parlağa
 * çıkınca, WCAG'nin parlama alanı büyüklüğündeki en parlak bölgede bağıl
 * parlaklık yalnızca ~0,02 artıyor (parlama eşiği 0,10). Bu sınır ölçülenin
 * çok üstünde seçildi: renkler ya da gölgelendirici değişse de test korur.
 */
export const AURORA_MAX_LUMINANCE = 0.35;
/** Enerji sıfırken perdenin parlaklık payı (tamamen sönmez). */
export const AURORA_BASE = 0.25;

/** Spektrum bant seviyelerini (bastan tize, 0..1) üç perde enerjisine çevirir. */
export function bandEnergies(bands: readonly number[]): Energies {
  if (bands.length === 0) return [0, 0, 0];
  const bassEnd = Math.max(1, Math.round(bands.length * 0.2));
  const midEnd = Math.max(bassEnd + 1, Math.round(bands.length * 0.58));
  return [
    contrast(mean(bands, 0, bassEnd)),
    contrast(mean(bands, bassEnd, midEnd)),
    contrast(mean(bands, midEnd, bands.length)),
  ];
}

/**
 * Göğü `dt` saniye ilerletir. `targets` o an duyulan anın enerjileridir;
 * çalmıyorken `null` (ışıklar dinlenme parlaklığına iner, akış sürer).
 */
export function stepSky(
  prev: SkyState,
  targets: Energies | null,
  dt: number,
  reducedMotion = false,
  director: DirectorInput | null = null,
  safe = false,
): SkyState {
  const step = Math.max(0, Math.min(dt, 0.25)); // sekme/uyku sonrası sıçramayı sınırla
  const slewRate = (safe ? 0.5 : 1) * ENERGY_SLEW_PER_SECOND * step;
  const raw = targets ?? [0, 0, 0];
  // Yönetmen varsa: sakin bölümde perdeler sönük, yoğun bölümde canlı; drop'ta açılım eklenir.
  const goal = raw.map((e) =>
    director && targets
      ? clamp01(
          e * (0.55 + 0.45 * clamp01(director.mood)) + BLOOM_ENERGY * clamp01(director.release),
        )
      : clamp01(e),
  );
  const energies = prev.energies.map((energy, i) =>
    slew(energy, smooth(energy, goal[i] ?? 0, step), slewRate),
  ) as Energies;

  const bass = clamp01(raw[0] ?? 0);
  const pulse =
    prev.pulse +
    (bass - prev.pulse) *
      (1 - Math.exp(-step / (bass > prev.pulse ? PULSE_ATTACK_SECONDS : PULSE_RELEASE_SECONDS)));

  const playingDirector = director && targets ? director : null;
  const tension = slew(
    prev.tension,
    clamp01(playingDirector?.anticipation ?? 0),
    TENSION_SLEW_PER_SECOND * step,
  );
  const bloom = slew(prev.bloom, clamp01(playingDirector?.release ?? 0), slewRate);

  const palette = stepPalette(prev.palette, playingDirector?.theme ?? null, step);

  // Ölçü başı dalgası: yeni vurgu gelince baştan başlar ("animasyonları azalt"ta yok).
  const accent = playingDirector?.accent ?? 0;
  const freshAccent = accent > prev.lastAccent + 0.3 && !reducedMotion;
  const rippleTime = freshAccent ? 0 : Math.min(100, prev.rippleTime + step);

  // Akış: vuruşta hızlanır; gerilimde yavaşlar, açılımda hızlanır.
  const directorPulse = playingDirector?.pulse ?? 0;
  const speed =
    (IDLE_SPEED + PULSE_SPEED * Math.max(pulse, 0.6 * directorPulse)) *
    (1 - 0.6 * tension) *
    (1 + 1.5 * bloom);
  const motion = reducedMotion ? REDUCED_MOTION_SPEED : 1;
  return {
    energies,
    pulse,
    flowTime: (prev.flowTime + step * speed * motion) % 10_000,
    // "Animasyonları azalt" açıkken yıldızlar kırpışmaz.
    twinkleTime: reducedMotion ? prev.twinkleTime : (prev.twinkleTime + step) % 10_000,
    tension,
    bloom,
    palette,
    rippleTime,
    lastAccent: accent,
  };
}

/** Renk teması geçişi: eski tema, yeni tema ve karışım (0 → 1). */
export interface Palette {
  from: number;
  to: number;
  mix: number;
}

/**
 * Tema geçişini `dt` saniye ilerletir: yeni tema gelince eskisinden
 * `PALETTE_SECONDS` içinde yumuşakça geçilir. `theme` `null` ise tema korunur.
 */
export function stepPalette(prev: Palette, theme: number | null, dt: number): Palette {
  const target = theme === null ? prev.to : Math.abs(Math.trunc(theme)) % SKY_THEMES;
  let palette = prev;
  // Geçiş yarıda kesilirse çoğunluktaki temadan devam edilir.
  if (target !== palette.to) {
    palette = { from: palette.mix >= 0.5 ? palette.to : palette.from, to: target, mix: 0 };
  }
  return { ...palette, mix: Math.min(1, palette.mix + Math.max(0, dt) / PALETTE_SECONDS) };
}

/** İki tema rengi arasındaki karışım (0..1 RGB). */
export function mixColors(
  a: [number, number, number],
  b: [number, number, number],
  t: number,
): [number, number, number] {
  const k = clamp01(t);
  return [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k];
}

/**
 * Perdenin ekrana kattığı göreli parlaklık (gölgelendiricideki formülün üst sınırı).
 * Açılım perdeleri genişletir: aydınlık alan, dolayısıyla parlaklık da artar.
 */
export function auroraLuminance(energies: Energies, bloom = 0): number {
  const strength = energies.reduce((sum, e) => sum + AURORA_BASE + (1 - AURORA_BASE) * e, 0) / 3;
  return (
    (AURORA_MAX_LUMINANCE * strength * (1 + BLOOM_LUMINANCE * clamp01(bloom))) /
    (1 + BLOOM_LUMINANCE)
  );
}

/**
 * CSS renk değerini ("#rgb", "#rrggbb") 0..1 aralığında RGB'ye çevirir.
 * Okunamazsa `fallback` döner.
 */
export function parseHexColor(value: string, fallback: [number, number, number]) {
  const hex = value.trim().replace(/^#/, "");
  const full =
    hex.length === 3
      ? hex
          .split("")
          .map((c) => c + c)
          .join("")
      : hex;
  if (!/^[0-9a-f]{6}$/i.test(full)) return fallback;
  return [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16) / 255) as [number, number, number];
}

/**
 * Lyra takımyıldızı (lir): gök koordinatlarından düzleme açılmış konumlar
 * (derece; Vega merkezde, doğu solda) ve gösterilecek parlaklık.
 */
export const LYRA_STARS: { name: string; x: number; y: number; brightness: number }[] = [
  { name: "Vega", x: 0, y: 0, brightness: 1 },
  { name: "Epsilon", x: -1.44, y: 0.89, brightness: 0.4 },
  { name: "Zeta", x: -1.56, y: -1.18, brightness: 0.42 },
  { name: "Delta", x: -3.52, y: -1.88, brightness: 0.42 },
  { name: "Gamma", x: -4.65, y: -6.09, brightness: 0.58 },
  { name: "Beta", x: -2.76, y: -5.42, brightness: 0.52 },
];

/** Takımyıldızın çizgileri (`LYRA_STARS` sıra numaralarıyla). */
export const LYRA_LINES: [number, number][] = [
  [0, 1],
  [0, 2],
  [2, 3],
  [3, 4],
  [4, 5],
  [5, 2],
];

function mean(values: readonly number[], from: number, to: number): number {
  let sum = 0;
  for (let i = from; i < to; i++) sum += values[i] ?? 0;
  return to > from ? sum / (to - from) : 0;
}

/** Bant seviyeleri müzikte çoğunlukla ortada gezinir; aralığı açarak farkı görünür kıl. */
function contrast(level: number): number {
  return clamp01((level - 0.2) / 0.6);
}

/** Enerji yumuşatma: yükselişte hızlı, düşüşte yavaş. */
export function smooth(current: number, target: number, dt: number): number {
  const tau = target > current ? ENERGY_ATTACK_SECONDS : ENERGY_RELEASE_SECONDS;
  return current + (target - current) * (1 - Math.exp(-dt / tau));
}

/** `target`'a doğru en fazla `maxStep` kadar ilerler (parlaklık hız sınırı). */
export function slew(current: number, target: number, maxStep: number): number {
  return current + Math.max(-maxStep, Math.min(maxStep, target - current));
}

export function clamp01(x: number): number {
  return Number.isFinite(x) ? Math.max(0, Math.min(1, x)) : 0;
}
