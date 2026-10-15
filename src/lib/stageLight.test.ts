import { describe, expect, it } from "vitest";
import { countFlashes } from "./flash";
import { DEFAULT_PEAKS, emptyMeter, stepMeter } from "./meter";
import { ENERGY_SLEW_PER_SECOND } from "./sky";
import {
  STAGE_LIGHT_AT_REST,
  STAGE_MAX_LUMINANCE,
  isStageLightAtRest,
  stageLightInput,
  stageLightStyle,
  stageLuminance,
  stagePeaks,
  stepStageLight,
  type StageLight,
  type StageLightInput,
} from "./stageLight";

const DT = 1 / 60;

function note(extra: Partial<StageLightInput> = {}): StageLightInput {
  return {
    mood: 0.6,
    accent: 0,
    anticipation: 0,
    release: 0,
    detail: 0.5,
    motion: 0.5,
    ...extra,
  };
}

/** Işığı `seconds` saniye boyunca aynı notla ilerletir. */
function settle(
  input: StageLightInput | null,
  seconds = 5,
  from: StageLight = STAGE_LIGHT_AT_REST,
): StageLight {
  let light = from;
  for (let i = 0; i < seconds * 60; i++) light = stepStageLight(light, input, DT);
  return light;
}

describe("sahne ışığı", () => {
  it("ruh hâlini izler: yoğun bölümde canlı, sakin bölümde kısık, şarkı yokken söner", () => {
    const calm = settle(note({ mood: 0.1 }));
    const loud = settle(note({ mood: 1 }));
    expect(loud.glow).toBeGreaterThan(calm.glow + 0.5);
    const off = settle(null, 5, loud);
    expect(off.glow).toBeLessThan(1e-3);
    expect(isStageLightAtRest(off)).toBe(true);
  });

  it("droptan önce hale daralıp kısılır, drop'ta genişleyip parlar", () => {
    const steady = settle(note({ mood: 0.8 }));
    const tense = settle(note({ mood: 0.8, anticipation: 1 }), 3, steady);
    const open = settle(note({ mood: 0.8, release: 1 }), 2, tense);
    const spread = (l: StageLight) => Number(stageLightStyle(l)["--stage-spread"]);
    expect(spread(tense)).toBeLessThan(spread(steady) - 0.2);
    expect(tense.glow).toBeLessThan(steady.glow);
    expect(spread(open)).toBeGreaterThan(spread(steady));
    expect(open.glow).toBeGreaterThan(steady.glow);
  });

  it("ölçü başında hale kabarır ama parlaklık değişmez", () => {
    const before = settle(note({ mood: 0.8 }));
    const hit = stepStageLight(before, note({ mood: 0.8, accent: 1 }), DT);
    expect(hit.swell).toBe(1);
    expect(Math.abs(hit.glow - before.glow)).toBeLessThan(1e-3);
    // Sönmeye bırakılınca kabarma kaybolur; aynı vurgu yeniden kabartmaz.
    const later = settle(note({ mood: 0.8, accent: 1 }), 2, hit);
    expect(later.swell).toBe(0);
    // "Animasyonları azalt": kabarma yok, hale süzülmez.
    const reduced = stepStageLight(before, note({ mood: 0.8, accent: 1 }), DT, true);
    expect(reduced.swell).toBe(0);
    expect(reduced.drift).toBe(before.drift);
  });

  it("doku: vuruş yoğunluğu halenin süzülmesini hızlandırır", () => {
    const still = settle(note({ motion: 0 }), 10);
    const busy = settle(note({ motion: 1 }), 10);
    const advance = (l: StageLight, input: StageLightInput) => {
      const next = settle(input, 1, l);
      return next.drift - l.drift;
    };
    expect(advance(busy, note({ motion: 1 }))).toBeGreaterThan(
      2 * advance(still, note({ motion: 0 })),
    );
    expect(busy.motion).toBeGreaterThan(0.95);
  });

  it("parlaklık her karede hız sınırı içinde değişir; güvenli modda yarı hızda", () => {
    for (const safe of [false, true]) {
      let light = STAGE_LIGHT_AT_REST;
      for (let i = 0; i < 600; i++) {
        const loud = Math.floor(i / 7) % 2 === 0;
        const next = stepStageLight(
          light,
          loud ? note({ mood: 1, release: 1 }) : null,
          DT,
          false,
          safe,
        );
        const limit = (safe ? 0.5 : 1) * ENERGY_SLEW_PER_SECOND * DT + 1e-12;
        expect(Math.abs(next.glow - light.glow)).toBeLessThanOrEqual(limit);
        expect(Math.abs(next.bloom - light.bloom)).toBeLessThanOrEqual(limit);
        light = next;
      }
    }
  });

  it("en kötü durumda bile saniyede 3'ten fazla parlamaz (epilepsi güvenliği)", () => {
    for (const safe of [false, true]) {
      for (const hz of [0.5, 1, 1.5, 2, 3, 4, 6, 8, 12, 20, 30]) {
        let light = STAGE_LIGHT_AT_REST;
        const luminance: number[] = [];
        for (let i = 0; i < 600; i++) {
          const on = Math.floor(i * DT * hz * 2) % 2 === 0;
          const input = on
            ? note({ mood: 1, release: 1, accent: i % 2 })
            : note({ mood: 0, anticipation: 1 });
          light = stepStageLight(light, input, DT, false, safe);
          luminance.push(stageLuminance(light));
          expect(stageLuminance(light)).toBeLessThanOrEqual(STAGE_MAX_LUMINANCE);
        }
        expect(countFlashes(luminance) / 10, `${hz} Hz, güvenli: ${safe}`).toBeLessThanOrEqual(2);
      }
    }
    // Ölçüm boş değil: tam parlaklığın parlama sayacına göre anlamı var.
    expect(STAGE_MAX_LUMINANCE).toBeGreaterThan(0.1);
  });

  it("spektrum: droptan önce tepe noktaları asılı kalır, tizler canlıyken çevik düşer", () => {
    const steady = settle(note({ detail: 0.5 }), 10);
    expect(stagePeaks(steady).holdSeconds).toBeCloseTo(DEFAULT_PEAKS.holdSeconds, 6);
    expect(stagePeaks(steady).fallPerSecond).toBeCloseTo(DEFAULT_PEAKS.fallPerSecond, 2);
    const tense = settle(note({ anticipation: 1 }), 3);
    // Tepe noktası tam ölçekte bırakılır: gerilimde daha uzun asılı kalır.
    const peakAfter = (seconds: number, light: StageLight) => {
      let meter = stepMeter(emptyMeter(1), [1], DT, stagePeaks(light));
      for (let i = 0; i < seconds * 60; i++) meter = stepMeter(meter, [0], DT, stagePeaks(light));
      return meter.peaks[0]!;
    };
    expect(peakAfter(1, steady)).toBeLessThan(0.8);
    expect(peakAfter(1, tense)).toBe(1);
    const lively = settle(note({ detail: 1 }), 10);
    const dull = settle(note({ detail: 0 }), 10);
    expect(stagePeaks(lively).fallPerSecond).toBeGreaterThan(1.5 * stagePeaks(dull).fallPerSecond);
  });

  it("uzun bir sekme aradan sonra sıçramaz", () => {
    const after = stepStageLight(STAGE_LIGHT_AT_REST, note({ mood: 1, release: 1 }), 30);
    expect(after.glow).toBeLessThanOrEqual(ENERGY_SLEW_PER_SECOND * 0.25 + 1e-12);
  });

  it("yönetmen notunun gerekli kısmını alır; not yoksa ışık dinlenir", () => {
    expect(stageLightInput(null)).toBeNull();
    expect(
      stageLightInput({
        atmosphere: { mood: 0.7 },
        rhythm: { accent: 0.2, anticipation: 0.3, release: 0.4 },
        texture: { detail: 0.5, motion: 0.6 },
      }),
    ).toEqual({
      mood: 0.7,
      accent: 0.2,
      anticipation: 0.3,
      release: 0.4,
      detail: 0.5,
      motion: 0.6,
    });
  });
});
