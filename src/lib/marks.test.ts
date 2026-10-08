import { describe, expect, it } from "vitest";
import { EMPTY_MARKS, addMark, marksFrom, positionAtEvent, tapBpm, undoMark } from "./marks";

describe("işaretler", () => {
  it("sıralı ekler, türleri ayırır, geri alır", () => {
    let m = addMark(EMPTY_MARKS, "beat", 2.0);
    m = addMark(m, "beat", 1.0);
    m = addMark(m, "drop", 1.5);
    expect(m.beats).toEqual([1.0, 2.0]);
    expect(m.drops).toEqual([1.5]);
    m = undoMark(m);
    expect(m.drops).toEqual([]);
    m = undoMark(m);
    expect(m.beats).toEqual([2.0]);
    m = undoMark(undoMark(m));
    expect(m).toEqual(EMPTY_MARKS);
  });

  it("titremeyi ve geçersiz zamanı yok sayar", () => {
    let m = addMark(EMPTY_MARKS, "beat", 1.0);
    expect(addMark(m, "beat", 1.05)).toBe(m);
    m = addMark(m, "drop", 1.05); // başka tür: sayılır
    expect(m.drops).toEqual([1.05]);
    expect(addMark(m, "beat", Number.NaN)).toBe(m);
    expect(addMark(m, "beat", -1)).toBe(m);
  });

  it("vuruşlardan tempo", () => {
    const beats = Array.from({ length: 12 }, (_, i) => 1 + i * 0.5);
    expect(tapBpm(beats)).toBe(120);
    expect(tapBpm([1, 1.5, 2])).toBeNull();
  });

  it("kayıtlı işaretlerden başlar; tuş anı konumu", () => {
    expect(marksFrom([3, 1], [9])).toEqual({ beats: [1, 3], drops: [9], history: [] });
    expect(positionAtEvent(10, 1004, 1000)).toBeCloseTo(9.996, 6);
    expect(positionAtEvent(10, 1000, 1004)).toBe(10); // saat tutarsızsa düşme
    expect(positionAtEvent(0.001, 2000, 1000)).toBe(0);
  });
});
