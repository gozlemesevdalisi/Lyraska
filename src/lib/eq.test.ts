import { describe, expect, it } from "vitest";
import {
  EQ_BANDS_HZ,
  EQ_PRESETS,
  curvePath,
  formatGain,
  formatHz,
  frequencyToRatio,
  matchPreset,
  snapBass,
  snapDepth,
  snapPunch,
  PUNCH_MAX_DB,
  BASS_CLUB_DB,
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
    const of = (
      gainsDb: number[],
      bassDb = 0,
      smallSpeaker = false,
      bassDepth = 0,
      bassPunch = 0,
    ) => ({ gainsDb, bassDb, smallSpeaker, bassDepth, bassPunch });
    expect(matchPreset(of(rock.gains, 0, false, 0, rock.bassPunch))).toBe("Rock");
    expect(matchPreset(of(Array(10).fill(0)))).toBe("Düz");
    const rockGains = rock.gains.map((g, i) => (i === 0 ? g + 0.5 : g));
    expect(matchPreset(of(rockGains, 0, false, 0, rock.bassPunch))).toBeNull();
    // Bas düğmesi, küçük hoparlör ve vuruş da hazır ayarın parçası.
    expect(matchPreset(of(rock.gains, 2, false, 0, rock.bassPunch))).toBeNull();
    expect(matchPreset(of(rock.gains, 0, true, 0, rock.bassPunch))).toBeNull();
    expect(matchPreset(of(rock.gains))).toBeNull();
    const small = EQ_PRESETS.find((p) => p.name === "Küçük hoparlör")!;
    expect(matchPreset(of(small.gains, small.bassDb, true, 0, small.bassPunch))).toBe(
      "Küçük hoparlör",
    );
    // Derinlik de.
    const club = EQ_PRESETS.find((p) => p.name === "Kulüp")!;
    const clubOf = (depth: number, punch: number) =>
      of(club.gains, club.bassDb, false, depth, punch);
    expect(matchPreset(clubOf(club.bassDepth, club.bassPunch))).toBe("Kulüp");
    expect(matchPreset(clubOf(0, club.bassPunch))).toBeNull();
    expect(matchPreset(clubOf(club.bassDepth, 0))).toBeNull();
  });

  it("hazır ayarlar 10 bant, ±8 dB içinde ve adları farklı", () => {
    // Bas ayarları +8 dB'ye kadar çıkar: eşitleme açıkken yükseltme sesi kısmadan yapılır
    // ve bas gerçekten hissedilir. Daha fazlası kulağı yorar.
    for (const preset of EQ_PRESETS) {
      expect(preset.gains).toHaveLength(10);
      expect(preset.gains.every((g) => Math.abs(g) <= 8 && snapGain(g) === g)).toBe(true);
      expect(snapBass(preset.bassDb)).toBe(preset.bassDb);
      expect(snapDepth(preset.bassDepth)).toBe(preset.bassDepth);
      expect(snapPunch(preset.bassPunch)).toBe(preset.bassPunch);
      // Küçük hoparlörde alt oktav çalınamaz.
      if (preset.smallSpeaker) expect(preset.bassDepth).toBe(0);
    }
    expect(new Set(EQ_PRESETS.map((p) => p.name)).size).toBe(EQ_PRESETS.length);
  });

  it("bas ayarları cihaza göre farklı yollarla bas verir", () => {
    const preset = (name: string) => EQ_PRESETS.find((p) => p.name === name)!;
    const at = (name: string, hz: number) => {
      const state = previewEqState({ enabled: true, ...pick(preset(name)) });
      return state.curveDb[state.curveHz.findIndex((f) => f >= hz)]!;
    };
    const pick = (p: (typeof EQ_PRESETS)[number]) => ({
      gainsDb: p.gains,
      bassDb: p.bassDb,
      smallSpeaker: p.smallSpeaker,
      bassDepth: p.bassDepth,
      bassPunch: p.bassPunch,
    });
    // "Bas": bas düğmesi ve 62–125 Hz birlikte; 62 Hz en az +8 dB.
    expect(preset("Bas").bassDb).toBeGreaterThanOrEqual(6);
    expect(at("Bas", 62)).toBeGreaterThanOrEqual(8);
    // "Derin bas": en çok en altta.
    expect(at("Derin bas", 31)).toBeGreaterThan(at("Derin bas", 125));
    // "Küçük hoparlör": harmonik bas açık; veremediği alt bas süzülür, 125–250 Hz yükselir.
    expect(preset("Küçük hoparlör").smallSpeaker).toBe(true);
    expect(at("Küçük hoparlör", 31)).toBeLessThan(0);
    expect(at("Küçük hoparlör", 200)).toBeGreaterThan(2);
    // "Kulüp": kulüp bölgesinde bas ve alt oktav; "Derin bas" da alt oktav kullanır.
    expect(preset("Kulüp").bassDb).toBeGreaterThan(BASS_CLUB_DB);
    expect(preset("Kulüp").bassDepth).toBeGreaterThanOrEqual(0.5);
    expect(at("Kulüp", 40)).toBeGreaterThan(12);
    expect(preset("Derin bas").bassDepth).toBeGreaterThan(0);
    // Vuruş: kulüpte en güçlü; davulun öne çıktığı ayarlarda da var.
    for (const name of ["Kulüp", "Elektronik", "Rock", "Bas", "Küçük hoparlör"]) {
      expect(preset(name).bassPunch, name).toBeGreaterThan(0);
    }
    expect(Math.max(...EQ_PRESETS.map((p) => p.bassPunch))).toBe(preset("Kulüp").bassPunch);
    // Diğerlerinde küçük hoparlör kapalı.
    expect(EQ_PRESETS.filter((p) => p.smallSpeaker).map((p) => p.name)).toEqual(["Küçük hoparlör"]);
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
    const flat = { bassDb: 0, smallSpeaker: false, bassDepth: 0, bassPunch: 0 };
    const state = previewEqState({ enabled: true, gainsDb: gains, ...flat });
    expect(state.preampDb).toBe(-6);
    const at31 = state.curveDb[state.curveHz.findIndex((f) => f >= 31.25)]!;
    expect(at31).toBeGreaterThan(5);
    expect(state.curveDb[state.curveDb.length - 1]).toBe(0);
    const off = previewEqState({
      enabled: false,
      gainsDb: gains,
      bassDb: 9,
      smallSpeaker: true,
      bassDepth: 0.5,
      bassPunch: 1,
    });
    expect(off.curveDb.every((d) => d === 0)).toBe(true);
    expect(off.gainsDb[0]).toBe(6);
    expect(off.bassDb).toBe(9);
    // Bas düğmesi alt frekansları yükseltir, ortaya dokunmaz; koruma ona göre.
    const bass = previewEqState({
      enabled: true,
      gainsDb: Array(10).fill(0),
      bassDb: 9,
      smallSpeaker: false,
      bassDepth: 0,
      bassPunch: 0,
    });
    expect(bass.curveDb[0]).toBeGreaterThan(8);
    expect(Math.abs(bass.curveDb[bass.curveHz.findIndex((f) => f >= 2000)]!)).toBeLessThan(0.1);
    expect(bass.preampDb).toBe(-9);
    // Derinlik eğride görünmez (doğrusal değil) ama korumaya girer.
    const deep = previewEqState({
      enabled: true,
      gainsDb: Array(10).fill(0),
      bassDb: 9,
      smallSpeaker: false,
      bassDepth: 1,
      bassPunch: 0,
    });
    expect(deep.curveDb).toEqual(bass.curveDb);
    expect(deep.preampDb).toBeLessThan(bass.preampDb);
    // Vuruş da eğride görünmez (yalnızca vuruşun ilk anı); en yüksek anı korumaya girer.
    const punchy = previewEqState({
      enabled: true,
      gainsDb: Array(10).fill(0),
      bassDb: 9,
      smallSpeaker: false,
      bassDepth: 0,
      bassPunch: 0.5,
    });
    expect(punchy.curveDb).toEqual(bass.curveDb);
    expect(punchy.preampDb).toBe(-(9 + PUNCH_MAX_DB / 2));
    expect(punchy.bassPunch).toBe(0.5);
    expect(snapPunch(0.62)).toBe(0.6);
    expect(snapPunch(-1)).toBe(0);
    expect(snapDepth(0.33)).toBe(0.35);
    expect(snapDepth(2)).toBe(1);
    expect(snapBass(19)).toBe(18);
    expect(snapBass(-1)).toBe(0);
    expect(snapBass(4.3)).toBe(4.5);
  });
});
