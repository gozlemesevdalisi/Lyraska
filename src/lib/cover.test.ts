import { describe, expect, it } from "vitest";
import { coverGradient } from "./cover";

describe("renk kapağı", () => {
  it("aynı ad her zaman aynı kapağı verir", () => {
    expect(coverGradient("Gülümse")).toBe(coverGradient("Gülümse"));
  });

  it("farklı adlar farklı kapak alır", () => {
    const covers = new Set(
      ["Gülümse", "Firuze", "Hayvan Terli", "Kelimeler Yetse"].map(coverGradient),
    );
    expect(covers.size).toBe(4);
  });

  it("geçerli bir CSS geçişi üretir", () => {
    expect(coverGradient("")).toMatch(
      /^linear-gradient\(135deg, hsl\(\d+ 78% 62%\), hsl\(\d+ 66% 44%\)\)$/,
    );
  });
});
