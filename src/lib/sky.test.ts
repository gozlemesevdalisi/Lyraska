import { describe, expect, it } from "vitest";
import { countFlashes } from "./flash";
import {
  AURORA_BASE,
  AURORA_MAX_LUMINANCE,
  BLOOM_LUMINANCE,
  ENERGY_SLEW_PER_SECOND,
  LYRA_LINES,
  LYRA_STARS,
  SKY_AT_REST,
  auroraLuminance,
  bandEnergies,
  parseHexColor,
  mixColors,
  stepSky,
  type DirectorInput,
  type Energies,
  type SkyState,
} from "./sky";

const DT = 1 / 60;

function run(state: SkyState, targets: Energies | null, seconds: number, reduced = false) {
  let s = state;
  for (let i = 0; i < Math.round(seconds / DT); i++) s = stepSky(s, targets, DT, reduced);
  return s;
}

describe("bant enerjileri", () => {
  it("bası, ortayı ve tizi ayrı ayrı ölçer", () => {
    const bands = Array.from({ length: 32 }, (_, i) => (i < 6 ? 0.8 : i < 19 ? 0.5 : 0.2));
    const [bass, mid, high] = bandEnergies(bands);
    expect(bass).toBeCloseTo(1, 5);
    expect(mid).toBeCloseTo(0.5, 5);
    expect(high).toBe(0);
  });

  it("boş ya da bozuk veride sıfır verir", () => {
    expect(bandEnergies([])).toEqual([0, 0, 0]);
    expect(bandEnergies([Number.NaN, 2, -1])).toEqual([0, 1, 0]);
  });
});

describe("gece göğü", () => {
  it("müzikle perdeler güçlenir, susunca dinlenmeye döner", () => {
    const loud = run(SKY_AT_REST, [1, 0.5, 0], 2);
    expect(loud.energies[0]).toBeGreaterThan(0.99);
    expect(loud.energies[1]).toBeCloseTo(0.5, 2);
    expect(loud.energies[2]).toBe(0);
    const quiet = run(loud, null, 4);
    expect(Math.max(...quiet.energies)).toBeLessThan(0.01);
  });

  it("parlaklık saniyede ENERGY_SLEW_PER_SECOND'dan hızlı değişmez", () => {
    let s = SKY_AT_REST;
    for (let i = 0; i < 600; i++) {
      const target: Energies = i % 2 ? [1, 1, 1] : [0, 0, 0];
      const next = stepSky(s, target, DT);
      next.energies.forEach((e, k) => {
        expect(Math.abs(e - s.energies[k]!)).toBeLessThanOrEqual(
          ENERGY_SLEW_PER_SECOND * DT + 1e-12,
        );
      });
      s = next;
    }
  });

  it("en kötü durumda bile saniyede 3'ten fazla parlamaz (epilepsi güvenliği)", () => {
    // Tüm bantlar aynı anda tam ölçekle sıfır arasında gidip geliyor: çeşitli hızlarda dene.
    for (const hz of [0.5, 1, 1.5, 2, 3, 4, 6, 8, 12, 20, 30]) {
      let s = SKY_AT_REST;
      const luminance: number[] = [];
      for (let i = 0; i < 600; i++) {
        const on = Math.floor(i * DT * hz * 2) % 2 === 0;
        s = stepSky(s, on ? [1, 1, 1] : [0, 0, 0], DT);
        luminance.push(auroraLuminance(s.energies));
      }
      expect(countFlashes(luminance) / 10, `${hz} Hz`).toBeLessThanOrEqual(2);
    }
    // Senaryo gerçekten parlatıp söndürebiliyor: yavaş değişimde parlamalar sayılır.
    let s = SKY_AT_REST;
    const slow: number[] = [];
    for (let i = 0; i < 600; i++) {
      s = stepSky(s, Math.floor(i * DT) % 2 === 0 ? [1, 1, 1] : [0, 0, 0], DT);
      slow.push(auroraLuminance(s.energies));
    }
    expect(countFlashes(slow)).toBeGreaterThan(3);
  });

  it("parlaklık sınırları doğru", () => {
    const calm = AURORA_MAX_LUMINANCE / (1 + BLOOM_LUMINANCE);
    expect(auroraLuminance([0, 0, 0])).toBeCloseTo(calm * AURORA_BASE, 10);
    expect(auroraLuminance([1, 1, 1])).toBeCloseTo(calm, 10);
    // Açılım dahil en parlak hâl bile üst sınırı aşmaz.
    expect(auroraLuminance([1, 1, 1], 1)).toBeCloseTo(AURORA_MAX_LUMINANCE, 10);
    expect(auroraLuminance([1, 1, 1], 5)).toBeCloseTo(AURORA_MAX_LUMINANCE, 10);
  });

  it("vuruşlar akışı hızlandırır", () => {
    const calm = run(SKY_AT_REST, [0, 0, 0], 1);
    const beat = run(SKY_AT_REST, [1, 0, 0], 1);
    expect(beat.flowTime).toBeGreaterThan(calm.flowTime * 3);
    expect(calm.flowTime).toBeCloseTo(0.35, 2);
  });

  it("animasyonları azalt açıkken yavaşlar ve yıldızlar kırpışmaz", () => {
    const normal = run(SKY_AT_REST, [1, 1, 1], 2);
    const reduced = run(SKY_AT_REST, [1, 1, 1], 2, true);
    expect(reduced.flowTime).toBeCloseTo(normal.flowTime * 0.25, 5);
    expect(reduced.twinkleTime).toBe(0);
    expect(normal.twinkleTime).toBeCloseTo(2, 5);
  });

  it("uzun bir sekme aradan sonra sıçramaz", () => {
    const after = stepSky(SKY_AT_REST, [1, 1, 1], 30);
    expect(after.energies[0]).toBeLessThanOrEqual(ENERGY_SLEW_PER_SECOND * 0.25 + 1e-12);
  });
});

const CALM: DirectorInput = {
  mood: 0,
  theme: 0,
  pulse: 0,
  accent: 0,
  anticipation: 0,
  release: 0,
};

function runDirected(
  state: SkyState,
  director: DirectorInput,
  seconds: number,
  reduced = false,
  safe = false,
) {
  let s = state;
  for (let i = 0; i < Math.round(seconds / DT); i++) {
    s = stepSky(s, [0.5, 0.5, 0.5], DT, reduced, director, safe);
  }
  return s;
}

describe("Görsel Yönetmen ile gök", () => {
  it("en kötü durumda bile saniyede 3'ten fazla parlamaz (normal ve güvenli mod)", () => {
    // Yönetmenin bütün değerleri ve bantlar aynı anda tam ölçekle gidip geliyor.
    for (const safe of [false, true]) {
      for (const hz of [0.5, 1, 1.5, 2, 3, 4, 6, 8, 12, 20, 30]) {
        let s = SKY_AT_REST;
        const luminance: number[] = [];
        let flips = 0;
        for (let i = 0; i < 600; i++) {
          const on = Math.floor(i * DT * hz * 2) % 2 === 0;
          const director: DirectorInput = on
            ? { mood: 1, theme: flips++, pulse: 1, accent: 1, anticipation: 0, release: 1 }
            : { ...CALM, anticipation: 1 };
          s = stepSky(s, on ? [1, 1, 1] : [0, 0, 0], DT, false, director, safe);
          luminance.push(auroraLuminance(s.energies, s.bloom));
        }
        expect(countFlashes(luminance) / 10, `${hz} Hz, güvenli: ${safe}`).toBeLessThanOrEqual(2);
      }
    }
  });

  it("güvenli modda parlaklık yarı hızla değişir", () => {
    const loud: DirectorInput = { ...CALM, mood: 1, release: 1 };
    const normal = stepSky(SKY_AT_REST, [1, 1, 1], 0.1, false, loud);
    const safe = stepSky(SKY_AT_REST, [1, 1, 1], 0.1, false, loud, true);
    expect(safe.bloom).toBeCloseTo(normal.bloom / 2, 10);
    expect(safe.energies[0]).toBeLessThanOrEqual(normal.energies[0] / 2 + 1e-12);
  });

  it("drop öncesi gerilim akışı yavaşlatır, drop açılımı hızlandırır", () => {
    const calm = runDirected(SKY_AT_REST, CALM, 3);
    const tense = runDirected(SKY_AT_REST, { ...CALM, anticipation: 1 }, 3);
    const bloom = runDirected(SKY_AT_REST, { ...CALM, release: 1 }, 3);
    expect(tense.tension).toBeCloseTo(1, 5);
    expect(tense.flowTime).toBeLessThan(calm.flowTime * 0.6);
    expect(bloom.bloom).toBeCloseTo(1, 5);
    expect(bloom.flowTime).toBeGreaterThan(calm.flowTime * 2);
  });

  it("sakin bölümde perdeler sönük, yoğun bölümde canlı", () => {
    const calm = runDirected(SKY_AT_REST, CALM, 3);
    const intense = runDirected(SKY_AT_REST, { ...CALM, mood: 1 }, 3);
    expect(calm.energies[0]).toBeCloseTo(0.5 * 0.55, 3);
    expect(intense.energies[0]).toBeCloseTo(0.5, 3);
  });

  it("tema değişince renk yumuşakça geçer", () => {
    const first = runDirected(SKY_AT_REST, { ...CALM, theme: 2 }, 0.5);
    expect(first.palette).toMatchObject({ from: 0, to: 2 });
    expect(first.palette.mix).toBeCloseTo(0.25, 2);
    const done = runDirected(first, { ...CALM, theme: 2 }, 2);
    expect(done.palette.mix).toBe(1);
    // Geçiş yarıda kesilirse çoğunluktaki temadan devam edilir.
    const back = runDirected(first, { ...CALM, theme: 1 }, DT);
    expect(back.palette.from).toBe(0);
    expect(mixColors([0, 0, 0], [1, 0.5, 0], 0.5)).toEqual([0.5, 0.25, 0]);
  });

  it("ölçü başında dalga başlar; animasyonları azalt açıkken başlamaz", () => {
    const before = runDirected(SKY_AT_REST, CALM, 1);
    const hit = stepSky(before, [0.5, 0.5, 0.5], DT, false, { ...CALM, accent: 1 });
    expect(hit.rippleTime).toBe(0);
    // Vurgu sönerken yeni dalga başlamaz.
    const fading = stepSky(hit, [0.5, 0.5, 0.5], DT, false, { ...CALM, accent: 0.9 });
    expect(fading.rippleTime).toBeCloseTo(DT, 10);
    const reduced = stepSky(before, [0.5, 0.5, 0.5], DT, true, { ...CALM, accent: 1 });
    expect(reduced.rippleTime).toBeGreaterThan(1);
  });

  it("çalmıyorken Yönetmen yok sayılır", () => {
    let s = runDirected(SKY_AT_REST, { ...CALM, release: 1, anticipation: 1 }, 2);
    for (let i = 0; i < 300; i++) {
      s = stepSky(s, null, DT, false, { ...CALM, release: 1, anticipation: 1 });
    }
    expect(s.bloom).toBe(0);
    expect(s.tension).toBe(0);
  });
});

describe("renkler ve takımyıldız", () => {
  it("CSS renklerini okur", () => {
    expect(parseHexColor("#ff8000", [0, 0, 0])).toEqual([1, 128 / 255, 0]);
    expect(parseHexColor(" #0f0 ", [0, 0, 0])).toEqual([0, 1, 0]);
    expect(parseHexColor("rgb(1,2,3)", [0.1, 0.2, 0.3])).toEqual([0.1, 0.2, 0.3]);
    expect(parseHexColor("", [0.5, 0.5, 0.5])).toEqual([0.5, 0.5, 0.5]);
  });

  it("Lyra çizgileri var olan yıldızları bağlar; en parlak yıldız Vega", () => {
    for (const [a, b] of LYRA_LINES) {
      expect(LYRA_STARS[a]).toBeDefined();
      expect(LYRA_STARS[b]).toBeDefined();
    }
    const brightest = [...LYRA_STARS].sort((a, b) => b.brightness - a.brightness)[0];
    expect(brightest?.name).toBe("Vega");
  });
});
