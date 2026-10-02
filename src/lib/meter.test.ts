import {
  PEAK_HOLD_SECONDS,
  RELEASE_PER_SECOND,
  emptyMeter,
  resampleBands,
  stepMeter,
} from "./meter";

describe("stepMeter", () => {
  it("yükselişe hemen tepki verir", () => {
    const next = stepMeter(emptyMeter(1), [0.8], 1 / 60);
    expect(next.levels[0]).toBe(0.8);
    expect(next.peaks[0]).toBe(0.8);
  });

  it("düşüşte yavaşça iner", () => {
    const high = stepMeter(emptyMeter(1), [1], 1 / 60);
    const next = stepMeter(high, [0], 0.1);
    expect(next.levels[0]).toBeCloseTo(1 - RELEASE_PER_SECOND * 0.1);
  });

  it("tepe noktası önce asılı kalır, sonra düşer", () => {
    let state = stepMeter(emptyMeter(1), [1], 1 / 60);
    const advance = (seconds: number) => {
      for (let t = 0; t < seconds - 1e-9; t += 0.05) state = stepMeter(state, [0], 0.05);
    };
    advance(PEAK_HOLD_SECONDS - 0.1);
    expect(state.peaks[0]).toBe(1);
    advance(0.3);
    expect(state.peaks[0]).toBeLessThan(1);
    expect(state.peaks[0]).toBeGreaterThan(state.levels[0]!);
  });

  it("aralık dışı hedefleri sınırlar ve büyük zaman sıçramalarını kırpar", () => {
    const next = stepMeter(emptyMeter(2), [5, -1], 1 / 60);
    expect(next.levels).toEqual([1, 0]);
    const afterSleep = stepMeter(next, [0, 0], 30);
    expect(afterSleep.levels[0]).toBeCloseTo(1 - RELEASE_PER_SECOND * 0.25);
  });
});

describe("resampleBands", () => {
  it("bant sayısını azaltırken grubun en yükseğini alır", () => {
    expect(resampleBands([0.1, 0.9, 0.2, 0.3], 2)).toEqual([0.9, 0.3]);
  });

  it("32 bandı 12'ye indirir, hiçbir giriş bandını atlamaz", () => {
    const bands = Array.from({ length: 32 }, (_, i) => i / 31);
    const out = resampleBands(bands, 12);
    expect(out).toHaveLength(12);
    expect(out[11]).toBe(1);
    expect(out.every((v, i) => i === 0 || v >= out[i - 1]!)).toBe(true);
  });

  it("boş girişte sıfır döndürür", () => {
    expect(resampleBands([], 3)).toEqual([0, 0, 0]);
    expect(resampleBands([1], 0)).toEqual([]);
  });
});
