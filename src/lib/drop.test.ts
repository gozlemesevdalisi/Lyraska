import { describe, expect, it } from "vitest";
import type { DropOutcome } from "./backend";
import { dropSummary } from "./drop";

const outcome = (over: Partial<DropOutcome>): DropOutcome => ({
  tracks: [],
  addedTracks: 0,
  addedFolders: 0,
  already: 0,
  problems: [],
  ...over,
});

describe("dropSummary", () => {
  it("eklenen şarkıları ve klasörleri sayar", () => {
    expect(dropSummary(outcome({ addedTracks: 2, addedFolders: 1 }))).toEqual({
      text: "2 şarkı kütüphaneye eklendi · 1 klasör kütüphaneye eklendi, şarkıları taranıyor",
      problem: false,
    });
  });

  it("zaten kütüphanede olanı ayrıca söyler", () => {
    expect(dropSummary(outcome({ already: 1 })).text).toBe("Zaten kütüphanede");
    expect(dropSummary(outcome({ already: 3 })).text).toBe("Hepsi zaten kütüphanede");
    expect(dropSummary(outcome({ addedTracks: 1, already: 2 })).text).toBe(
      "1 şarkı kütüphaneye eklendi · 2 tanesi zaten kütüphanedeydi",
    );
    expect(dropSummary(outcome({ addedTracks: 1, already: 1 })).text).toBe(
      "1 şarkı kütüphaneye eklendi · biri zaten kütüphanedeydi",
    );
  });

  it("sorunu gösterir, fazlasını sayar", () => {
    const notice = dropSummary(
      outcome({
        addedTracks: 1,
        problems: ["Bu dosya türü desteklenmiyor: a.wma", "Bulunamadı: b.mp3"],
      }),
    );
    expect(notice).toEqual({
      text: "1 şarkı kütüphaneye eklendi · Bu dosya türü desteklenmiyor: a.wma (ve 1 sorun daha)",
      problem: true,
    });
    expect(dropSummary(outcome({ problems: ["Bu dosya türü desteklenmiyor: a.wma"] }))).toEqual({
      text: "Bu dosya türü desteklenmiyor: a.wma",
      problem: true,
    });
  });

  it("hiçbir şey yoksa bunu söyler (sessiz kalmaz)", () => {
    expect(dropSummary(outcome({}))).toEqual({
      text: "Bırakılanlarda şarkı ya da klasör bulunamadı.",
      problem: true,
    });
  });
});
