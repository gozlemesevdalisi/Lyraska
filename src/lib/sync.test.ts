import { describe, expect, it } from "vitest";
import type { VisualFrame } from "./backend";
import {
  advanceBeat,
  beatSwing,
  clampAudioDelay,
  estimateAudioDelay,
  extrapolateFrame,
} from "./sync";

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

  it("geç ulaşan veride vuruş konumu geçen süre kadar ileri alınır", () => {
    // 120 BPM: 10 ms = 0,02 vuruş.
    const moved = advanceBeat(4, 0.5, 120, 0.01);
    expect(moved.index).toBe(4);
    expect(moved.phase).toBeCloseTo(0.52, 10);
    // Vuruş sınırını geçince sıra da ilerler (nokta doğru kenara döner).
    const crossed = advanceBeat(4, 0.99, 120, 0.02);
    expect(crossed.index).toBe(5);
    expect(crossed.phase).toBeCloseTo(0.03, 10);
    expect(advanceBeat(4, 0.5, 0, 0.01)).toEqual({ index: 4, phase: 0.5 });
    expect(advanceBeat(4, 0.5, 120, -1)).toEqual({ index: 4, phase: 0.5 });
  });
});

describe("görsel verinin ileri alınması", () => {
  const frame: VisualFrame = {
    positionSecs: 10,
    bands: [0.5],
    rmsDb: [-20, -20],
    peakDb: [-6, -6],
    vuReferenceDb: -14,
    beat: { bpm: 120, index: 7, phase: 0.95, barBeat: 4, meter: 4 },
    energy: 0.6,
    section: 1,
    director: {
      atmosphere: { section: 1, theme: 1, mood: 0.6, warmth: 0.5 },
      rhythm: {
        pulse: 0.4,
        accent: 0,
        beatPhase: 0.95,
        barPhase: 0.9875,
        anticipation: 0,
        release: 0,
      },
      texture: { detail: 0.5, motion: 0.5 },
    },
  };

  it("vuruşa bağlı değerler geçen süre kadar ilerler, ölçü başına döner", () => {
    // 40 ms, 120 BPM'de 0,08 vuruş: 0,95 + 0,08 = 1,03 → 8. vuruş (yeni ölçünün başı).
    const moved = extrapolateFrame(frame, 0.04);
    expect(moved.positionSecs).toBeCloseTo(10.04, 10);
    expect(moved.beat).toMatchObject({ index: 8, barBeat: 1, meter: 4 });
    expect(moved.beat!.phase).toBeCloseTo(0.03, 10);
    expect(moved.director!.rhythm.beatPhase).toBeCloseTo(0.03, 10);
    // Ölçü fazı: 0,9875 + 0,08 / 4 = 1,0075 → 0,0075.
    expect(moved.director!.rhythm.barPhase).toBeCloseTo(0.0075, 10);
    // Seviyeler ve yumuşak değerler olduğu gibi.
    expect(moved.bands).toBe(frame.bands);
    expect(moved.energy).toBe(0.6);
    expect(moved.director!.rhythm.pulse).toBe(0.4);
    expect(frame.beat!.index).toBe(7); // asıl veri değişmez
    // Sınırı geçmeyen kısa süre: aynı vuruşta kalır.
    const small = extrapolateFrame(frame, 0.01);
    expect(small.beat).toMatchObject({ index: 7, barBeat: 4 });
    expect(small.beat!.phase).toBeCloseTo(0.97, 10);
  });

  it("süre yoksa ya da ritim yoksa yalnızca konum ilerler", () => {
    expect(extrapolateFrame(frame, 0)).toBe(frame);
    const noBeat = extrapolateFrame({ ...frame, beat: null }, 0.01);
    expect(noBeat.positionSecs).toBeCloseTo(10.01, 10);
    expect(noBeat.director!.rhythm.beatPhase).toBe(0.95);
    const noMeter = extrapolateFrame(
      { ...frame, beat: { ...frame.beat!, barBeat: null, meter: null } },
      0.02,
    );
    expect(noMeter.beat!.barBeat).toBeNull();
    expect(noMeter.director!.rhythm.barPhase).toBe(0.9875);
  });
});
