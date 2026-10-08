/**
 * Ses–görüntü senkronu: ses gecikmesi ölçümünün hesabı ve vuruş göstergesi.
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

function median(values: readonly number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid]! : (sorted[mid - 1]! + sorted[mid]!) / 2;
}
