import { afterEach, describe, expect, it, vi } from "vitest";
import { loadScene, nextScene, saveScene, sceneName } from "./scene";

describe("sahneler", () => {
  afterEach(() => {
    window.localStorage.clear();
    vi.restoreAllMocks();
  });

  it("sırayla döner", () => {
    expect(nextScene("spectrum")).toBe("vu");
    expect(nextScene("vu")).toBe("spectrum");
    expect(sceneName("vu")).toBe("VU ibreleri");
  });

  it("seçimi hatırlar; bozuk kayıt ya da erişim hatasında spektrumla başlar", () => {
    expect(loadScene()).toBe("spectrum");
    saveScene("vu");
    expect(loadScene()).toBe("vu");
    window.localStorage.setItem("lyraska.scene", "bilinmeyen");
    expect(loadScene()).toBe("spectrum");
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("erişim yok");
    });
    expect(loadScene()).toBe("spectrum");
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("erişim yok");
    });
    expect(() => saveScene("vu")).not.toThrow();
  });
});
