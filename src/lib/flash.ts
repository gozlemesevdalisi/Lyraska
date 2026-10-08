/**
 * Parlama sayacı (epilepsi güvenliği). WCAG 2.3.1'e göre parlama: bağıl
 * parlaklıkta en az %10'luk, birbirine zıt iki değişim. Sahnelerin parlaklık
 * eğrileri bununla denetlenir: hiçbir sahne saniyede 3'ten fazla parlamaz.
 */
export const FLASH_DELTA = 0.1;

/** Parlaklık eğrisindeki (0..1 bağıl parlaklık) parlama sayısı. */
export function countFlashes(luminance: readonly number[]): number {
  // Yükselirken en yüksek, düşerken en düşük değer izlenir; ondan %10 dönüş bir değişimdir.
  let min = luminance[0] ?? 0;
  let max = min;
  let direction = 0;
  let changes = 0;
  for (const l of luminance) {
    min = direction > 0 ? min : Math.min(min, l);
    max = direction < 0 ? max : Math.max(max, l);
    if (direction >= 0 && max - l >= FLASH_DELTA) {
      direction = -1;
      changes++;
      min = l;
    } else if (direction <= 0 && l - min >= FLASH_DELTA) {
      direction = 1;
      changes++;
      max = l;
    }
  }
  return Math.floor(changes / 2);
}
