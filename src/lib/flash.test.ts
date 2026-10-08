import { describe, expect, it } from "vitest";
import { countFlashes } from "./flash";

describe("parlama sayacı", () => {
  it("%10'luk zıt değişim çiftlerini sayar", () => {
    expect(countFlashes([0, 0.2, 0, 0.2, 0])).toBe(2);
    expect(countFlashes([0, 0.05, 0, 0.05, 0])).toBe(0); // eşiğin altında
    expect(countFlashes([0, 0.1, 0.2, 0.3])).toBe(0); // tek yönlü değişim parlama değil
    expect(countFlashes([])).toBe(0);
  });

  it("yavaş da olsa gidip gelmeyi sayar", () => {
    const wave = Array.from({ length: 400 }, (_, i) => 0.5 + 0.2 * Math.sin(i / 20));
    expect(countFlashes(wave)).toBeGreaterThanOrEqual(2);
  });
});
