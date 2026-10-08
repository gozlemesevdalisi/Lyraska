import { describe, expect, it } from "vitest";
import { dropCountdown, dropLabels, meterLabel, sectionTheme } from "./songMap";

describe("bölüm teması", () => {
  it("etiketi dört temaya dağıtır; geçersiz etiket ilk tema olur", () => {
    expect([0, 1, 2, 3, 4, 9].map(sectionTheme)).toEqual([0, 1, 2, 3, 0, 1]);
    expect(sectionTheme(-1)).toBe(0);
    expect(sectionTheme(Number.NaN)).toBe(0);
  });
});

describe("drop sayacı", () => {
  it("yalnızca sıradaki drop yakınsa görünür ve drop'a doğru dolar", () => {
    const drops = [40, 90, 150];
    expect(dropCountdown(drops, 0)).toBeNull(); // 40 sn var
    expect(dropCountdown(drops, 10)).toEqual({ seconds: 30, progress: 0 });
    expect(dropCountdown(drops, 25)).toEqual({ seconds: 15, progress: 0.5 });
    // Drop geçince sayaç sıradakine geçer, o da uzaksa gizlenir.
    expect(dropCountdown(drops, 41)).toBeNull();
    expect(dropCountdown(drops, 87)?.seconds).toBeCloseTo(3);
  });

  it("sırasız ya da geçersiz drop listesinde de en yakını bulur", () => {
    expect(dropCountdown([150, Number.NaN, 90], 80)?.seconds).toBe(10);
    expect(dropCountdown([], 80)).toBeNull();
    expect(dropCountdown([10], 80)).toBeNull(); // son drop geçti
  });
});

describe("şarkı haritası yazıları", () => {
  it("ölçüyü yazar", () => {
    expect(meterLabel(4)).toBe("4/4");
    expect(meterLabel(3)).toBe("3/4");
  });

  it("yakın drop'larda yalnızca ilkinin yazısı gösterilir", () => {
    expect(dropLabels([0.1, 0.12, 0.3, 0.33, 0.5], 0.06)).toEqual([true, false, true, false, true]);
    expect(dropLabels([], 0.06)).toEqual([]);
  });
});
