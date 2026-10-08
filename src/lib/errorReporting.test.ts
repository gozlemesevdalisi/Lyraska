import { afterEach, describe, expect, it, vi } from "vitest";

const logged = vi.hoisted(() => vi.fn());
vi.mock("./backend", () => ({
  logFrontendError: async (message: string) => logged(message),
}));

const { describeError, installErrorReporting } = await import("./errorReporting");

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
});
