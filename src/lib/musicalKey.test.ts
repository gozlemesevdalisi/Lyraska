import type { KeyMap, MusicalKey } from "./backend";
import {
  MARKED_KEY_OPTIONS,
  camelotCode,
  camelotNumber,
  isKeyHit,
  keyAt,
  keyLabel,
  keyLetters,
  keyName,
  markedKeyValue,
  parseMarkedKey,
  relationText,
} from "./musicalKey";

const key = (tonic: number, minor: boolean, camelot: number): MusicalKey => ({
  tonic,
  minor,
  camelot,
  confidence: 0.8,
});

describe("ton gösterimi", () => {
  it("Camelot kodunu, Türkçe adı ve harfle yazılışı verir", () => {
    const am = key(9, true, 8);
    expect(camelotCode(am)).toBe("8A");
    expect(keyName(am)).toBe("La minör");
    expect(keyLetters(am)).toBe("Am");
    expect(keyLabel(am)).toBe("8A · La minör");
    const fSharp = key(6, false, 2);
    expect(camelotCode(fSharp)).toBe("2B");
    expect(keyName(fSharp)).toBe("Fa diyez majör");
    expect(keyLetters(fSharp)).toBe("F#");
    expect(keyName(key(10, false, 6))).toBe("Si bemol majör");
    expect(keyLetters(key(8, true, 1))).toBe("G#m");
  });

  it("konumdaki tonu verir: tonu farklı bölümde o bölümün tonu", () => {
    const sections = [
      { start: 0, end: 30 },
      { start: 30, end: 42 },
    ];
    const map: KeyMap = { key: key(0, false, 8), sections: [null, key(2, false, 10)] };
    expect(keyAt(map, sections, 10)).toEqual({ key: map.key, changed: false });
    expect(keyAt(map, sections, 35)).toEqual({ key: map.sections[1], changed: true });
    // Bölüm dışında (ör. harita yok) şarkının tonu.
    expect(keyAt(map, [], 35)).toEqual({ key: map.key, changed: false });
    // Güvenli ton yok: gösterilmez.
    expect(keyAt({ key: null, sections: [null, null] }, sections, 10)).toBeNull();
    expect(keyAt(null, sections, 10)).toBeNull();
  });
});

describe("işaretleme aracında ton", () => {
  it("24 tonu Camelot sırasıyla sunar; seçim değeri geri çözülür", () => {
    expect(MARKED_KEY_OPTIONS).toHaveLength(24);
    expect(MARKED_KEY_OPTIONS[0]!.label).toBe("1A · Sol diyez minör (G#m)");
    expect(MARKED_KEY_OPTIONS[7]!.label).toBe("8A · La minör (Am)");
    expect(MARKED_KEY_OPTIONS[19]!.label).toBe("8B · Do majör (C)");
    for (const option of MARKED_KEY_OPTIONS) {
      expect(markedKeyValue(parseMarkedKey(option.value))).toBe(option.value);
    }
    expect(parseMarkedKey("")).toBeNull();
    expect(parseMarkedKey("12m")).toBeNull();
    expect(markedKeyValue(null)).toBe("");
  });

  it("Camelot sayısı çekirdekle aynı hesaplanır", () => {
    expect(camelotNumber({ tonic: 0, minor: false })).toBe(8);
    expect(camelotNumber({ tonic: 9, minor: true })).toBe(8);
    expect(camelotNumber({ tonic: 7, minor: false })).toBe(9);
    expect(camelotNumber({ tonic: 1, minor: true })).toBe(12);
  });

  it("ilişkiyi sade anlatır; aynı, komşu ve paralel ton isabettir", () => {
    expect(relationText("same")).toBe("aynı ton");
    expect(isKeyHit("same") && isKeyHit("fifth") && isKeyHit("relative")).toBe(true);
    expect(isKeyHit("parallel") || isKeyHit("other")).toBe(false);
  });
});
