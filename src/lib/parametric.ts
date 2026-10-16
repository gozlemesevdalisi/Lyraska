import type { FilterKind, ParametricBand, ParametricSettings, ParametricState } from "./backend";

/** Grafiğin frekans aralığı (Hz) ve varsayılan kazanç aralığı (±dB). */
export const PEQ_MIN_HZ = 20;
export const PEQ_MAX_HZ = 20000;
export const PEQ_RANGE_DB = 18;
/** Çekirdeğin sınırları (durum gelmeden önce de kullanılır; çekirdekteki `sanitized` ile aynı). */
export const PEQ_LIMITS = { maxBands: 20, maxGainDb: 24, minQ: 0.1, maxQ: 30 };
/** Frekansın kabul edilen aralığı (Hz). */
export const PEQ_FREQ_MIN = 10;
export const PEQ_FREQ_MAX = 24000;

/** Türlerin kısa adları (Equalizer APO kısaltmaları) ve açıklamaları. */
export const PEQ_KINDS: { kind: FilterKind; short: string; name: string }[] = [
  { kind: "peaking", short: "PK", name: "Tepe / çukur" },
  { kind: "lowShelf", short: "LS", name: "Alçak raf" },
  { kind: "highShelf", short: "HS", name: "Yüksek raf" },
  { kind: "lowPass", short: "LP", name: "Alçak geçiren" },
  { kind: "highPass", short: "HP", name: "Yüksek geçiren" },
  { kind: "bandPass", short: "BP", name: "Bant geçiren" },
  { kind: "notch", short: "NO", name: "Çentik" },
  { kind: "allPass", short: "AP", name: "Tüm geçiren" },
];

export function kindLabel(kind: FilterKind): string {
  return PEQ_KINDS.find((k) => k.kind === kind)?.name ?? kind;
}

/** Kazanç parametresi olan türler. */
export function kindHasGain(kind: FilterKind): boolean {
  return kind === "peaking" || kind === "lowShelf" || kind === "highShelf";
}

/** Çekirdeğin varsayılanı: düz beş bant (alçak raf, üç tepe, yüksek raf), eşlenmiş tasarım. */
export const DEFAULT_PARAMETRIC: ParametricSettings = {
  enabled: true,
  bands: [
    { kind: "lowShelf", freqHz: 105, gainDb: 0, q: Math.SQRT1_2, enabled: true },
    { kind: "peaking", freqHz: 250, gainDb: 0, q: 1, enabled: true },
    { kind: "peaking", freqHz: 1000, gainDb: 0, q: 1, enabled: true },
    { kind: "peaking", freqHz: 4000, gainDb: 0, q: 1, enabled: true },
    { kind: "highShelf", freqHz: 10000, gainDb: 0, q: Math.SQRT1_2, enabled: true },
  ],
  design: "matched",
};

/** Frekansın yatay konumu (0..1, logaritmik). */
export function hzToX(hz: number): number {
  const ratio = Math.log(hz / PEQ_MIN_HZ) / Math.log(PEQ_MAX_HZ / PEQ_MIN_HZ);
  return Math.min(1, Math.max(0, ratio));
}

/** Yatay konumdan frekans (Hz). */
export function xToHz(x: number): number {
  const clamped = Math.min(1, Math.max(0, x));
  return PEQ_MIN_HZ * Math.pow(PEQ_MAX_HZ / PEQ_MIN_HZ, clamped);
}

/** Kazancın dikey konumu (0 üst, 1 alt). */
export function dbToY(db: number, rangeDb = PEQ_RANGE_DB): number {
  const clamped = Math.min(rangeDb, Math.max(-rangeDb, db));
  return 0.5 - clamped / (2 * rangeDb);
}

/** Dikey konumdan kazanç (dB). */
export function yToDb(y: number, rangeDb = PEQ_RANGE_DB): number {
  const clamped = Math.min(1, Math.max(0, y));
  return (0.5 - clamped) * 2 * rangeDb;
}

/** Frekansı anlamlı adıma yuvarlar: <100 Hz 0,1; <1 kHz 1; <10 kHz 10; üstü 100 Hz. */
export function roundHz(hz: number): number {
  const clamped = Math.min(PEQ_FREQ_MAX, Math.max(PEQ_FREQ_MIN, hz));
  const step = clamped < 100 ? 0.1 : clamped < 1000 ? 1 : clamped < 10000 ? 10 : 100;
  return Math.round(Math.round(clamped / step) * step * 10) / 10;
}

export function roundGain(db: number, maxGainDb = PEQ_LIMITS.maxGainDb): number {
  return Math.round(Math.min(maxGainDb, Math.max(-maxGainDb, db)) * 10) / 10;
}

export function roundQ(q: number, minQ = PEQ_LIMITS.minQ, maxQ = PEQ_LIMITS.maxQ): number {
  const clamped = Math.min(maxQ, Math.max(minQ, q));
  return clamped < 1 ? Math.round(clamped * 1000) / 1000 : Math.round(clamped * 100) / 100;
}

/** Q'yu logaritmik adımla değiştirir (fare tekerleği, +/− tuşları): 12 adım bir oktav (×2). */
export function stepQ(q: number, steps: number): number {
  return roundQ(q * Math.pow(2, steps / 12));
}

/** Frekansı oktavın kesri kadar kaydırır. */
export function stepHz(hz: number, octaves: number): number {
  return roundHz(hz * Math.pow(2, octaves));
}

/** Q'dan bant genişliği (oktav): BW = (2/ln2)·asinh(1/(2Q)). */
export function qToOctaves(q: number): number {
  return (2 / Math.LN2) * Math.asinh(1 / (2 * q));
}

const number = (value: number, digits: number) =>
  value.toLocaleString("tr-TR", { maximumFractionDigits: digits });

/** "86,3 Hz", "1,25 kHz", "12,5 kHz". */
export function formatFrequency(hz: number): string {
  if (hz >= 1000) return `${number(hz / 1000, hz >= 10000 ? 1 : 2)} kHz`;
  return `${number(hz, hz < 100 ? 1 : 0)} Hz`;
}

/** Q: "0,707", "1,41". */
export function formatQ(q: number): string {
  return number(q, q < 1 ? 3 : 2);
}

/** Kazanç: "+3,5", "−2", "0". */
export function formatDb(db: number): string {
  const rounded = Math.round(db * 10) / 10;
  if (rounded === 0) return "0";
  return `${rounded > 0 ? "+" : "−"}${number(Math.abs(rounded), 1)}`;
}

/** Türkçe ondalık virgülü ve "k" ekiyle yazılmış frekansı okur ("1,2k" → 1200). */
export function parseFrequency(text: string): number | null {
  const cleaned = text.trim().toLowerCase().replace(/\s|hz/g, "").replace(",", ".");
  const kilo = cleaned.endsWith("k");
  const value = Number(kilo ? cleaned.slice(0, -1) : cleaned);
  if (cleaned === "" || !Number.isFinite(value) || value <= 0) return null;
  return kilo ? value * 1000 : value;
}

/** Türkçe ondalık virgüllü, "−" işaretli de olabilen sayı. */
export function parseDecimal(text: string): number | null {
  const cleaned = text.trim().replace(/\s|db/gi, "").replace(",", ".").replace("−", "-");
  if (cleaned === "") return null;
  const value = Number(cleaned);
  return Number.isFinite(value) ? value : null;
}

/** Yeni bant: tıklanan yerde tepe süzgeci (Q ≈ 1 oktav). */
export function newBand(hz: number, gainDb: number): ParametricBand {
  return {
    kind: "peaking",
    freqHz: roundHz(hz),
    gainDb: roundGain(gainDb),
    q: 1.41,
    enabled: true,
  };
}

/** Bandın sese dokunup dokunmadığı (çekirdekteki `is_effective` ile aynı). */
export function bandIsEffective(band: ParametricBand): boolean {
  return band.enabled && (!kindHasGain(band.kind) || Math.abs(band.gainDb) >= 0.005);
}

/** Ayar sesi değiştiriyor mu? */
export function parametricIsActive(settings: ParametricSettings): boolean {
  return settings.enabled && settings.bands.some(bandIsEffective);
}

// Analog prototipin genliği (çekirdekteki `biquad::Spec::analog` ile aynı formüller). Tarayıcı
// önizlemesinde ve sürüklerken anında çizim için; programda eğriyi çekirdek hesaplar.

type C = [number, number];
const add = (a: C, b: C): C => [a[0] + b[0], a[1] + b[1]];
const sub = (a: C, b: C): C => [a[0] - b[0], a[1] - b[1]];
const mul = (a: C, b: C): C => [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]];
const scale = (a: C, k: number): C => [a[0] * k, a[1] * k];
const abs2 = (a: C) => a[0] * a[0] + a[1] * a[1];

/** Analog prototipin `hz`'teki kazancı (dB). */
export function analogDb(band: ParametricBand, hz: number): number {
  const s: C = [0, hz / band.freqHz];
  const s2 = mul(s, s);
  const one: C = [1, 0];
  const a = Math.pow(10, (kindHasGain(band.kind) ? band.gainDb : 0) / 40);
  const q = band.q;
  let num: C;
  let den: C;
  switch (band.kind) {
    case "peaking":
      num = add(add(s2, scale(s, a / q)), one);
      den = add(add(s2, scale(s, 1 / (a * q))), one);
      break;
    case "lowShelf": {
      const k = Math.sqrt(a) / q;
      num = scale(add(add(s2, scale(s, k)), [a, 0]), a);
      den = add(add(scale(s2, a), scale(s, k)), one);
      break;
    }
    case "highShelf": {
      const k = Math.sqrt(a) / q;
      num = scale(add(add(scale(s2, a), scale(s, k)), one), a);
      den = add(add(s2, scale(s, k)), [a, 0]);
      break;
    }
    case "lowPass":
      num = one;
      den = add(add(s2, scale(s, 1 / q)), one);
      break;
    case "highPass":
      num = s2;
      den = add(add(s2, scale(s, 1 / q)), one);
      break;
    case "bandPass":
      num = scale(s, 1 / q);
      den = add(add(s2, scale(s, 1 / q)), one);
      break;
    case "notch":
      num = add(s2, one);
      den = add(add(s2, scale(s, 1 / q)), one);
      break;
    case "allPass":
      num = add(sub(s2, scale(s, 1 / q)), one);
      den = add(add(s2, scale(s, 1 / q)), one);
      break;
  }
  return 10 * Math.log10(Math.max(abs2(num), 1e-30) / Math.max(abs2(den), 1e-30));
}

/** Eğri frekansları: 20 Hz – 20 kHz, logaritmik (çekirdekle aynı 241 nokta). */
export const PEQ_CURVE_HZ: number[] = Array.from(
  { length: 241 },
  (_, i) => PEQ_MIN_HZ * Math.pow(PEQ_MAX_HZ / PEQ_MIN_HZ, i / 240),
);

/**
 * Tarayıcı önizlemesi ve sürüklerken anında çizim için yaklaşık durum: analog prototiplerin
 * eğrileri. (Gerçek sayısal eğriyi, korumayı ve ses yüksekliği farkını çekirdek hesaplar.)
 */
export function previewParametricState(
  settings: ParametricSettings,
  comparing = false,
): ParametricState {
  const bandCurvesDb = settings.bands.map((band) => PEQ_CURVE_HZ.map((hz) => analogDb(band, hz)));
  const curveDb = PEQ_CURVE_HZ.map((_, i) =>
    settings.enabled
      ? settings.bands.reduce(
          (sum, band, b) => sum + (bandIsEffective(band) ? bandCurvesDb[b]![i]! : 0),
          0,
        )
      : 0,
  );
  return {
    settings,
    comparing: comparing && parametricIsActive(settings),
    curveHz: PEQ_CURVE_HZ,
    curveDb,
    bandCurvesDb,
    boostDb: Math.max(0, ...curveDb),
    loudnessDb: 0,
    ...PEQ_LIMITS,
  };
}

/** Eğriyi SVG yoluna çevirir (genişlik `width`, yükseklik `height`, ±`rangeDb`). */
export function peqPath(
  hz: number[],
  db: number[],
  width: number,
  height: number,
  rangeDb = PEQ_RANGE_DB,
): string {
  const points = hz.map((f, i) => {
    const x = hzToX(f) * width;
    const y = dbToY(db[i] ?? 0, rangeDb) * height;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });
  return points.length > 0 ? `M${points.join("L")}` : "";
}

/** Eğriyi gösterecek dikey aralık: en az ±18 dB, gerekirse 6 dB'lik adımlarla ±30'a kadar. */
export function plotRangeDb(curves: number[][]): number {
  const peak = Math.max(0, ...curves.flat().map((db) => Math.abs(db)));
  return Math.min(30, Math.max(PEQ_RANGE_DB, Math.ceil((peak + 1) / 6) * 6));
}

/** Bir bandın grafikteki düğümünün kazancı: kazançlı türde kendi kazancı, ötekilerde eğrinin değeri. */
export function nodeDb(band: ParametricBand): number {
  return kindHasGain(band.kind)
    ? band.gainDb
    : Math.max(-PEQ_RANGE_DB, analogDb(band, band.freqHz));
}
