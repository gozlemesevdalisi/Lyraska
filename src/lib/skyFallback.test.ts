import { describe, expect, it } from "vitest";
import { LYRA_STARS } from "./sky";
import { auroraCenter, lyraScreenStars } from "./skyFallback";

describe("gece göğünün yedek çizimi", () => {
  it("Lyra sağ üstte, gölgelendiricideki yerinde; Vega en parlak", () => {
    const stars = lyraScreenStars(926, 273);
    expect(stars).toHaveLength(LYRA_STARS.length);
    for (const s of stars) {
      expect(s.x).toBeGreaterThan(926 * 0.6); // sağda
      expect(s.y).toBeLessThan(273 * 0.5); // üstte
    }
    const vega = stars[0]!;
    // Vega: ekran yüksekliğinin %14'ü kadar aşağıda (gölgelendiricide y = 0,86).
    expect(vega.y).toBeCloseTo(273 * 0.14, 3);
    expect(vega.x).toBeCloseTo(0.84 * 926, 3);
    expect(Math.max(...stars.map((s) => s.brightness))).toBe(vega.brightness);
  });

  it("perdeler gerilimde ufka iner, açılımda yükselir", () => {
    const calm = auroraCenter(1.2, 0, 3, 0, 0);
    expect(auroraCenter(1.2, 0, 3, 1, 0)).toBeCloseTo(calm - 0.12, 10);
    expect(auroraCenter(1.2, 0, 3, 0, 1)).toBeCloseTo(calm + 0.05, 10);
    // Perde akar: zamanla yeri değişir ama ekranın içinde kalır.
    expect(auroraCenter(1.2, 0, 30, 0, 0)).not.toBeCloseTo(calm, 3);
    for (let t = 0; t < 100; t += 7) {
      const y = auroraCenter(t * 0.3, 2, t, 0, 0);
      expect(y).toBeGreaterThan(0.3);
      expect(y).toBeLessThan(0.9);
    }
  });
});
