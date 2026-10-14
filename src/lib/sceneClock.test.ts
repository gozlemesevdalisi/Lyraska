import { describe, expect, it } from "vitest";
import { SCENE_CLOCK_PERIOD, SHADER_TIME_LIMIT, foldTime } from "./sceneClock";

describe("sahne saati", () => {
  it("0 → 10 000 → 0 diye katlanır, hiç sıçramaz", () => {
    expect(foldTime(0)).toBe(0);
    expect(foldTime(1234.5)).toBe(1234.5);
    expect(foldTime(SHADER_TIME_LIMIT)).toBe(SHADER_TIME_LIMIT);
    expect(foldTime(SHADER_TIME_LIMIT + 1)).toBe(SHADER_TIME_LIMIT - 1);
    // İç saat 20 000'de sarılınca da katlanmış değer sürekli (0'a döner).
    expect(foldTime(SCENE_CLOCK_PERIOD - 0.01)).toBeCloseTo(0.01, 9);
    expect(foldTime(SCENE_CLOCK_PERIOD)).toBe(0);
    for (let t = 0; t < 3 * SCENE_CLOCK_PERIOD; t += 7.3) {
      expect(Math.abs(foldTime(t + 0.01) - foldTime(t))).toBeLessThanOrEqual(0.0100001);
    }
  });

  it("geçersiz saat 0 sayılır", () => {
    expect(foldTime(Number.NaN)).toBe(0);
    expect(foldTime(Number.POSITIVE_INFINITY)).toBe(0);
    expect(foldTime(-5)).toBe(5);
  });
});
