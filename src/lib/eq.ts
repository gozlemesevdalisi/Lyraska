import type { EqSettings, EqState } from "./backend";

/** Bant orta frekansları (Hz); Rust tarafındaki `CENTERS_HZ` ile aynı. */
export const EQ_BANDS_HZ = [31.25, 62.5, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
export const EQ_MAX_DB = 12;
/** Sürgü adımı (dB). */
export const EQ_STEP_DB = 0.5;
/** Eğri ekseninin frekans aralığı (Hz). */
export const CURVE_MIN_HZ = 20;
export const CURVE_MAX_HZ = 20000;

export interface EqPreset {
  name: string;
  /** Kısa açıklama (düğmenin ipucu). */
  hint: string;
  gains: number[];
}

/** Hazır ayarlar. Değerler özgündür; ±6 dB içinde, kulağı yormayan eğriler. */
export const EQ_PRESETS: EqPreset[] = [
  { name: "Düz", hint: "Ses olduğu gibi", gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  { name: "Bas", hint: "Derin bas", gains: [6, 5.5, 4.5, 2.5, 0.5, 0, 0, 0, 0, 0] },
  { name: "Tiz", hint: "Parlak, net tizler", gains: [0, 0, 0, 0, 0, 0.5, 2, 4, 5.5, 6] },
  { name: "Vokal", hint: "Sesler önde", gains: [-2, -2, -1, 0.5, 2, 3.5, 3.5, 2, 0, -1] },
  { name: "Akustik", hint: "Sıcak ve doğal", gains: [3, 3, 2, 1, 1, 1, 2, 2.5, 3, 2] },
  {
    name: "Elektronik",
    hint: "Güçlü bas ve parlak üst",
    gains: [5, 4.5, 2, 0, -1.5, 0, 1, 2, 4, 4.5],
  },
  { name: "Rock", hint: "Vurucu davul ve gitar", gains: [4, 3.5, 2, 0, -1, -1, 1, 2.5, 3.5, 4] },
  { name: "Gece", hint: "Kısık seste dolgun ses", gains: [4.5, 4, 2, 0, -0.5, 0, 0, 1, 2.5, 3] },
  {
    name: "Konuşma",
    hint: "Podcast ve sesli kitap",
    gains: [-6, -4, -1.5, 1, 3, 4, 3, 1.5, 0, -2],
  },
];

/** Kazançlar bir hazır ayarla aynıysa onun adı, değilse `null` ("Özel"). */
export function matchPreset(gains: number[]): string | null {
  const found = EQ_PRESETS.find((preset) =>
    preset.gains.every((g, i) => Math.abs(g - (gains[i] ?? 0)) < 1e-6),
  );
  return found?.name ?? null;
}

/** Değeri sürgü aralığına ve adımına oturtur. */
export function snapGain(db: number): number {
  if (!Number.isFinite(db)) return 0;
  const clamped = Math.min(EQ_MAX_DB, Math.max(-EQ_MAX_DB, db));
  // -0 yerine 0 (ekranda "-0" görünmesin).
  return Math.round(clamped / EQ_STEP_DB) * EQ_STEP_DB + 0;
}

/** "+3,5", "−6", "0" gibi (Türkçe ondalık virgül, gerçek eksi işareti). */
export function formatGain(db: number, fractionDigits = 1): string {
  const rounded = Number(db.toFixed(fractionDigits));
  if (rounded === 0) return "0";
  const text = Math.abs(rounded).toLocaleString("tr-TR", {
    maximumFractionDigits: fractionDigits,
  });
  return `${rounded > 0 ? "+" : "−"}${text}`;
}

/** "31", "125", "1k", "16k". */
export function formatHz(hz: number): string {
  return hz >= 1000 ? `${Math.round(hz / 100) / 10}k`.replace(".", ",") : `${Math.floor(hz)}`;
}

/** Frekansın logaritmik eksendeki konumu (0..1). */
export function frequencyToRatio(hz: number): number {
  const ratio = Math.log(hz / CURVE_MIN_HZ) / Math.log(CURVE_MAX_HZ / CURVE_MIN_HZ);
  return Math.min(1, Math.max(0, ratio));
}

/**
 * Eğriyi SVG yoluna çevirir. Genişlik `width`, yükseklik `height`; dikey eksen
 * ±`rangeDb` (ortası 0 dB).
 */
export function curvePath(
  hz: number[],
  db: number[],
  width: number,
  height: number,
  rangeDb = EQ_MAX_DB + 3,
): string {
  const points = hz.map((f, i) => {
    const x = frequencyToRatio(f) * width;
    const clamped = Math.min(rangeDb, Math.max(-rangeDb, db[i] ?? 0));
    const y = height / 2 - (clamped / rangeDb) * (height / 2);
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });
  return points.length > 0 ? `M${points.join("L")}` : "";
}

/**
 * Tarayıcı önizlemesi için yaklaşık ekolayzer durumu: bant noktaları arasında
 * logaritmik eksende doğrusal eğri. (Gerçek eğriyi programda Rust hesaplar.)
 */
export function previewEqState(settings: EqSettings): EqState {
  const gains = EQ_BANDS_HZ.map((_, i) => snapGain(settings.gainsDb[i] ?? 0));
  const curveHz = Array.from(
    { length: 96 },
    (_, i) => CURVE_MIN_HZ * Math.pow(CURVE_MAX_HZ / CURVE_MIN_HZ, i / 95),
  );
  const effective = settings.enabled ? gains : gains.map(() => 0);
  const curveDb = curveHz.map((f) => {
    const position = Math.log2(f / EQ_BANDS_HZ[0]!);
    if (position <= 0) return effective[0]! * Math.max(0, 1 + position / 2);
    const last = EQ_BANDS_HZ.length - 1;
    if (position >= last) return effective[last]! * Math.max(0, 1 - (position - last) / 2);
    const i = Math.floor(position);
    const t = position - i;
    return effective[i]! * (1 - t) + effective[i + 1]! * t;
  });
  return {
    enabled: settings.enabled,
    gainsDb: gains,
    bandsHz: EQ_BANDS_HZ,
    maxGainDb: EQ_MAX_DB,
    preampDb: -Math.max(0, ...effective),
    curveHz,
    curveDb,
  };
}
