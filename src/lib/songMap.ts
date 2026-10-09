/** Çalan şarkının haritasından ekranda gösterilenler (saf hesaplar). */

/** Sahnelerin renk teması sayısı (`global.css`: `--sky-theme-0` … `--sky-theme-3`). */
export const THEME_COUNT = 4;

/**
 * Bölümün renk teması. Benzer bölümler (ör. her nakarat) aynı etiketi alır; gece göğü
 * de aynı temayla boyanır. Böylece şeritteki renk, o bölümde gökyüzünün alacağı renktir.
 */
export function sectionTheme(label: number): number {
  return Number.isFinite(label) && label >= 0 ? Math.floor(label) % THEME_COUNT : 0;
}

/**
 * Çalan anın bölümünün renk teması (arayüzün vurgu renkleri buna uyar). Harita yoksa ya da
 * konum bir bölümde değilse 0 (varsayılan tema).
 */
export function themeAt(
  sections: readonly { start: number; end: number; label: number }[],
  positionSecs: number,
): number {
  const section = sections.find((s) => positionSecs >= s.start && positionSecs < s.end);
  return section ? sectionTheme(section.label) : 0;
}

/** Drop sayacı bu kadar saniye kala görünür. */
export const DROP_COUNTDOWN_SECS = 30;

export interface DropCountdown {
  /** Drop'a kalan süre (saniye). */
  seconds: number;
  /** Sayacın dolma oranı: 0 sayaç yeni göründü … 1 drop anı. */
  progress: number;
}

/** Sıradaki drop yakınsa (≤ `windowSecs`) kalan süre; değilse `null`. */
export function dropCountdown(
  drops: number[],
  positionSecs: number,
  windowSecs = DROP_COUNTDOWN_SECS,
): DropCountdown | null {
  let next = Infinity;
  for (const drop of drops) {
    if (Number.isFinite(drop) && drop > positionSecs && drop < next) next = drop;
  }
  const seconds = next - positionSecs;
  if (!Number.isFinite(seconds) || seconds > windowSecs) return null;
  return { seconds, progress: 1 - seconds / windowSecs };
}

/** Ölçü: "4/4", "3/4". */
export function meterLabel(meter: number): string {
  return `${meter}/4`;
}

/**
 * Şeritte hangi drop'ların yanına "DROP" yazılacağı: birbirine `minGap`'ten (şerit
 * genişliğine oranla) yakın olanlarda yalnızca ilki yazılır, yazılar üst üste binmez.
 * `xs` soldan sağa sıralı olmalıdır.
 */
export function dropLabels(xs: number[], minGap: number): boolean[] {
  let last = -Infinity;
  return xs.map((x) => {
    if (x - last < minGap) return false;
    last = x;
    return true;
  });
}
