import type { VisualFrame } from "./backend";

/**
 * Ses–görüntü senkronu: ses gecikmesi ölçümünün hesabı, görsel verinin ekrana
 * ulaşana kadar geçen süre kadar ileri alınması ve vuruş göstergesi.
 *
 * Ölçüm: program tıklama kaydı çalar, kullanıcı her tıklamayı duyduğu anda
 * Boşluk'a basar. Her basışın en yakın tıklamaya göre ne kadar geç (ya da erken)
 * geldiği bulunur; ortanca değer, sesin aygıtta beklenenden ne kadar geç duyulduğudur.
 */

/** Rust tarafındaki `visual_bridge` sınırları (milisaniye). */
export const MIN_AUDIO_DELAY_MS = -100;
export const MAX_AUDIO_DELAY_MS = 400;
/** Güvenilir bir ölçüm için gereken en az basış. */
export const MIN_TAPS = 8;
/** Basışlar bundan daha dağınıksa (ortanca sapma, ms) ölçüm şüphelidir. */
export const MAX_RELIABLE_SPREAD_MS = 40;
/** Tıklamaya bundan uzak basış yok sayılır (saniye; tıklama aralığı 0,6 sn). */
const MAX_TAP_DISTANCE = 0.28;

export interface DelayEstimate {
  /** Önerilen ses gecikmesi (ms, sınırlar içinde). */
  delayMs: number;
  /** Hesaba katılan basış sayısı. */
  used: number;
  /** Basışların dağınıklığı (ortanca mutlak sapma, ms). */
  spreadMs: number;
  /** Dağınıklık kabul edilebilir mi? */
  reliable: boolean;
}

export function clampAudioDelay(ms: number): number {
  if (!Number.isFinite(ms)) return 0;
  return Math.max(MIN_AUDIO_DELAY_MS, Math.min(MAX_AUDIO_DELAY_MS, Math.round(ms)));
}

/** Basış ve tıklama zamanlarından (saniye) ses gecikmesini tahmin eder; yetersizse `null`. */
export function estimateAudioDelay(
  taps: readonly number[],
  clicks: readonly number[],
): DelayEstimate | null {
  if (clicks.length === 0) return null;
  const offsets = taps
    .map((tap) => {
      let best = Number.POSITIVE_INFINITY;
      for (const click of clicks) {
        if (Math.abs(tap - click) < Math.abs(best)) best = tap - click;
      }
      return best;
    })
    .filter((d) => Math.abs(d) <= MAX_TAP_DISTANCE);
  if (offsets.length < MIN_TAPS) return null;
  const center = median(offsets);
  const spreadMs = Math.round(median(offsets.map((d) => Math.abs(d - center))) * 1000);
  return {
    delayMs: clampAudioDelay(center * 1000),
    used: offsets.length,
    spreadMs,
    reliable: spreadMs <= MAX_RELIABLE_SPREAD_MS,
  };
}

/**
 * Vuruş göstergesindeki noktanın yeri (0 = sol, 1 = sağ): her vuruşta bir kenara
 * değer, vuruşlar arasında sabit hızla karşıya gider. Parlamaz, yalnızca hareket eder.
 */
export function beatSwing(index: number, phase: number): number {
  const p = Math.max(0, Math.min(1, Number.isFinite(phase) ? phase : 0));
  return index % 2 === 0 ? p : 1 - p;
}

/**
 * Vuruş konumunu `ageSeconds` kadar ileri alır (veri ekrana geç ulaştıysa).
 * Tempo bilinmiyorsa ya da süre geçersizse olduğu gibi döner.
 */
export function advanceBeat(
  index: number,
  phase: number,
  bpm: number,
  ageSeconds: number,
): { index: number; phase: number } {
  if (!(bpm > 0) || !(ageSeconds > 0) || !Number.isFinite(phase)) return { index, phase };
  const position = index + phase + (ageSeconds * bpm) / 60;
  const whole = Math.floor(position);
  return { index: whole, phase: position - whole };
}

/**
 * Görsel veriyi `ageSeconds` kadar ileri alır: verinin hesaplanmasından ekrana
 * çizilmesine kadar geçen süre telafi edilir. Vuruşa bağlı değerler (konum, vuruş,
 * ölçüdeki yer, Yönetmen'in vuruş ve ölçü fazı) ilerletilir; seviyeler ve yumuşak
 * değerler (bantlar, enerji, nabız) bir kareden kısa bir süre için olduğu gibi kalır.
 */
export function extrapolateFrame(frame: VisualFrame, ageSeconds: number): VisualFrame {
  if (!(ageSeconds > 0)) return frame;
  const beat = frame.beat;
  const beatsElapsed = beat && beat.bpm > 0 ? (ageSeconds * beat.bpm) / 60 : 0;
  let nextBeat = beat;
  if (beat && beatsElapsed > 0) {
    const moved = advanceBeat(beat.index, beat.phase, beat.bpm, ageSeconds);
    const meter = beat.meter ?? 0;
    const barBeat =
      beat.barBeat !== null && meter > 0
        ? ((((beat.barBeat - 1 + moved.index - beat.index) % meter) + meter) % meter) + 1
        : beat.barBeat;
    nextBeat = { ...beat, index: moved.index, phase: moved.phase, barBeat };
  }
  const director = frame.director;
  const meter = beat?.meter ?? 0;
  const nextDirector =
    director && beatsElapsed > 0
      ? {
          ...director,
          rhythm: {
            ...director.rhythm,
            beatPhase: fract(director.rhythm.beatPhase + beatsElapsed),
            barPhase:
              meter > 0
                ? fract(director.rhythm.barPhase + beatsElapsed / meter)
                : director.rhythm.barPhase,
          },
        }
      : director;
  return {
    ...frame,
    positionSecs: frame.positionSecs + ageSeconds,
    beat: nextBeat,
    director: nextDirector,
  };
}

function fract(x: number): number {
  return x - Math.floor(x);
}

function median(values: readonly number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid]! : (sorted[mid - 1]! + sorted[mid]!) / 2;
}
