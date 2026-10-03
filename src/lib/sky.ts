/**
 * "Gece göğü" sahnesi için saf hesaplar: kuzey ışıklarını süren enerjiler,
 * hareket hızı, renk ayrıştırma ve Lyra takımyıldızı.
 *
 * Epilepsi güvenliği: ekranın parlaklığını değiştiren her değer (perde
 * enerjileri) saniyede en fazla `ENERGY_SLEW_PER_SECOND` kadar değişir. Müziğin
 * vuruşları parlaklığa değil, ışıkların akış hızına yansır; hız değişimi
 * parlama üretmez.
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
}

export const SKY_AT_REST: SkyState = {
  energies: [0, 0, 0],
  pulse: 0,
  flowTime: 0,
  twinkleTime: 0,
};

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
): SkyState {
  const step = Math.max(0, Math.min(dt, 0.25)); // sekme/uyku sonrası sıçramayı sınırla
  const goal = targets ?? [0, 0, 0];
  const energies = prev.energies.map((energy, i) =>
    slew(energy, smooth(energy, clamp01(goal[i] ?? 0), step), ENERGY_SLEW_PER_SECOND * step),
  ) as Energies;

  const bass = clamp01(goal[0] ?? 0);
  const pulse =
    prev.pulse +
    (bass - prev.pulse) *
      (1 - Math.exp(-step / (bass > prev.pulse ? PULSE_ATTACK_SECONDS : PULSE_RELEASE_SECONDS)));

  const motion = reducedMotion ? REDUCED_MOTION_SPEED : 1;
  return {
    energies,
    pulse,
    flowTime: (prev.flowTime + step * (IDLE_SPEED + PULSE_SPEED * pulse) * motion) % 10_000,
    // "Animasyonları azalt" açıkken yıldızlar kırpışmaz.
    twinkleTime: reducedMotion ? prev.twinkleTime : (prev.twinkleTime + step) % 10_000,
  };
}

/** Perdenin ekrana kattığı göreli parlaklık (gölgelendiricideki formülün üst sınırı). */
export function auroraLuminance(energies: Energies): number {
  const strength = energies.reduce((sum, e) => sum + AURORA_BASE + (1 - AURORA_BASE) * e, 0) / 3;
  return AURORA_MAX_LUMINANCE * strength;
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

function smooth(current: number, target: number, dt: number): number {
  const tau = target > current ? ENERGY_ATTACK_SECONDS : ENERGY_RELEASE_SECONDS;
  return current + (target - current) * (1 - Math.exp(-dt / tau));
}

function slew(current: number, target: number, maxStep: number): number {
  return current + Math.max(-maxStep, Math.min(maxStep, target - current));
}

function clamp01(x: number): number {
  return Number.isFinite(x) ? Math.max(0, Math.min(1, x)) : 0;
}
