import { describe, expect, it } from "vitest";
import {
  EQ_BANDS_HZ,
  EQ_PRESETS,
  curvePath,
  formatGain,
  formatHz,
  frequencyToRatio,
  matchPreset,
  previewEqState,
  snapGain,
} from "./eq";

describe("ekolayzer yardımcıları", () => {
  it("kazancı aralığa ve 0,5 dB adımına oturtur", () => {
    expect(snapGain(3.26)).toBe(3.5);
    expect(snapGain(3.24)).toBe(3);
    expect(snapGain(40)).toBe(12);
    expect(snapGain(-40)).toBe(-12);
    expect(snapGain(Number.NaN)).toBe(0);
    expect(Object.is(snapGain(-0.1), 0)).toBe(true);
  });

  it("kazancı Türkçe biçimde yazar", () => {
    expect(formatGain(3.5)).toBe("+3,5");
    expect(formatGain(-6)).toBe("−6");
    expect(formatGain(0)).toBe("0");
    expect(formatGain(-0.04)).toBe("0");
    expect(formatGain(12, 0)).toBe("+12");
  });

  it("frekansı kısa yazar", () => {
    expect(EQ_BANDS_HZ.map(formatHz)).toEqual([
      "31",
      "62",
      "125",
      "250",
      "500",
      "1k",
      "2k",
      "4k",
      "8k",
      "16k",
    ]);
  });

  it("hazır ayarı tanır, değiştirilince 'özel' sayar", () => {
    const rock = EQ_PRESETS.find((p) => p.name === "Rock")!;
    expect(matchPreset(rock.gains)).toBe("Rock");
    expect(matchPreset(Array(10).fill(0))).toBe("Düz");
    expect(matchPreset(rock.gains.map((g, i) => (i === 0 ? g + 0.5 : g)))).toBeNull();
  });

  it("hazır ayarlar 10 bant, ±8 dB içinde ve adları farklı", () => {
    // Bas ayarları +8 dB'ye kadar çıkar: eşitleme açıkken yükseltme sesi kısmadan yapılır
    // ve bas gerçekten hissedilir. Daha fazlası kulağı yorar.
    for (const preset of EQ_PRESETS) {
      expect(preset.gains).toHaveLength(10);
      expect(preset.gains.every((g) => Math.abs(g) <= 8 && snapGain(g) === g)).toBe(true);
    }
    expect(new Set(EQ_PRESETS.map((p) => p.name)).size).toBe(EQ_PRESETS.length);
  });

  it("bas ayarları cihaza göre farklı frekansları yükseltir", () => {
    const gains = (name: string) => EQ_PRESETS.find((p) => p.name === name)!.gains;
    const [sub31, bass62, bass125, low250] = gains("Bas");
    // "Bas": göğse vuran bölge (62–125 Hz) en yüksek.
    expect(Math.max(bass62!, bass125!)).toBeGreaterThan(sub31!);
    expect(bass62!).toBeGreaterThanOrEqual(6);
    // "Derin bas": en çok 31 Hz.
    expect(gains("Derin bas")[0]).toBe(Math.max(...gains("Derin bas")));
    // "Küçük hoparlör": veremediği 31 Hz kısılır, 125–250 Hz yükseltilir.
    const small = gains("Küçük hoparlör");
    expect(small[0]!).toBeLessThan(0);
    expect(Math.min(small[2]!, small[3]!)).toBeGreaterThan(3);
    expect(low250).toBeGreaterThan(0);
  });

  it("frekansları logaritmik eksene yerleştirir", () => {
    expect(frequencyToRatio(20)).toBe(0);
    expect(frequencyToRatio(20000)).toBe(1);
    expect(frequencyToRatio(Math.sqrt(20 * 20000))).toBeCloseTo(0.5);
    expect(frequencyToRatio(5)).toBe(0);
  });

  it("eğriyi SVG yoluna çevirir (0 dB ortada)", () => {
    expect(curvePath([20, 20000], [0, 15], 100, 200, 15)).toBe("M0.0,100.0L100.0,0.0");
    expect(curvePath([], [], 100, 200)).toBe("");
  });

  it("tarayıcı önizlemesinde yaklaşık eğri üretir", () => {
    const gains = [6, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    const state = previewEqState({ enabled: true, gainsDb: gains });
    expect(state.preampDb).toBe(-6);
    const at31 = state.curveDb[state.curveHz.findIndex((f) => f >= 31.25)]!;
    expect(at31).toBeGreaterThan(5);
    expect(state.curveDb[state.curveDb.length - 1]).toBe(0);
    const off = previewEqState({ enabled: false, gainsDb: gains });
    expect(off.curveDb.every((d) => d === 0)).toBe(true);
    expect(off.gainsDb[0]).toBe(6);
  });
});
