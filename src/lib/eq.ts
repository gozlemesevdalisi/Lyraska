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
  /** Bas düğmesi (dB, 0–12). */
  bassDb: number;
  /** Küçük hoparlör bası (alt bas yerine harmonikler). */
  smallSpeaker: boolean;
}

/** Bas düğmesinin üst sınırı (dB); Rust tarafındaki `MAX_BASS_DB` ile aynı. */
export const BASS_MAX_DB = 12;

/** Yalnızca bantları ayarlayan hazır ayar (bas düğmesi 0). */
function bands(name: string, hint: string, gains: number[]): EqPreset {
  return { name, hint, gains, bassDb: 0, smallSpeaker: false };
}

/**
 * Hazır ayarlar. Değerler özgündür. Sürgüler gerçekten duyulan eğridir ve ses yüksekliği
 * eşitlemesi açıkken yükseltme sesi kısmadan yapılır: "Bas" bası gerçekten yükseltir.
 * Bas ayarları bas düğmesini (100 Hz raf) ve bantları birlikte kullanır; cihaza göre üç tane:
 * - Bas: alt bas ve 60–125 Hz (davul ve bas gitarın göğse vurduğu yer); her cihazda.
 * - Derin bas: 30–60 Hz alt bas; iyi kulaklık ya da subwoofer ister.
 * - Küçük hoparlör: dizüstü hoparlörü 100 Hz'in altını veremez; orayı yükseltmek yalnızca
 *   bozulma yaratır. Küçük hoparlör bası alt bası süzüp harmoniklerini ekler (beyin eksik
 *   notayı tamamlar); hoparlörün verebildiği 125–250 Hz de yükseltilir.
 */
export const EQ_PRESETS: EqPreset[] = [
  bands("Düz", "Ses olduğu gibi", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
  {
    name: "Bas",
    hint: "Davul ve bas göğse vurur (kulaklık ve hoparlör)",
    gains: [0, 4, 3, 1, 0, 0, 0, 0, 0, 0],
    bassDb: 6,
    smallSpeaker: false,
  },
  {
    name: "Derin bas",
    hint: "Alt bas, gümbürtü (iyi kulaklık ya da subwoofer)",
    gains: [2, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    bassDb: 9,
    smallSpeaker: false,
  },
  {
    name: "Küçük hoparlör",
    hint: "Dizüstü ve küçük hoparlörde bas hissedilir (çalınamayan alt bas yerine harmonikleri)",
    gains: [0, 0, 3, 3, 1.5, 0, 0, 1, 1.5, 0],
    bassDb: 8,
    smallSpeaker: true,
  },
  bands("Tiz", "Parlak, net tizler", [0, 0, 0, 0, 0, 0.5, 2, 4, 5.5, 6]),
  bands("Vokal", "Sesler önde", [-2, -2, -1, 0.5, 2, 3.5, 3.5, 2, 0, -1]),
  bands("Akustik", "Sıcak ve doğal", [3, 3, 2, 1, 1, 1, 2, 2.5, 3, 2]),
  {
    name: "Elektronik",
    hint: "Güçlü bas ve parlak üst",
    gains: [3, 3, 2, 0, -1.5, 0, 1, 2, 4, 4.5],
    bassDb: 4,
    smallSpeaker: false,
  },
  bands("Rock", "Vurucu davul ve gitar", [4, 4.5, 2.5, 0, -1, -1, 1, 2.5, 3.5, 4]),
  bands("Gece", "Kısık seste dolgun ses", [4.5, 4, 2, 0, -0.5, 0, 0, 1, 2.5, 3]),
  bands("Konuşma", "Podcast ve sesli kitap", [-6, -4, -1.5, 1, 3, 4, 3, 1.5, 0, -2]),
];

/** Ayar (bantlar, bas düğmesi, küçük hoparlör) bir hazır ayarla aynıysa onun adı, değilse `null`. */
export function matchPreset(
  settings: Pick<EqSettings, "gainsDb" | "bassDb" | "smallSpeaker">,
): string | null {
  const found = EQ_PRESETS.find(
    (preset) =>
      preset.gains.every((g, i) => Math.abs(g - (settings.gainsDb[i] ?? 0)) < 1e-6) &&
      Math.abs(preset.bassDb - settings.bassDb) < 1e-6 &&
      preset.smallSpeaker === settings.smallSpeaker,
  );
  return found?.name ?? null;
}

/** Bas düğmesinin değerini aralığa (0–12) ve 0,5 dB adımına oturtur. */
export function snapBass(db: number): number {
  if (!Number.isFinite(db)) return 0;
  return Math.round(Math.min(BASS_MAX_DB, Math.max(0, db)) / EQ_STEP_DB) * EQ_STEP_DB + 0;
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

/** Bas düğmesinin yaklaşık eğrisi (dB): 100 Hz raf; küçük hoparlörde 80 Hz altı süzülür. */
function previewBassDb(bassDb: number, small: boolean, hz: number): number {
  const shelf = bassDb / (1 + (hz / 100) ** 2);
  const cut = small ? 20 * Math.log10((hz / 80) ** 4 / (1 + (hz / 80) ** 4)) : 0;
  return shelf + cut;
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
  const bass = settings.enabled ? snapBass(settings.bassDb) : 0;
  const small = settings.enabled && settings.smallSpeaker;
  const curveDb = curveHz.map((f) => {
    const position = Math.log2(f / EQ_BANDS_HZ[0]!);
    let db: number;
    if (position <= 0) db = effective[0]! * Math.max(0, 1 + position / 2);
    else if (position >= EQ_BANDS_HZ.length - 1) {
      const last = EQ_BANDS_HZ.length - 1;
      db = effective[last]! * Math.max(0, 1 - (position - last) / 2);
    } else {
      const i = Math.floor(position);
      const t = position - i;
      db = effective[i]! * (1 - t) + effective[i + 1]! * t;
    }
    return db + previewBassDb(bass, small, f);
  });
  // Rust'taki gibi: küçük hoparlörde alt bas süzüldüğü için rafın tamamı değil, süzgeçlerin
  // en büyük artışı ve harmonikler için 3 dB.
  const bassBoost = small
    ? Math.max(
        0,
        ...curveHz.filter((f) => f >= 30 && f <= 300).map((f) => previewBassDb(bass, true, f)),
      ) + 3
    : bass;
  return {
    enabled: settings.enabled,
    gainsDb: gains,
    bassDb: snapBass(settings.bassDb),
    smallSpeaker: settings.smallSpeaker,
    maxBassDb: BASS_MAX_DB,
    bandsHz: EQ_BANDS_HZ,
    maxGainDb: EQ_MAX_DB,
    // Rust'taki gibi: bantların en büyük yükseltmesi + bas motorunun yükseltmesi.
    preampDb: -(Math.max(0, ...effective) + bassBoost) + 0,
    curveHz,
    curveDb,
  };
}
