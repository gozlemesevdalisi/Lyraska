import { describe, expect, it } from "vitest";
import type { ParametricBand } from "./backend";
import {
  DEFAULT_PARAMETRIC,
  analogDb,
  dbToY,
  formatDb,
  formatFrequency,
  hzToX,
  nodeDb,
  parametricIsActive,
  parseDecimal,
  parseFrequency,
  plotRangeDb,
  previewParametricState,
  qToOctaves,
  roundGain,
  roundHz,
  roundQ,
  stepHz,
  stepQ,
  xToHz,
  yToDb,
} from "./parametric";

const band = (over: Partial<ParametricBand>): ParametricBand => ({
  kind: "peaking",
  freqHz: 1000,
  gainDb: 0,
  q: 1,
  enabled: true,
  ...over,
});

describe("parametrik ekolayzer yardımcıları", () => {
  it("logaritmik frekans ekseni ve dB ekseni birbirinin tersi", () => {
    for (const hz of [20, 63, 1000, 12345, 20000]) expect(xToHz(hzToX(hz))).toBeCloseTo(hz, 6);
    expect(hzToX(5)).toBe(0);
    expect(hzToX(30000)).toBe(1);
    for (const db of [-18, -3.5, 0, 6, 18]) expect(yToDb(dbToY(db))).toBeCloseTo(db, 9);
    expect(dbToY(0)).toBe(0.5);
    expect(dbToY(30)).toBe(0);
  });

  it("değerler anlamlı adımlara yuvarlanır ve sınırlara çekilir", () => {
    expect(roundHz(86.34)).toBe(86.3);
    expect(roundHz(1234.4)).toBe(1230);
    expect(roundHz(15051)).toBe(15100);
    expect(roundHz(3)).toBe(10);
    expect(roundHz(99999)).toBe(24000);
    expect(roundGain(30)).toBe(24);
    expect(roundGain(-3.14159)).toBe(-3.1);
    expect(roundQ(0.70711)).toBe(0.707);
    expect(roundQ(1.414)).toBe(1.41);
    expect(roundQ(0)).toBe(0.1);
    expect(roundQ(99)).toBe(30);
  });

  it("Q logaritmik adımlarla, frekans oktav kesirleriyle değişir", () => {
    expect(stepQ(1, 12)).toBe(2);
    expect(stepQ(2, -12)).toBe(1);
    expect(stepHz(1000, 1)).toBe(2000);
    expect(stepHz(1000, -1 / 12)).toBe(944);
    // Q = √2 bir oktav, Q = 2/3 tam iki oktav (asinh(3/4) = ln 2).
    expect(qToOctaves(Math.SQRT2)).toBeCloseTo(1, 9);
    expect(qToOctaves(2 / 3)).toBeCloseTo(2, 9);
  });

  it("Türkçe yazılmış sayıları okur ve yazar", () => {
    expect(parseFrequency("1,2k")).toBe(1200);
    expect(parseFrequency(" 86,3 Hz ")).toBe(86.3);
    expect(parseFrequency("12.5K")).toBe(12500);
    expect(parseFrequency("abc")).toBeNull();
    expect(parseFrequency("")).toBeNull();
    expect(parseFrequency("-5")).toBeNull();
    expect(parseDecimal("−3,5")).toBe(-3.5);
    expect(parseDecimal("+4,5 dB")).toBe(4.5);
    expect(parseDecimal("")).toBeNull();
    expect(formatFrequency(86.34)).toBe("86,3 Hz");
    expect(formatFrequency(1250)).toBe("1,25 kHz");
    expect(formatFrequency(12500)).toBe("12,5 kHz");
    expect(formatDb(3.25)).toBe("+3,3");
    expect(formatDb(-2)).toBe("−2");
    expect(formatDb(0.01)).toBe("0");
  });

  it("analog prototipler (çekirdekle aynı formüller) bilinen değerleri verir", () => {
    // Tepe merkezde kazancı, uzakta 0 dB.
    expect(analogDb(band({ gainDb: 6 }), 1000)).toBeCloseTo(6, 9);
    expect(analogDb(band({ gainDb: 6 }), 20)).toBeCloseTo(0, 2);
    // Raf orta frekansta kazancın yarısı, bölgesinde tamamı.
    const shelf = band({ kind: "lowShelf", freqHz: 100, gainDb: 8, q: Math.SQRT1_2 });
    expect(analogDb(shelf, 100)).toBeCloseTo(4, 9);
    expect(analogDb(shelf, 10)).toBeCloseTo(8, 1);
    expect(
      analogDb(band({ kind: "highShelf", freqHz: 5000, gainDb: -6, q: 0.7 }), 20000),
    ).toBeCloseTo(-6, 0);
    // Butterworth alçak/yüksek geçiren köşede −3,01 dB; bant geçiren merkezde 0 dB.
    expect(analogDb(band({ kind: "lowPass", q: Math.SQRT1_2 }), 1000)).toBeCloseTo(-3.0103, 3);
    expect(analogDb(band({ kind: "highPass", q: Math.SQRT1_2 }), 1000)).toBeCloseTo(-3.0103, 3);
    expect(analogDb(band({ kind: "bandPass", q: 2 }), 1000)).toBeCloseTo(0, 9);
    // Çentik merkezde derin, tüm geçiren her yerde 0 dB.
    expect(analogDb(band({ kind: "notch", q: 30 }), 1000)).toBeLessThan(-100);
    for (const hz of [20, 900, 1000, 15000])
      expect(analogDb(band({ kind: "allPass", q: 0.7 }), hz)).toBeCloseTo(0, 9);
    // Kazanç kullanmayan türde kazanç yok sayılır.
    expect(analogDb(band({ kind: "bandPass", q: 2, gainDb: 12 }), 1000)).toBeCloseTo(0, 9);
  });

  it("önizleme bant eğrilerinin toplamıdır; kapalı bant ve kapalı ekolayzer sese dokunmaz", () => {
    const settings = {
      ...DEFAULT_PARAMETRIC,
      bands: [
        band({ gainDb: 6 }),
        band({ freqHz: 100, gainDb: -3 }),
        band({ gainDb: 9, enabled: false }),
      ],
    };
    const state = previewParametricState(settings);
    for (let i = 0; i < state.curveHz.length; i++) {
      expect(state.curveDb[i]).toBeCloseTo(
        state.bandCurvesDb[0]![i]! + state.bandCurvesDb[1]![i]!,
        9,
      );
    }
    expect(state.boostDb).toBeGreaterThan(5.9);
    expect(state.boostDb).toBeLessThan(6.1);
    expect(parametricIsActive(settings)).toBe(true);
    const off = previewParametricState({ ...settings, enabled: false });
    expect(off.curveDb.every((db) => db === 0)).toBe(true);
    expect(parametricIsActive(DEFAULT_PARAMETRIC)).toBe(false);
    expect(previewParametricState(DEFAULT_PARAMETRIC, true).comparing).toBe(false);
  });

  it("grafiğin aralığı eğriyi kesmeyecek kadar genişler", () => {
    expect(plotRangeDb([[0, 3, -5]])).toBe(18);
    expect(plotRangeDb([[0, 20]])).toBe(24);
    expect(plotRangeDb([[-29]])).toBe(30);
    expect(plotRangeDb([[100]])).toBe(30);
  });

  it("kazancı olmayan türün düğümü eğrinin üstünde durur", () => {
    expect(nodeDb(band({ gainDb: 4 }))).toBe(4);
    expect(nodeDb(band({ kind: "lowPass", q: Math.SQRT1_2 }))).toBeCloseTo(-3.01, 2);
    expect(nodeDb(band({ kind: "notch", q: 30 }))).toBe(-18);
  });
});
