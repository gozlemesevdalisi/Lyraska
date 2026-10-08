import { describe, expect, it } from "vitest";
import { countFlashes } from "./flash";
import {
  GLOW_BASE,
  HIGHWAY_AT_REST,
  HIGHWAY_MAX_LUMINANCE,
  highwayLuminance,
  stepHighway,
  type HighwayInput,
  type HighwayState,
} from "./highway";
import { ENERGY_SLEW_PER_SECOND } from "./sky";

const DT = 1 / 60;

/** 120 BPM, 4/4: `t` saniyedeki vuruş ve ölçü fazı. */
function music(
  t: number,
  extra: Partial<NonNullable<HighwayInput["director"]>> = {},
): HighwayInput {
  const beats = t * 2;
  return {
    energies: [0.6, 0.5, 0.4],
    beat: { bpm: 120, phase: beats - Math.floor(beats) },
    director: {
      mood: 1,
      theme: 0,
      barPhase: beats / 4 - Math.floor(beats / 4),
      anticipation: 0,
      release: 0,
      ...extra,
    },
  };
}

function drive(
  state: HighwayState,
  seconds: number,
  input: (t: number) => HighwayInput | null,
  reduced = false,
  start = 0,
) {
  let s = state;
  for (let i = 1; i <= Math.round(seconds / DT); i++) {
    s = stepHighway(s, input(start + i * DT), DT, reduced);
  }
  return s;
}

const fract = (x: number) => x - Math.floor(x);
const phaseDistance = (a: number, b: number) => Math.abs(fract(a - b + 0.5) - 0.5);

describe("gece otoyolu", () => {
  it("şerit çizgileri vuruşa, lambalar ölçü başına kilitlenir", () => {
    // Yol yanlış fazda başlar; birkaç saniyede vuruşu yakalar.
    const start = { ...HIGHWAY_AT_REST, dash: 0.37, lamp: 0.81 };
    const s = drive(start, 3, (t) => music(t));
    const t = 3;
    expect(phaseDistance(s.dash, fract(t * 2))).toBeLessThan(0.02);
    expect(phaseDistance(s.lamp, fract((t * 2) / 4))).toBeLessThan(0.02);
    // Kilitliyken her vuruşta bir çizgi geçer.
    const later = drive(s, 1, (u) => music(u), false, t);
    expect(later.dash - s.dash).toBeCloseTo(2, 1);
    expect(later.lamp - s.lamp).toBeCloseTo(0.5, 1);
  });

  it("yakalarken bile yol geri gitmez", () => {
    let s = { ...HIGHWAY_AT_REST, dash: 0.49 };
    for (let i = 1; i <= 120; i++) {
      const next = stepHighway(s, music(i * DT), DT);
      expect(next.dash).toBeGreaterThan(s.dash);
      expect(next.lamp).toBeGreaterThan(s.lamp);
      s = next;
    }
  });

  it("drop açılımında çizgiler iki kat hızla geçer", () => {
    const calm = drive(HIGHWAY_AT_REST, 3, (t) => music(t));
    const drop = drive(HIGHWAY_AT_REST, 3, (t) => music(t, { release: 1 }));
    const a = drive(calm, 1, (t) => music(t), false, 3);
    const b = drive(drop, 1, (t) => music(t, { release: 1 }), false, 3);
    expect(b.dash - drop.dash).toBeCloseTo(2 * (a.dash - calm.dash), 1);
    // İki kat hızda da vuruşla aynı fazda (her yarım vuruşta bir çizgi).
    expect(phaseDistance(b.dash, fract(4 * 2 * 2))).toBeLessThan(0.02);
  });

  it("lambalar en hızlı tempoda bile saniyede en fazla 2 geçer; güvenli modda 1", () => {
    for (const safe of [false, true]) {
      let s = HIGHWAY_AT_REST;
      for (let i = 1; i <= 600; i++) {
        const t = i * DT;
        const beats = t * 10; // 600 BPM istense de tempo sınırlanır
        s = stepHighway(
          s,
          {
            energies: [1, 1, 1],
            beat: { bpm: 600, phase: beats % 1 },
            director: { mood: 1, theme: 0, barPhase: (beats / 4) % 1, anticipation: 0, release: 1 },
          },
          DT,
          false,
          safe,
        );
      }
      expect(s.lamp / 10, `güvenli: ${safe}`).toBeLessThanOrEqual(safe ? 1.05 : 2.05);
    }
  });

  it("ritim yokken ve çalmıyorken yol yavaşça akar", () => {
    const idle = drive(HIGHWAY_AT_REST, 1, () => null);
    expect(idle.dash).toBeCloseTo(1.2, 5);
    expect(idle.glow).toBe(0);
    const noBeat = drive(HIGHWAY_AT_REST, 1, () => ({
      energies: [1, 1, 1],
      beat: null,
      director: null,
    }));
    expect(noBeat.dash).toBeCloseTo(1.2, 5);
    expect(noBeat.glow).toBeGreaterThan(0.9);
  });

  it("animasyonları azalt açıkken yavaşlar, vuruşa kilitlenmez, pencereler durur", () => {
    const s = drive(HIGHWAY_AT_REST, 2, (t) => music(t), true);
    expect(s.dash).toBeCloseTo(2 * 2 * 0.25, 5);
    expect(s.time).toBe(0);
  });

  it("gerilim ve açılım Yönetmen'den gelir; tema yumuşak geçer", () => {
    const tense = drive(HIGHWAY_AT_REST, 2.5, (t) => music(t, { anticipation: 1, theme: 3 }));
    expect(tense.tension).toBeCloseTo(1, 5);
    expect(tense.palette).toMatchObject({ from: 0, to: 3, mix: 1 });
    const sakin = drive(HIGHWAY_AT_REST, 3, (t) => music(t, { mood: 0 }));
    const yogun = drive(HIGHWAY_AT_REST, 3, (t) => music(t));
    expect(yogun.glow).toBeGreaterThan(sakin.glow * 1.5);
  });

  it("parlaklık sınırları doğru", () => {
    expect(highwayLuminance(1, 1)).toBeCloseTo(HIGHWAY_MAX_LUMINANCE, 10);
    expect(highwayLuminance(0)).toBeLessThan(HIGHWAY_MAX_LUMINANCE * GLOW_BASE);
    expect(highwayLuminance(5, 5)).toBeCloseTo(HIGHWAY_MAX_LUMINANCE, 10);
  });

  it("parıltı saniyede ENERGY_SLEW_PER_SECOND'dan hızlı değişmez; güvenli modda yarısı", () => {
    for (const safe of [false, true]) {
      let s = HIGHWAY_AT_REST;
      for (let i = 0; i < 600; i++) {
        const loud = i % 2 === 0;
        const next = stepHighway(
          s,
          loud
            ? music(i * DT, { release: 1 })
            : { energies: [0, 0, 0], beat: null, director: null },
          DT,
          false,
          safe,
        );
        const limit = (safe ? 0.5 : 1) * ENERGY_SLEW_PER_SECOND * DT + 1e-12;
        expect(Math.abs(next.glow - s.glow)).toBeLessThanOrEqual(limit);
        expect(Math.abs(next.bloom - s.bloom)).toBeLessThanOrEqual(limit);
        s = next;
      }
    }
  });

  it("en kötü durumda bile saniyede 3'ten fazla parlamaz (epilepsi güvenliği)", () => {
    for (const safe of [false, true]) {
      for (const hz of [0.5, 1, 1.5, 2, 3, 4, 6, 8, 12, 20, 30]) {
        let s = HIGHWAY_AT_REST;
        const luminance: number[] = [];
        for (let i = 0; i < 600; i++) {
          const on = Math.floor(i * DT * hz * 2) % 2 === 0;
          const input: HighwayInput = on
            ? { ...music(i * DT, { release: 1, theme: i }), energies: [1, 1, 1] }
            : { ...music(i * DT, { mood: 0, anticipation: 1 }), energies: [0, 0, 0] };
          s = stepHighway(s, input, DT, false, safe);
          luminance.push(highwayLuminance(s.glow, s.bloom));
        }
        expect(countFlashes(luminance) / 10, `${hz} Hz, güvenli: ${safe}`).toBeLessThanOrEqual(2);
      }
    }
    // Senaryo gerçekten parlatıp söndürebiliyor: yavaş değişimde parlamalar sayılır.
    let s = HIGHWAY_AT_REST;
    const slow: number[] = [];
    for (let i = 0; i < 600; i++) {
      const on = Math.floor(i * DT) % 2 === 0;
      s = stepHighway(s, on ? music(i * DT, { release: 1 }) : null, DT);
      slow.push(highwayLuminance(s.glow, s.bloom));
    }
    expect(countFlashes(slow)).toBeGreaterThan(3);
  });

  it("uzun bir sekme aradan sonra sıçramaz", () => {
    const after = stepHighway(HIGHWAY_AT_REST, music(1, { release: 1 }), 30);
    expect(after.glow).toBeLessThanOrEqual(ENERGY_SLEW_PER_SECOND * 0.25 + 1e-12);
    expect(after.dash).toBeLessThan(2);
  });
});
