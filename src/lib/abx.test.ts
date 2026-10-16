import type { AbxStatus } from "./backend";
import { baseName, binomialTail, neededCorrect, percentText, setupSummary, verdict } from "./abx";

describe("kör test hesapları", () => {
  it("binom kuyruğu çekirdekle aynı değerleri verir", () => {
    expect(binomialTail(12, 16)).toBeCloseTo(2517 / 65536, 12);
    expect(binomialTail(8, 16)).toBeCloseTo(39203 / 65536, 12);
    expect(binomialTail(0, 16)).toBe(1);
    expect(binomialTail(17, 16)).toBe(0);
  });

  it("kaç doğru gerektiğini söyler", () => {
    expect(neededCorrect(16)).toBe(12);
    expect(neededCorrect(8)).toBe(7);
    expect(neededCorrect(20)).toBe(15);
  });

  it("yüzdeyi Türkçe yazar", () => {
    expect(percentText(2517 / 65536)).toBe("%3,8");
    expect(percentText(0.5982)).toBe("%59,8");
    expect(percentText(1 / 65536)).toBe("%0,1'den az");
  });

  it("sonucu sade anlatır", () => {
    const heard = verdict({ correct: 12, total: 16, pValue: 2517 / 65536 });
    expect(heard.heard).toBe(true);
    expect(heard.title).toBe("Farkı duyuyorsunuz");
    expect(heard.detail).toContain("16 denemede 12 doğru");
    expect(heard.detail).toContain("%3,8");
    const not = verdict({ correct: 8, total: 16, pValue: 39203 / 65536 });
    expect(not.heard).toBe(false);
    expect(not.title).toBe("Ayırt edilemedi");
  });

  it("kurulumu özetler, küçük farkları yazmaz", () => {
    const status = {
      startSecs: 30,
      clipSecs: 20,
      levelDiffDb: -0.42,
      offsetMs: 25.06,
    } as AbxStatus;
    expect(setupSummary(status)).toBe(
      "00:30–00:50 · seviye farkı 0,4 dB eşitlendi · kayma 25,1 ms düzeltildi",
    );
    expect(setupSummary({ ...status, levelDiffDb: 0.01, offsetMs: 0 })).toBe("00:30–00:50");
  });

  it("dosya adını yoldan ayırır", () => {
    expect(baseName("C:\\Müzik\\Şarkı.flac")).toBe("Şarkı.flac");
    expect(baseName("/home/x/a.mp3")).toBe("a.mp3");
  });
});
