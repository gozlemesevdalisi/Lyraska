/** Sahnelerin iç saatleri: ekran kartına giden değer hiç sıçramaz (saf hesap). */

/**
 * Sahne saatlerinin ekran kartına giden en büyük değeri. Gölgelendiricideki 32 bit kayan
 * noktada daha büyük değerlerde kare başına ilerleme hassasiyetini yitirir.
 */
export const SHADER_TIME_LIMIT = 10_000;

/** Sahnelerin iç saati bu değerde sarılır: katlanmış hâli ([`foldTime`]) burada da süreklidir. */
export const SCENE_CLOCK_PERIOD = 2 * SHADER_TIME_LIMIT;

/**
 * Sürekli ilerleyen sahne saatini ekran kartına gidecek değere çevirir: 0 → 10 000 → 0 üçgen
 * dalga. Saat sıfıra sarılsaydı bütün görüntü bir karede başka yere sıçrardı (ani tam ekran
 * değişim); katlanınca görüntü sürekli kalır, yalnızca akışın yönü bir an tersine döner.
 */
export function foldTime(clock: number): number {
  if (!Number.isFinite(clock)) return 0;
  const t = ((clock % SCENE_CLOCK_PERIOD) + SCENE_CLOCK_PERIOD) % SCENE_CLOCK_PERIOD;
  return t <= SHADER_TIME_LIMIT ? t : SCENE_CLOCK_PERIOD - t;
}
