import { afterEach, describe, expect, it, vi } from "vitest";
import { loadScene, nextScene, saveScene, sceneName } from "./scene";

describe("sahneler", () => {
  afterEach(() => {
    window.localStorage.clear();
    vi.restoreAllMocks();
  });

  it("sırayla döner", () => {
    expect(nextScene("spectrum")).toBe("vu");
    expect(nextScene("vu")).toBe("sky");
    expect(nextScene("sky")).toBe("highway");
    expect(nextScene("highway")).toBe("spectrum");
    expect(sceneName("vu")).toBe("VU ibreleri");
    expect(sceneName("sky")).toBe("Gece göğü");
  });

  it("seçimi hatırlar; bozuk kayıt ya da erişim hatasında spektrumla başlar", () => {
    expect(loadScene()).toBe("spectrum");
    saveScene("vu");
    expect(loadScene()).toBe("vu");
    saveScene("sky");
    expect(loadScene()).toBe("sky");
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
