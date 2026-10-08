import { describe, expect, it } from "vitest";
import { beatSwing, clampAudioDelay, estimateAudioDelay } from "./sync";

const clicks = Array.from({ length: 24 }, (_, i) => 2 + i * 0.6);

describe("ses gecikmesi ölçümü", () => {
  it("basışların tıklamalardan ortanca gecikmesini bulur", () => {
    // Bluetooth kulaklık: ses 190 ms geç; kullanıcı ±15 ms dağınık basıyor.
    const jitter = [0.01, -0.015, 0.005, 0, -0.01, 0.015, -0.005, 0.01, 0, -0.01, 0.005, -0.015];
    const taps = jitter.map((j, i) => clicks[i + 3]! + 0.19 + j);
    const estimate = estimateAudioDelay(taps, clicks)!;
    expect(estimate.delayMs).toBe(190);
    expect(estimate.used).toBe(12);
    expect(estimate.spreadMs).toBeLessThanOrEqual(15);
    expect(estimate.reliable).toBe(true);
  });

  it("erken basış ve kaçırılan tıklamalar da sayılır; uzak basış yok sayılır", () => {
    const taps = [
      ...clicks.slice(0, 10).map((c) => c - 0.03), // biraz önden
      clicks[12]! + 0.3, // iki tıklamanın ortası: yok sayılır
    ];
    const estimate = estimateAudioDelay(taps, clicks)!;
    expect(estimate.delayMs).toBe(-30);
    expect(estimate.used).toBe(10);
  });

  it("yetersiz ya da dağınık basışta güvenilmez", () => {
    expect(estimateAudioDelay(clicks.slice(0, 5), clicks)).toBeNull();
    expect(estimateAudioDelay([1, 2, 3], [])).toBeNull();
    const scattered = clicks.slice(0, 12).map((c, i) => c + (i % 2 ? 0.12 : -0.12));
    expect(estimateAudioDelay(scattered, clicks)!.reliable).toBe(false);
  });

  it("öneri sınırlar içinde kalır", () => {
    expect(clampAudioDelay(1200)).toBe(400);
    expect(clampAudioDelay(-500)).toBe(-100);
    expect(clampAudioDelay(Number.NaN)).toBe(0);
    expect(clampAudioDelay(123.6)).toBe(124);
  });
});

describe("vuruş göstergesi", () => {
  it("her vuruşta bir kenara değer, arada sabit hızla gider", () => {
    expect(beatSwing(0, 0)).toBe(0);
    expect(beatSwing(0, 0.5)).toBe(0.5);
    expect(beatSwing(0, 1)).toBe(1);
    expect(beatSwing(1, 0)).toBe(1);
    expect(beatSwing(1, 0.25)).toBe(0.75);
    expect(beatSwing(2, 0.1)).toBeCloseTo(0.1, 10);
    expect(beatSwing(3, Number.NaN)).toBe(1);
  });
});
