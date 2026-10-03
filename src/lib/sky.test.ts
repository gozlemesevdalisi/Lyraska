import { describe, expect, it } from "vitest";
import {
  AURORA_BASE,
  AURORA_MAX_LUMINANCE,
  ENERGY_SLEW_PER_SECOND,
  LYRA_LINES,
  LYRA_STARS,
  SKY_AT_REST,
  auroraLuminance,
  bandEnergies,
  parseHexColor,
  stepSky,
  type Energies,
  type SkyState,
} from "./sky";

const DT = 1 / 60;

function run(state: SkyState, targets: Energies | null, seconds: number, reduced = false) {
  let s = state;
  for (let i = 0; i < Math.round(seconds / DT); i++) s = stepSky(s, targets, DT, reduced);
  return s;
}

/**
 * WCAG 2.3.1'e göre parlama: bağıl parlaklıkta en az %10'luk, birbirine zıt iki
 * değişim. Parlaklık eğrisindeki bu tür dönüşleri sayar.
 */
function countFlashes(luminance: number[]): number {
  // Yükselirken en yüksek, düşerken en düşük değer izlenir; ondan %10 dönüş bir değişimdir.
  let min = luminance[0] ?? 0;
  let max = min;
  let direction = 0;
  let changes = 0;
  for (const l of luminance) {
    min = direction > 0 ? min : Math.min(min, l);
    max = direction < 0 ? max : Math.max(max, l);
    if (direction >= 0 && max - l >= 0.1) {
      direction = -1;
      changes++;
      min = l;
    } else if (direction <= 0 && l - min >= 0.1) {
      direction = 1;
      changes++;
      max = l;
    }
  }
  return Math.floor(changes / 2);
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
    expect(auroraLuminance([0, 0, 0])).toBeCloseTo(AURORA_MAX_LUMINANCE * AURORA_BASE, 10);
    expect(auroraLuminance([1, 1, 1])).toBeCloseTo(AURORA_MAX_LUMINANCE, 10);
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
