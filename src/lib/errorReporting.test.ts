import { afterEach, describe, expect, it, vi } from "vitest";

const logged = vi.hoisted(() => vi.fn());
vi.mock("./backend", () => ({
  logFrontendError: async (message: string) => logged(message),
}));

const { createRepeatFilter, describeError, installErrorReporting, MAX_SAME_ERROR } =
  await import("./errorReporting");

describe("arayüz hata günlüğü", () => {
  afterEach(() => logged.mockReset());

  it("hatayı okunur biçimde yazar", () => {
    expect(describeError(new TypeError("x tanımsız"))).toMatch(/^TypeError: x tanımsız/);
    expect(describeError("düz metin")).toBe("düz metin");
    expect(describeError({ kod: 3 })).toBe('{"kod":3}');
  });

  it("yakalanmamış hatalar ve sonuçlanmayan sözler günlüğe gider", async () => {
    // Testin fırlattığı sahte hatalar test çalıştırıcısına "yakalanmamış" görünmesin.
    const swallow = (event: Event) => event.preventDefault();
    window.addEventListener("error", swallow);
    const uninstall = installErrorReporting(window);
    window.dispatchEvent(new ErrorEvent("error", { error: new Error("patladı") }));
    const rejection = new Event("unhandledrejection") as PromiseRejectionEvent;
    Object.defineProperty(rejection, "reason", { value: "söz reddedildi" });
    window.dispatchEvent(rejection);
    await Promise.resolve();
    expect(logged).toHaveBeenCalledWith(expect.stringContaining("Error: patladı"));
    expect(logged).toHaveBeenCalledWith("söz reddedildi");

    uninstall();
    window.dispatchEvent(new ErrorEvent("error", { error: new Error("sonra") }));
    await Promise.resolve();
    expect(logged).toHaveBeenCalledTimes(2);
    window.removeEventListener("error", swallow);
  });

  it("her karede tekrarlanan hata günlüğü doldurmaz", async () => {
    const swallow = (event: Event) => event.preventDefault();
    window.addEventListener("error", swallow);
    const uninstall = installErrorReporting(window);
    // Bir sahne 10 saniye boyunca her karede aynı hatayı atıyor (600 kez).
    for (let i = 0; i < 600; i++) {
      window.dispatchEvent(new ErrorEvent("error", { error: new Error("kare çizilemedi") }));
    }
    window.dispatchEvent(new ErrorEvent("error", { error: new Error("başka bir hata") }));
    await Promise.resolve();
    // İlk 3 tekrar ayrıntısıyla, sonra her 100 tekrarda bir sayıyla; farklı hata yine yazılır.
    expect(logged).toHaveBeenCalledTimes(MAX_SAME_ERROR + 6 + 1);
    expect(logged).toHaveBeenCalledWith("Error: kare çizilemedi (aynı hata 600. kez)");
    expect(logged).toHaveBeenLastCalledWith(expect.stringContaining("başka bir hata"));
    uninstall();
    window.removeEventListener("error", swallow);
  });

  it("tekrar süzgeci", () => {
    const filter = createRepeatFilter();
    expect(filter("a\nyığın")).toBe("a\nyığın");
    filter("a\nyığın");
    filter("a\nyığın");
    expect(filter("a\nyığın")).toBeNull();
    for (let i = 5; i < 100; i++) filter("a\nyığın");
    expect(filter("a\nyığın")).toBe("a (aynı hata 100. kez)");
    expect(filter("b")).toBe("b");
  });
});
