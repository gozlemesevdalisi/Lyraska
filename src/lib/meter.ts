/**
 * Seviye göstergesi (spektrum çubukları) için saf hesaplar.
 * Gerçek ölçerler gibi: yükselişte hemen tepki verir, düşüşte yavaşça iner;
 * tepe noktası kısa süre asılı kalır, sonra süzülerek düşer.
 */

/** Tam ölçekten sıfıra iniş süresi ≈ 1 / RELEASE_PER_SECOND saniye. */
export const RELEASE_PER_SECOND = 2.4;
/** Tepe noktasının asılı kaldığı süre (saniye). */
export const PEAK_HOLD_SECONDS = 0.6;
/** Tepe noktasının düşme hızı (tam ölçek / saniye). */
export const PEAK_FALL_PER_SECOND = 0.8;

export interface MeterState {
  levels: number[];
  peaks: number[];
  /** Her tepe noktasının kalan asılı kalma süresi (saniye). */
  holds: number[];
}

export function emptyMeter(bands: number): MeterState {
  return {
    levels: new Array<number>(bands).fill(0),
    peaks: new Array<number>(bands).fill(0),
    holds: new Array<number>(bands).fill(0),
  };
}

/** Ölçeri `dt` saniye ilerletir; `targets` o anki ölçülen seviyelerdir (0..1). */
export function stepMeter(prev: MeterState, targets: number[], dt: number): MeterState {
  const step = Math.max(0, Math.min(dt, 0.25)); // sekme/uyku sonrası büyük sıçramaları sınırla
  const next = emptyMeter(prev.levels.length);
  for (let i = 0; i < prev.levels.length; i++) {
    const target = clamp01(targets[i] ?? 0);
    const level = prev.levels[i] ?? 0;
    next.levels[i] = target >= level ? target : Math.max(target, level - RELEASE_PER_SECOND * step);

    const peak = prev.peaks[i] ?? 0;
    const hold = prev.holds[i] ?? 0;
    if (next.levels[i]! >= peak) {
      next.peaks[i] = next.levels[i]!;
      next.holds[i] = PEAK_HOLD_SECONDS;
    } else if (hold > 0) {
      next.peaks[i] = peak;
      next.holds[i] = Math.max(0, hold - step);
    } else {
      next.peaks[i] = Math.max(next.levels[i]!, peak - PEAK_FALL_PER_SECOND * step);
      next.holds[i] = 0;
    }
  }
  return next;
}

/**
 * Bant sayısını değiştirir (ör. 32 → 12). Her çıkış bandı, kapsadığı giriş
 * bantlarının en yükseğini alır; böylece dar bir notanın tepesi kaybolmaz.
 */
export function resampleBands(bands: number[], count: number): number[] {
  if (count <= 0) return [];
  if (bands.length === 0) return new Array<number>(count).fill(0);
  const out: number[] = [];
  for (let i = 0; i < count; i++) {
    const start = Math.floor((i * bands.length) / count);
    const end = Math.max(start + 1, Math.floor(((i + 1) * bands.length) / count));
    out.push(Math.max(...bands.slice(start, end)));
  }
  return out;
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}
