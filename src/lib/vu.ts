/**
 * VU ibreleri için saf hesaplar.
 *
 * Gerçek bir VU ölçer gibi: ibre gerilimle (doğrusal) orantılı sapar, ölçek
 * üzerindeki desibel işaretleri bu yüzden sağa doğru sıklaşır. İbre bir kütle-yay
 * sistemi gibi hareket eder: ani bir sese 300 ms'de %99 tepki verir ve ~%1,5
 * hafifçe aşar (standart VU davranışı).
 */

/** Ölçeğin sağ ucu: +3 VU. */
export const VU_MAX = 3;
/** Ölçek işaretleri (VU). */
export const SCALE_MARKS = [-20, -10, -7, -5, -3, -2, -1, 0, 1, 2, 3];
/** Analiz bitene kadar kullanılan referans (dBFS). */
export const DEFAULT_REFERENCE_DB = -16;
/** Bunun altı sessizlik (Rust tarafındaki `levels::FLOOR_DB`). */
export const SILENCE_DB = -60;
/** İbrenin sağdaki durdurucuya çarptığı konum (+3 VU'nun biraz ötesi). */
export const PEG_POSITION = 1.06;
/** İbre hareketi: doğal açısal frekans (rad/s) ve sönüm oranı. */
export const NEEDLE_OMEGA = 13.1;
export const NEEDLE_DAMPING = 0.8;
/** İbrenin yarım açısı (derece): konum 0 → −SWING, konum 1 → +SWING. */
export const SWING_DEGREES = 47;
/** Kırmızı bölge ışığının yandığı seviye ve en az yanık kalma süresi. */
export const LAMP_VU = 1;
export const LAMP_HOLD_SECONDS = 0.5;

/** VU değerinin ölçekteki konumu (0 = sol uç, 1 = +3 VU). */
export function vuToPosition(vu: number): number {
  return Math.pow(10, (vu - VU_MAX) / 20);
}

/** Ölçülen seviyenin (dBFS) ibre için hedef konumu. */
export function levelToPosition(rmsDb: number, referenceDb: number): number {
  if (!Number.isFinite(rmsDb) || rmsDb <= SILENCE_DB) return 0;
  return Math.min(PEG_POSITION, vuToPosition(rmsDb - referenceDb));
}

/** Konumun ibre açısı (derece, dikeye göre). */
export function positionToAngle(position: number): number {
  return -SWING_DEGREES + 2 * SWING_DEGREES * position;
}

export interface Needle {
  position: number;
  velocity: number;
}

export const NEEDLE_AT_REST: Needle = { position: 0, velocity: 0 };

/** Sayısal kararlılık için en büyük iç adım (saniye). */
const SUBSTEP = 0.002;

/** İbreyi `dt` saniye ilerletir. Uçlardaki durduruculara çarpınca durur. */
export function stepNeedle(needle: Needle, target: number, dt: number): Needle {
  let remaining = Math.max(0, Math.min(dt, 0.25)); // sekme/uyku sonrası büyük sıçramaları sınırla
  let { position, velocity } = needle;
  const w = NEEDLE_OMEGA;
  while (remaining > 1e-9) {
    const h = Math.min(SUBSTEP, remaining);
    const acceleration = w * w * (target - position) - 2 * NEEDLE_DAMPING * w * velocity;
    velocity += acceleration * h;
    position += velocity * h;
    if (position < 0) {
      position = 0;
      velocity = Math.max(0, velocity);
    } else if (position > PEG_POSITION) {
      position = PEG_POSITION;
      velocity = Math.min(0, velocity);
    }
    remaining -= h;
  }
  return { position, velocity };
}

/** İbre durgun mu (yeniden çizmeye gerek yok)? */
export function isAtRest(needle: Needle): boolean {
  return needle.position < 1e-4 && Math.abs(needle.velocity) < 1e-4;
}

/**
 * Kırmızı bölge ışığı. Bir kez yanınca en az `LAMP_HOLD_SECONDS` yanık kalır;
 * böylece saniyede en fazla 2 kez yanıp söner (epilepsi güvenliği: en fazla 3).
 * Dönen değer: kalan yanma süresi (0 = sönük).
 */
export function stepLamp(remaining: number, needle: Needle, dt: number): number {
  if (needle.position >= vuToPosition(LAMP_VU)) return LAMP_HOLD_SECONDS;
  return Math.max(0, remaining - Math.max(0, dt));
}
