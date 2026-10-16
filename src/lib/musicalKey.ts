import type { KeyMap, KeyRelation, MarkedKey, MusicalKey } from "./backend";

/**
 * Ton gösterimi (saf yardımcılar). Ton çekirdekte bulunur (`analysis::key`): tonik perde
 * sınıfı (0 = Do), majör/minör, güven ve Camelot sayısı. Güveni düşük ton çekirdekten hiç
 * gelmez (`KeyMap.key` boş).
 */

/** Tonun Türkçe adı: majörde düzlerin, minörde diyezlerin yaygın yazılışı. */
const MAJOR_NAMES = [
  "Do",
  "Re bemol",
  "Re",
  "Mi bemol",
  "Mi",
  "Fa",
  "Fa diyez",
  "Sol",
  "La bemol",
  "La",
  "Si bemol",
  "Si",
];
const MINOR_NAMES = [
  "Do",
  "Do diyez",
  "Re",
  "Mi bemol",
  "Mi",
  "Fa",
  "Fa diyez",
  "Sol",
  "Sol diyez",
  "La",
  "Si bemol",
  "Si",
];
/** Harfle yazılışı (akor adları gibi): Am, C, F#m. */
const MAJOR_LETTERS = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const MINOR_LETTERS = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "G#", "A", "Bb", "B"];

type KeyLike = Pick<MusicalKey, "tonic" | "minor">;

/** Bunun altındaki güvenle bulunan ton programda gösterilmez (çekirdekteki `KEY_CONFIDENCE_MIN`). */
export const KEY_CONFIDENCE_MIN = 0.35;

/** Camelot sayısı (çekirdekteki hesabın aynısı): Do majör 8, La minör 8; beşlide bir adım. */
export function camelotNumber(key: KeyLike): number {
  const major = (((key.minor ? key.tonic + 3 : key.tonic) % 12) + 12) % 12;
  return ((((7 * major) % 12) + 7) % 12) + 1;
}

/** Camelot kodu: "8A" (minör) ya da "8B" (majör). */
export function camelotCode(key: KeyLike): string {
  return `${camelotNumber(key)}${key.minor ? "A" : "B"}`;
}

/** "La minör", "Fa diyez majör". */
export function keyName(key: KeyLike): string {
  const names = key.minor ? MINOR_NAMES : MAJOR_NAMES;
  return `${names[key.tonic % 12] ?? "?"} ${key.minor ? "minör" : "majör"}`;
}

/** "Am", "C", "F#m". */
export function keyLetters(key: KeyLike): string {
  const letters = key.minor ? MINOR_LETTERS : MAJOR_LETTERS;
  return `${letters[key.tonic % 12] ?? "?"}${key.minor ? "m" : ""}`;
}

/** Bilgi kartı: "8A · La minör". */
export function keyLabel(key: KeyLike): string {
  return `${camelotCode(key)} · ${keyName(key)}`;
}

/** İşaretleme aracındaki seçim değeri ("9m" = La minör, "0M" = Do majör) ve tersi. */
export function markedKeyValue(key: MarkedKey | null): string {
  return key ? `${key.tonic}${key.minor ? "m" : "M"}` : "";
}

export function parseMarkedKey(value: string): MarkedKey | null {
  const match = /^(\d{1,2})([mM])$/.exec(value);
  if (!match) return null;
  const tonic = Number(match[1]);
  return tonic >= 0 && tonic < 12 ? { tonic, minor: match[2] === "m" } : null;
}

/** Ton seçimi: Camelot çarkının sırasıyla, önce minörler (1A … 12A), sonra majörler. */
export const MARKED_KEY_OPTIONS: { value: string; label: string }[] = [true, false].flatMap(
  (minor) =>
    Array.from({ length: 12 }, (_, tonic) => ({ tonic, minor }))
      .sort((a, b) => camelotNumber(a) - camelotNumber(b))
      .map((key) => ({
        value: markedKeyValue(key),
        label: `${camelotCode(key)} · ${keyName(key)} (${keyLetters(key)})`,
      })),
);

/** Bulunan tonun işaretlenen tona göre ilişkisi, sade Türkçeyle. */
export function relationText(relation: KeyRelation): string {
  switch (relation) {
    case "same":
      return "aynı ton";
    case "fifth":
      return "komşu ton (Camelot'ta bir yan)";
    case "relative":
      return "paralel ton (aynı Camelot sayısı)";
    case "parallel":
      return "aynı adlı, öteki kip (majör ↔ minör)";
    case "other":
      return "farklı ton";
  }
}

/** Yol haritasındaki ölçüt: aynı ton ya da Camelot komşusu isabet sayılır. */
export function isKeyHit(relation: KeyRelation): boolean {
  return relation === "same" || relation === "fifth" || relation === "relative";
}

/**
 * Konumdaki ton: tonu farklı bir bölümdeyse o bölümün tonu, değilse şarkının tonu. `changed`:
 * bölümün tonu şarkınınkinden farklı mı (ton değişimi).
 */
export function keyAt(
  map: KeyMap | null,
  sections: readonly { start: number; end: number }[],
  positionSecs: number,
): { key: MusicalKey; changed: boolean } | null {
  if (!map) return null;
  const index = sections.findIndex((s) => positionSecs >= s.start && positionSecs < s.end);
  const section = index >= 0 ? map.sections[index] : null;
  if (section) return { key: section, changed: true };
  return map.key ? { key: map.key, changed: false } : null;
}
