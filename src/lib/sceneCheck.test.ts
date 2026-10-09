import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { EqState } from "./backend";

/**
 * Sahne denetiminin (`scripts/check-scenes.mjs`) sahte çekirdeği, arayüzün beklediği verinin
 * bütün alanlarını döndürmeli: eksik bir alan arayüzü çökertir, sahne hiç çizilmez (0.0.29'da
 * yeni bas alanlarıyla oldu). Aşağıdaki tam örnek üretilen tipe bağlıdır: Rust'a yeni bir alan
 * eklenince bu dosya derlenmez, sahte çekirdek de birlikte güncellenir.
 */
const FULL_EQ_STATE: EqState = {
  enabled: true,
  gainsDb: [],
  bassDb: 0,
  smallSpeaker: false,
  bassDepth: 0,
  bassPunch: 0,
  maxBassDb: 18,
  bandsHz: [],
  maxGainDb: 12,
  preampDb: 0,
  curveHz: [],
  curveDb: [],
};

/** Sahte çekirdekte bir komutun döndürdüğü nesnenin metni. */
function fakeResponse(command: string): string {
  // Testler depo kökünde çalışır.
  const script = readFileSync("scripts/check-scenes.mjs", "utf8");
  const start = script.indexOf(`case "${command}": return {`);
  expect(start, `sahte çekirdekte "${command}" yok`).toBeGreaterThan(0);
  return script.slice(start, script.indexOf("};", start));
}

describe("sahne denetiminin sahte çekirdeği", () => {
  it("ekolayzer durumunun bütün alanlarını döndürür", () => {
    const response = fakeResponse("equalizer_get");
    for (const field of Object.keys(FULL_EQ_STATE)) {
      expect(response, `"${field}" eksik`).toMatch(new RegExp(`\\b${field}:`));
    }
  });
});
