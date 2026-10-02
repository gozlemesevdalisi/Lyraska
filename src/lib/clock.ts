/**
 * Oynatma saati: ekrandaki süre ve ilerleme çubuğunun akıcı ilerlemesi için.
 *
 * Ses motoru konumu saniyede birkaç kez bildirir. Arada ekran, son bildirilen
 * konumdan geçen süreyi ekleyerek her karede ileri sayar. Yeni bildirim küçük bir
 * sapma gösterirse konum zıplamaz, yumuşakça düzeltilir; büyük farklar (sarma,
 * şarkı değişimi) hemen uygulanır.
 */

export interface ClockAnchor {
  /** Bilinen konum (saniye). */
  position: number;
  /** Bu konumun bilindiği an (`performance.now()` ms). */
  at: number;
  /** Çalıyor mu? Çalmıyorsa konum sabit kalır. */
  running: boolean;
}

/** Bu kadar saniyeden büyük sapmalar düzeltilmez, doğrudan uygulanır. */
export const SNAP_SECONDS = 0.3;
/** Küçük sapmanın her bildirimde düzeltilen oranı. */
export const CORRECTION = 0.25;

/** Verilen anda tahmin edilen konum; [0, süre] aralığında. */
export function positionAt(anchor: ClockAnchor, now: number, duration: number | null): number {
  const elapsed = anchor.running ? Math.max(0, now - anchor.at) / 1000 : 0;
  const position = anchor.position + elapsed;
  return Math.max(0, duration && duration > 0 ? Math.min(position, duration) : position);
}

/** Ses motorundan yeni konum bildirimi geldiğinde saati günceller. */
export function reanchor(
  prev: ClockAnchor | null,
  reported: number,
  running: boolean,
  now: number,
  duration: number | null,
): ClockAnchor {
  if (prev && prev.running && running) {
    const predicted = positionAt(prev, now, duration);
    const drift = reported - predicted;
    if (Math.abs(drift) < SNAP_SECONDS) {
      return { position: predicted + drift * CORRECTION, at: now, running };
    }
  }
  return { position: reported, at: now, running };
}
