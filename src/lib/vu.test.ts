import { describe, expect, it } from "vitest";
import {
  LAMP_HOLD_SECONDS,
  NEEDLE_AT_REST,
  PEG_POSITION,
  SWING_DEGREES,
  isAtRest,
  levelToPosition,
  positionToAngle,
  stepLamp,
  stepNeedle,
  vuToPosition,
  type Needle,
} from "./vu";

/** İbreyi sabit bir hedefe doğru 60 fps adımlarla sürer; her karedeki konumu döndürür. */
function run(target: number, seconds: number, start: Needle = NEEDLE_AT_REST): number[] {
  const positions: number[] = [];
  let needle = start;
  for (let t = 0; t < seconds; t += 1 / 60) {
    needle = stepNeedle(needle, target, 1 / 60);
    positions.push(needle.position);
  }
  return positions;
}

describe("VU ölçeği", () => {
  it("0 VU ölçeğin ~%71'inde, +3 VU sağ uçta, −20 VU solda", () => {
    expect(vuToPosition(3)).toBeCloseTo(1);
    expect(vuToPosition(0)).toBeCloseTo(0.708, 3);
    expect(vuToPosition(-20)).toBeCloseTo(0.0708, 3);
  });

  it("seviyeyi şarkının referansına göre konuma çevirir", () => {
    expect(levelToPosition(-14, -14)).toBeCloseTo(vuToPosition(0));
    expect(levelToPosition(-60, -14)).toBe(0);
    expect(levelToPosition(Number.NEGATIVE_INFINITY, -14)).toBe(0);
    // Çok yüksek seviye ibreyi durdurucuya dayar, ötesine geçirmez.
    expect(levelToPosition(0, -30)).toBe(PEG_POSITION);
  });

  it("konumu açıya çevirir", () => {
    expect(positionToAngle(0)).toBe(-SWING_DEGREES);
    expect(positionToAngle(0.5)).toBeCloseTo(0);
    expect(positionToAngle(1)).toBe(SWING_DEGREES);
  });
});

describe("VU ibresi", () => {
  it("ani sese 300 ms civarında %99 tepki verir ve ~%1,5 aşar", () => {
    const target = vuToPosition(0);
    const positions = run(target, 1.5);
    const reach = positions.findIndex((p) => p >= 0.99 * target) / 60;
    expect(reach).toBeGreaterThan(0.26);
    expect(reach).toBeLessThan(0.34);
    const overshoot = Math.max(...positions) / target - 1;
    expect(overshoot).toBeGreaterThan(0.005);
    expect(overshoot).toBeLessThan(0.025);
    expect(positions[positions.length - 1]).toBeCloseTo(target, 3);
  });

  it("kare hızından bağımsız davranır", () => {
    const target = vuToPosition(-3);
    let slow: Needle = NEEDLE_AT_REST;
    let fast: Needle = NEEDLE_AT_REST;
    for (let i = 0; i < 30; i++) slow = stepNeedle(slow, target, 1 / 30);
    for (let i = 0; i < 144; i++) fast = stepNeedle(fast, target, 1 / 144);
    expect(slow.position).toBeCloseTo(fast.position, 2);
  });

  it("ses kesilince dinlenme konumuna döner; uçlarda durur", () => {
    const loud = run(PEG_POSITION, 1);
    expect(Math.max(...loud)).toBeLessThanOrEqual(PEG_POSITION);
    let needle: Needle = { position: 0.8, velocity: 0 };
    for (let i = 0; i < 120; i++) needle = stepNeedle(needle, 0, 1 / 60);
    expect(needle.position).toBeGreaterThanOrEqual(0);
    expect(isAtRest(needle)).toBe(true);
  });

  it("uzun bir duraklamadan sonra ibre fırlamaz", () => {
    const needle = stepNeedle(NEEDLE_AT_REST, vuToPosition(0), 5);
    // En fazla 0,25 sn ilerletilir.
    expect(needle.position).toBeLessThanOrEqual(vuToPosition(0) * 1.02);
    expect(Number.isFinite(needle.velocity)).toBe(true);
  });
});

describe("kırmızı bölge ışığı", () => {
  const hot: Needle = { position: vuToPosition(2), velocity: 0 };
  const calm: Needle = { position: vuToPosition(-5), velocity: 0 };

  it("kırmızı bölgede yanar, en az yarım saniye yanık kalır", () => {
    expect(stepLamp(0, hot, 1 / 60)).toBe(LAMP_HOLD_SECONDS);
    expect(stepLamp(LAMP_HOLD_SECONDS, calm, 0.2)).toBeCloseTo(0.3);
    expect(stepLamp(0.1, calm, 0.2)).toBe(0);
  });

  it("saniyede 3'ten fazla yanıp sönmez (epilepsi güvenliği)", () => {
    // En kötü durum: ışık söner sönmez ibre yeniden kırmızı bölgeye giriyor.
    let lamp = 0;
    let wasOn = false;
    let flashes = 0;
    for (let frame = 0; frame < 600; frame++) {
      lamp = stepLamp(lamp, lamp > 0 ? calm : hot, 1 / 60);
      const on = lamp > 0;
      if (on && !wasOn) flashes++;
      wasOn = on;
    }
    expect(flashes / 10).toBeLessThanOrEqual(3);
    expect(flashes).toBeGreaterThan(5); // senaryo gerçekten yakıp söndürüyor
  });
});
