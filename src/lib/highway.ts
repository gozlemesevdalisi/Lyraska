/**
 * "Gece otoyolu" sahnesi için saf hesaplar. Gece, ufukta şehir ışıkları olan
 * boş bir otoyolda gidiyoruz:
 *
 * - Yolun orta şerit çizgileri **vuruşlara kilitlidir**: her vuruşta bir çizgi
 *   geçer (drop açılımında iki). Sokak lambaları **ölçü başlarına kilitlidir**:
 *   her ölçü başında bir lamba yanımızdan geçer. Kilit yumuşaktır (faz kilitli
 *   döngü): şarkı sarıldığında ya da analiz sonradan geldiğinde yol sıçramaz,
 *   birkaç karede yakalar ve hiçbir zaman geri gitmez.
 * - Ufuk parıltısı müziğin enerjisiyle güçlenir; rengi bölüm temasından gelir
 *   (gece göğüyle aynı temalar). Droptan önce parıltı ufka çekilip daralır
 *   (gerilim), drop'ta yükselip genişler (açılım).
 *
 * Epilepsi güvenliği: ekranın parlaklığını değiştiren tek değer (parıltı ve
 * açılım) saniyede en fazla `ENERGY_SLEW_PER_SECOND` kadar değişir (güvenli
 * modda yarısı). Vuruşlar parlaklığa değil, yolun akışına yansır.
 */

import {
  BLOOM_LUMINANCE,
  ENERGY_SLEW_PER_SECOND,
  clamp01,
  slew,
  smooth,
  stepPalette,
  type Energies,
  type Palette,
} from "./sky";

export interface HighwayState {
  /** Ufuk parıltısı (parlaklık; hız sınırlı). */
  glow: number;
  /** Yolda alınan yol, şerit çizgisi aralığı biriminde (her vuruşta 1). */
  dash: number;
  /** Yolda alınan yol, lamba aralığı biriminde (her ölçüde 1). */
  lamp: number;
  /** Droptan önceki gerilim (0..1; yavaş). */
  tension: number;
  /** Drop açılımı (0..1; parlaklığı etkiler, hız sınırlı). */
  bloom: number;
  palette: Palette;
  /** Sahne saati (saniye; yolun kıvrılması ve şehir pencereleri için). */
  time: number;
}

export const HIGHWAY_AT_REST: HighwayState = {
  glow: 0,
  dash: 0,
  lamp: 0,
  tension: 0,
  bloom: 0,
  palette: { from: 0, to: 0, mix: 1 },
  time: 0,
};

/** O anın müziği: bant enerjileri, vuruş ve Görsel Yönetmen'in notu. */
export interface HighwayInput {
  energies: Energies;
  /** Tempo ve son vuruştan bu yana geçen kesir; ritim yoksa `null`. */
  beat: { bpm: number; phase: number } | null;
  /** Yönetmen notu; analiz bitmediyse `null`. */
  director: {
    mood: number;
    theme: number;
    barPhase: number;
    anticipation: number;
    release: number;
  } | null;
}

/** Bir lamba aralığında kaç şerit çizgisi var (4/4 ölçüde vuruş sayısı). */
export const DASHES_PER_LAMP = 4;
/** Ritim yokken yolun hızı (çizgi / saniye). */
const IDLE_DASH_RATE = 1.2;
/** Faz kilidinin yakalama süresi (saniye). */
const LOCK_SECONDS = 0.2;
/** Kilitlenirken bile yol bu orandan yavaş akmaz (geri gitmez). */
const MIN_ADVANCE = 0.3;
/**
 * Drop açılımı bu değerin üstündeyken çizgiler (ve lambalar) iki kat hızla geçer.
 * En hızlı tempoda (240 BPM) bile lambalar saniyede en fazla 2 geçer; güvenli modda 1.
 */
const DOUBLE_TIME_BLOOM = 0.5;
/** Lambaların geçiş sıklığı için üst sınır (lamba / saniye), yakalarken bile. */
const MAX_LAMP_RATE = 2;
const SAFE_LAMP_RATE = 1;
/** "Animasyonları azalt" açıkken hareket bu oranda yavaşlar (ve vuruşa kilitlenmez). */
const REDUCED_MOTION_SPEED = 0.25;
/** Gerilimin değişim hızı (tam ölçek / saniye). */
const TENSION_SLEW_PER_SECOND = 0.8;
/** Açılımın parıltıya katkısı. */
const BLOOM_GLOW = 0.35;

/**
 * Parıltının ekrana katabileceği bağıl parlaklık için güvenli üst sınır (parlama
 * testinde kullanılır). Chromium'da ölçülen değişim bunun çok altında kalır.
 */
export const HIGHWAY_MAX_LUMINANCE = 0.35;
/** Parıltı sıfırken bile ufuk tamamen kararmaz. */
export const GLOW_BASE = 0.3;

/**
 * Yolu `dt` saniye ilerletir. `input` o an duyulan anın verisidir; çalmıyorken
 * `null` (parıltı dinlenmeye iner, yol yavaşça akmaya devam eder).
 */
export function stepHighway(
  prev: HighwayState,
  input: HighwayInput | null,
  dt: number,
  reducedMotion = false,
  safe = false,
): HighwayState {
  const step = Math.max(0, Math.min(dt, 0.25)); // sekme/uyku sonrası sıçramayı sınırla
  const slewRate = (safe ? 0.5 : 1) * ENERGY_SLEW_PER_SECOND * step;
  const director = input?.director ?? null;

  // Parıltı: bas ağırlıklı enerji; sakin bölümde sönük, yoğun bölümde canlı.
  const [bass = 0, mid = 0, high = 0] = input?.energies ?? [0, 0, 0];
  const level = clamp01(0.5 * clamp01(bass) + 0.35 * clamp01(mid) + 0.15 * clamp01(high));
  const goal = input
    ? clamp01(
        director
          ? level * (0.55 + 0.45 * clamp01(director.mood)) + BLOOM_GLOW * clamp01(director.release)
          : level,
      )
    : 0;
  const glow = slew(prev.glow, smooth(prev.glow, goal, step), slewRate);
  const tension = slew(
    prev.tension,
    clamp01(director?.anticipation ?? 0),
    TENSION_SLEW_PER_SECOND * step,
  );
  const bloom = slew(prev.bloom, clamp01(director?.release ?? 0), slewRate);

  // Yol: vuruşa (çizgiler) ve ölçüye (lambalar) kilitli akış.
  const beat = input?.beat && Number.isFinite(input.beat.bpm) ? input.beat : null;
  // Drop açılımında çizgiler iki kat hızla geçer (güvenli modda geçmez).
  const multiple = bloom > DOUBLE_TIME_BLOOM && !safe ? 2 : 1;
  const dashRate = beat ? (clampBpm(beat.bpm) / 60) * multiple : IDLE_DASH_RATE;
  const motion = reducedMotion ? REDUCED_MOTION_SPEED : 1;
  const locked = !reducedMotion;
  const dash = advance(
    prev.dash,
    dashRate * motion * step,
    locked && beat ? beat.phase * multiple : null,
    step,
  );
  const lamp = advance(
    prev.lamp,
    (dashRate / DASHES_PER_LAMP) * motion * step,
    locked && beat && director ? director.barPhase * multiple : null,
    step,
    (safe ? SAFE_LAMP_RATE : MAX_LAMP_RATE) * step,
  );

  return {
    glow,
    dash,
    lamp,
    tension,
    bloom,
    palette: stepPalette(prev.palette, director?.theme ?? null, step),
    // "Animasyonları azalt" açıkken pencereler yanıp sönmez, yol kıvrılmaz.
    time: reducedMotion ? prev.time : (prev.time + step) % 10_000,
  };
}

/**
 * Faz kilitli ilerleme: `distance` her adımda `nominal` kadar ilerler; hedef faz
 * varsa (0..1 kesir, birim başına) farkın bir kısmı kapatılır. Hiçbir zaman geri gitmez.
 */
function advance(
  distance: number,
  nominal: number,
  phase: number | null,
  dt: number,
  maxDelta = Number.POSITIVE_INFINITY,
): number {
  let delta = nominal;
  if (phase !== null && Number.isFinite(phase)) {
    const error = wrapHalf(fract(phase) - fract(distance + nominal));
    delta = Math.max(nominal * MIN_ADVANCE, nominal + error * (1 - Math.exp(-dt / LOCK_SECONDS)));
  }
  delta = Math.min(delta, maxDelta);
  // Kayan noktada hassasiyet kaybolmasın: büyük değerler tam sayı adımlarla sarılır.
  const next = distance + delta;
  return next > 10_000 ? next - Math.floor(next / 1000) * 1000 : next;
}

/** Ufuk parıltısının ekrana kattığı göreli parlaklık (gölgelendiricideki formülün üst sınırı). */
export function highwayLuminance(glow: number, bloom = 0): number {
  const strength = GLOW_BASE + (1 - GLOW_BASE) * clamp01(glow);
  return (
    (HIGHWAY_MAX_LUMINANCE * strength * (1 + BLOOM_LUMINANCE * clamp01(bloom))) /
    (1 + BLOOM_LUMINANCE)
  );
}

function clampBpm(bpm: number): number {
  return Math.max(40, Math.min(240, bpm));
}

function fract(x: number): number {
  return x - Math.floor(x);
}

/** Faz farkını −0,5..0,5 aralığına getirir (en kısa yoldan). */
function wrapHalf(x: number): number {
  return x - Math.round(x);
}
