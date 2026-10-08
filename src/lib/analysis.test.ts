import { describe, expect, it } from "vitest";
import { analysisPending, analysisRefreshKey, bpmLabel } from "./analysis";

describe("BPM sütunu", () => {
  it("tempoyu yuvarlar; ritimsiz şarkıda çizgi, analiz edilmemişte boş", () => {
    expect(bpmLabel({ bpm: 127.6, analyzed: true })).toBe("128");
    expect(bpmLabel({ bpm: null, analyzed: true })).toBe("—");
    expect(bpmLabel({ bpm: null, analyzed: false })).toBe("");
  });
});

describe("analiz ilerlemesi", () => {
  it("liste kütüphanenin ~%2'si analiz edildikçe ve sonda tazelenir", () => {
    expect(analysisRefreshKey(0, 1000)).toBe(0);
    expect(analysisRefreshKey(19, 1000)).toBe(0);
    expect(analysisRefreshKey(20, 1000)).toBe(1);
    expect(analysisRefreshKey(1000, 1000)).toBe(-1);
    // Küçük kütüphanede her şarkıda.
    expect(analysisRefreshKey(3, 10)).toBe(3);
    expect(analysisRefreshKey(0, 0)).toBe(-1);
  });

  it("bekleyen şarkı varken sürüyor sayılır", () => {
    expect(analysisPending(3, 10)).toBe(true);
    expect(analysisPending(10, 10)).toBe(false);
    expect(analysisPending(0, 0)).toBe(false);
  });
});
