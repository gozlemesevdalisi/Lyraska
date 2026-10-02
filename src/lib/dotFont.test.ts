import {
  GLYPH_HEIGHT,
  GLYPH_WIDTH,
  glyphFor,
  isSupported,
  textToColumns,
  toDisplayCase,
} from "./dotFont";

describe("dotFont", () => {
  it("Türkçe büyük harf kurallarını uygular", () => {
    expect(toDisplayCase("müzik çalar")).toBe("MÜZİK ÇALAR");
    expect(toDisplayCase("ılık")).toBe("ILIK");
  });

  it("bütün Türkçe harfleri destekler", () => {
    for (const char of "ABCÇDEFGĞHIİJKLMNOÖPRSŞTUÜVYZ0123456789") {
      expect(isSupported(char), char).toBe(true);
    }
  });

  it("her karakter 5x8 boyutundadır", () => {
    for (const char of "AÇĞİÖŞÜ09.-:!?/· ") {
      const rows = glyphFor(char);
      expect(rows).toHaveLength(GLYPH_HEIGHT);
      rows.forEach((row) => expect(row).toMatch(/^[#.]{5}$/));
    }
  });

  it("bilinmeyen karakterleri soru işareti olarak gösterir", () => {
    expect(glyphFor("€")).toEqual(glyphFor("?"));
  });

  it("metni doğru sayıda sütuna çevirir", () => {
    // 3 karakter x 5 sütun + 2 boşluk sütunu
    const columns = textToColumns("ABC");
    expect(columns).toHaveLength(3 * GLYPH_WIDTH + 2);
    columns.forEach((column) => expect(column).toHaveLength(GLYPH_HEIGHT));
    // A'nın ilk sütunu: üst nokta sönük, altı yanık
    expect(columns[0]).toEqual([false, true, true, true, true, true, true, false]);
  });

  it("Ç harfinin alt çengeli 8. satırda yanar", () => {
    const columns = textToColumns("Ç");
    expect(columns[2]![GLYPH_HEIGHT - 1]).toBe(true);
  });
});
