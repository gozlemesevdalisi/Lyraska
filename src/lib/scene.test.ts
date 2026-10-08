import { afterEach, describe, expect, it, vi } from "vitest";
import { SCENES, loadScene, saveScene, sceneForKey } from "./scene";

describe("sahneler", () => {
  afterEach(() => {
    window.localStorage.clear();
    vi.restoreAllMocks();
  });

  it("adları ve 1–4 tuşlarını bilir", () => {
    expect(SCENES.map((s) => s.name)).toEqual([
      "Gece göğü",
      "Gece otoyolu",
      "VU ibreleri",
      "Spektrum",
    ]);
    expect(["1", "2", "3", "4"].map(sceneForKey)).toEqual(["sky", "highway", "vu", "spectrum"]);
    expect(sceneForKey("5")).toBeNull();
    expect(sceneForKey("0")).toBeNull();
    expect(sceneForKey("a")).toBeNull();
    expect(sceneForKey("12")).toBeNull();
  });

  it("seçimi hatırlar; bozuk kayıt ya da erişim hatasında gece göğüyle başlar", () => {
    expect(loadScene()).toBe("sky");
    saveScene("vu");
    expect(loadScene()).toBe("vu");
    saveScene("spectrum");
    expect(loadScene()).toBe("spectrum");
    window.localStorage.setItem("lyraska.scene", "bilinmeyen");
    expect(loadScene()).toBe("sky");
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("erişim yok");
    });
    expect(loadScene()).toBe("sky");
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("erişim yok");
    });
    expect(() => saveScene("vu")).not.toThrow();
  });
});
