import { describe, expect, it } from "vitest";
import {
  confidentDrops,
  dropCountdown,
  dropLabels,
  confidentMeterLabel,
  meterLabel,
  sectionTheme,
  themeAt,
} from "./songMap";

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

describe("drop güveni", () => {
  it("sayaç yalnızca güveni yüksek droplar için", () => {
    const map = { drops: [40, 90, 150], dropConfidence: [0.95, 0.2, 0.6] };
    expect(confidentDrops(map)).toEqual([40, 150]);
    // 90. saniyedeki şüpheli drop için sayaç çıkmaz; sıradaki güvenli drop uzak.
    expect(dropCountdown(confidentDrops(map), 70)).toBeNull();
    expect(dropCountdown(confidentDrops(map), 125)?.seconds).toBe(25);
  });

  it("güveni bilinmeyen drop sayılmaz", () => {
    expect(confidentDrops({ drops: [40, 90], dropConfidence: [0.9] })).toEqual([40]);
    expect(confidentDrops({ drops: [40] })).toEqual([]);
  });
});

describe("şarkı haritası yazıları", () => {
  it("ölçüyü yazar", () => {
    expect(meterLabel(4)).toBe("4/4");
    expect(meterLabel(3)).toBe("3/4");
    // Aksak: sekizlik gruplamasıyla.
    expect(meterLabel(4, [2, 2, 2, 3])).toBe("9/8 (2+2+2+3)");
    expect(meterLabel(3, [2, 2, 3])).toBe("7/8 (2+2+3)");
    // Ölçü belirsizse hiç gösterilmez.
    expect(confidentMeterLabel({ meter: 4, grouping: [], meterConfidence: 0.9 })).toBe("4/4");
    expect(confidentMeterLabel({ meter: 4, grouping: [], meterConfidence: 0.2 })).toBeNull();
  });

  it("yakın drop'larda yalnızca ilkinin yazısı gösterilir", () => {
    expect(dropLabels([0.1, 0.12, 0.3, 0.33, 0.5], 0.06)).toEqual([true, false, true, false, true]);
    expect(dropLabels([], 0.06)).toEqual([]);
  });
});

describe("çalan bölümün teması", () => {
  const sections = [
    { start: 0, end: 30, label: 0 },
    { start: 30, end: 90, label: 1 },
    { start: 90, end: 120, label: 6 },
  ];

  it("konumun bölümünün etiketinden gelir; bölüm sınırında yenisi başlar", () => {
    expect(themeAt(sections, 10)).toBe(0);
    expect(themeAt(sections, 30)).toBe(1);
    expect(themeAt(sections, 89.9)).toBe(1);
    expect(themeAt(sections, 100)).toBe(2); // etiket 6 → tema 2
  });

  it("harita yoksa ya da konum bölümlerin dışındaysa varsayılan tema", () => {
    expect(themeAt([], 10)).toBe(0);
    expect(themeAt(sections, 120)).toBe(0);
    expect(themeAt(sections, Number.NaN)).toBe(0);
  });
});
