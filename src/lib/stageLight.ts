import { PEAK_FALL_PER_SECOND, PEAK_HOLD_SECONDS, type PeakBallistics } from "./meter";
import { SCENE_CLOCK_PERIOD } from "./sceneClock";
import { ENERGY_SLEW_PER_SECOND, clamp01, slew, smooth, smoothTexture } from "./sky";

/**
 * "Sahne ışığı": VU ibreleri ve spektrum sahnelerinin Görsel Yönetmen'e bağlanan arka ışığı
 * (saf hesaplar). Ölçerlerin kendisi (ibreler, çubuklar) müziği olduğu gibi gösterir; ışık
 * şarkının haritasını izler:
 *
 * - **Atmosfer:** ışığın gücü bölümün ruh hâlinden gelir (sakin bölümde kısık, yoğun bölümde
 *   canlı). Rengi bölüm temasıdır: arayüzün vurgu rengi (`--aurora`) zaten temaya yumuşakça
 *   geçer, sahneler onu kullanır.
 * - **Ritim:** droptan önce ışık toplanır, hale daralır (gerilim); drop'ta açılır (açılım). Ölçü
 *   başlarında hale hafifçe kabarır: yalnızca boyut değişir, parlaklık değil.
 * - **Doku:** halenin süzülme hızı vuruş yoğunluğundan (hareket), spektrumda tepe noktalarının
 *   düşüş hızı tizlerin canlılığından (ayrıntı) gelir.
 *
 * Epilepsi güvenliği: ekranın parlaklığını değiştiren tek değer (`glow`) saniyede en fazla
 * `ENERGY_SLEW_PER_SECOND` kadar değişir (güvenli modda yarısı). Kabarma ve süzülme parlaklığa
 * dokunmaz; "animasyonları azalt" açıkken ikisi de durur.
 */

/** Sahne ışığının Görsel Yönetmen'den kullandığı değerler (`DirectorFrame`'in alt kümesi). */
export interface StageLightInput {
  mood: number;
  accent: number;
  anticipation: number;
  release: number;
  detail: number;
  motion: number;
}

export interface StageLight {
  /** Işığın gücü (0..1; parlaklık, hız sınırlı). */
  glow: number;
  /** Droptan önceki gerilim (0..1; yavaş). */
  tension: number;
  /** Drop açılımı (0..1; hız sınırlı). */
  bloom: number;
  /** Ölçü başı kabarması (0..1; yalnızca halenin boyutu). */
  swell: number;
  /** Doku: tizlerin canlılığı ve vuruş yoğunluğu (0..1; yumuşak). */
  detail: number;
  motion: number;
  /** Halenin süzülme saati (saniye; hareketle hızlanır). */
  drift: number;
  /** Son karede okunan ölçü vurgusu (yeni vurguyu ayırt etmek için). */
  lastAccent: number;
}

/** Şarkı yokken ya da analiz bitmeden: kısık, durgun ışık. */
export const STAGE_LIGHT_AT_REST: StageLight = {
  glow: 0,
  tension: 0,
  bloom: 0,
  swell: 0,
  detail: 0.5,
  motion: 0.5,
  drift: 0,
  lastAccent: 0,
};

/** Işık gücünün ruh hâlinden ve açılımdan payı; gerilimde kısılma oranı. */
const MOOD_GLOW = 0.75;
const BLOOM_GLOW = 0.25;
const TENSION_DIM = 0.35;
/** Gerilimin değişim hızı (tam ölçek / saniye). */
const TENSION_SLEW_PER_SECOND = 0.8;
/** Kabarmanın sönme süresi (saniye) ve yeni vurgu sayılan artış. */
const SWELL_SECONDS = 0.35;
const FRESH_ACCENT = 0.3;
/** Halenin süzülme hızı: hareket 0'da en yavaş, 1'de en hızlı (saat / saniye). */
const DRIFT_MIN = 0.4;
const DRIFT_MAX = 1.4;

/**
 * Işığın ekrana katabileceği bağıl parlaklık için güvenli üst sınır (parlama testinde
 * kullanılır): VU'da ölçer yüzünün ve arkasındaki halenin, spektrumda noktaların ışıması.
 * CSS'teki formüller bu sınırın altında kalır.
 */
export const STAGE_MAX_LUMINANCE = 0.2;

/**
 * Işığı `dt` saniye ilerletir. `input` o an duyulan anın yönetmen notudur; çalmıyorken ya da
 * analiz bitmediyse `null` (ışık dinlenmeye iner).
 */
export function stepStageLight(
  prev: StageLight,
  input: StageLightInput | null,
  dt: number,
  reducedMotion = false,
  safe = false,
): StageLight {
  const step = Math.max(0, Math.min(dt, 0.25)); // sekme/uyku sonrası sıçramayı sınırla
  const slewRate = (safe ? 0.5 : 1) * ENERGY_SLEW_PER_SECOND * step;

  const tension = slew(
    prev.tension,
    clamp01(input?.anticipation ?? 0),
    TENSION_SLEW_PER_SECOND * step,
  );
  const bloom = slew(prev.bloom, clamp01(input?.release ?? 0), slewRate);
  const goal = input
    ? clamp01((MOOD_GLOW * clamp01(input.mood) + BLOOM_GLOW * bloom) * (1 - TENSION_DIM * tension))
    : 0;
  const glow = slew(prev.glow, smooth(prev.glow, goal, step), slewRate);

  // Ölçü başı: yeni vurgu gelince hale kabarır ve söner (güvenli modda vurgu gelmez).
  const accent = input?.accent ?? 0;
  const fresh = !reducedMotion && accent > prev.lastAccent + FRESH_ACCENT;
  const swell = fresh ? 1 : prev.swell * Math.exp(-step / SWELL_SECONDS);

  const detail = smoothTexture(prev.detail, input?.detail, step);
  const motion = smoothTexture(prev.motion, input?.motion, step);
  const driftRate = reducedMotion ? 0 : DRIFT_MIN + (DRIFT_MAX - DRIFT_MIN) * motion;

  return {
    glow,
    tension,
    bloom,
    swell: swell < 0.01 ? 0 : swell,
    detail,
    motion,
    drift: (prev.drift + driftRate * step) % SCENE_CLOCK_PERIOD,
    lastAccent: accent,
  };
}

/**
 * Spektrumun tepe noktaları: droptan önce daha uzun asılı kalır (gerilim), tizler canlıyken
 * daha çevik düşer (ayrıntı). Çubukların kendisi müziği olduğu gibi gösterir.
 */
export function stagePeaks(light: StageLight): PeakBallistics {
  return {
    holdSeconds: PEAK_HOLD_SECONDS * (1 + 1.5 * clamp01(light.tension)),
    fallPerSecond: PEAK_FALL_PER_SECOND * (0.7 + 0.6 * clamp01(light.detail)),
  };
}

/** Işığın ekrana kattığı bağıl parlaklık (CSS'teki formüllerin üst sınırı). */
export function stageLuminance(light: StageLight): number {
  return STAGE_MAX_LUMINANCE * clamp01(light.glow);
}

/** Işık durgun mu (yeniden çizmeye gerek yok)? */
export function isStageLightAtRest(light: StageLight): boolean {
  return light.glow < 1e-3 && light.bloom < 1e-3 && light.tension < 1e-3 && light.swell === 0;
}

/**
 * Sahnenin CSS değişkenleri. `--stage-glow` parlaklığı, diğerleri yalnızca boyutu ve konumu
 * değiştirir: halenin genişliği (gerilimde daralır, açılımda genişler, ölçü başında kabarır)
 * ve süzülmesi.
 */
export function stageLightStyle(light: StageLight): Record<string, string> {
  const spread = 1 - 0.3 * light.tension + 0.25 * light.bloom + 0.06 * light.swell;
  const sway = Math.sin(light.drift * 0.35) * 3 + Math.sin(light.drift * 0.13 + 1.1) * 2;
  return {
    "--stage-glow": clamp01(light.glow).toFixed(3),
    "--stage-spread": spread.toFixed(3),
    "--stage-sway": `${sway.toFixed(2)}%`,
    "--stage-tension": clamp01(light.tension).toFixed(3),
  };
}

/** Görsel verinin sahne ışığının kullandığı kısmı. */
export function stageLightInput(
  director: {
    atmosphere: { mood: number };
    rhythm: { accent: number; anticipation: number; release: number };
    texture: { detail: number; motion: number };
  } | null,
): StageLightInput | null {
  if (!director) return null;
  return {
    mood: director.atmosphere.mood,
    accent: director.rhythm.accent,
    anticipation: director.rhythm.anticipation,
    release: director.rhythm.release,
    detail: director.texture.detail,
    motion: director.texture.motion,
  };
}
