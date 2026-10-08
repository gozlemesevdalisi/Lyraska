import { afterEach, describe, expect, it, vi } from "vitest";

const logged = vi.hoisted(() => ({ messages: [] as string[] }));

vi.mock("./backend", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./backend")>();
  return {
    ...actual,
    logFrontendError: async (message: string) => {
      logged.messages.push(message);
    },
  };
});

import { linkFullscreenProgram, openScene, retrySetup } from "./gl";

/** Gölgelendirici derleyen ama derleme hatası veren sahte bir WebGL2 bağlamı. */
function brokenGl(log: string): WebGL2RenderingContext {
  return {
    VERTEX_SHADER: 1,
    FRAGMENT_SHADER: 2,
    COMPILE_STATUS: 3,
    createShader: () => ({}),
    shaderSource: () => {},
    compileShader: () => {},
    getShaderParameter: () => false,
    getShaderInfoLog: () => log,
    deleteShader: () => {},
  } as unknown as WebGL2RenderingContext;
}

describe("sahne kurulumu", () => {
  afterEach(() => {
    logged.messages = [];
    vi.restoreAllMocks();
  });

  it("derleme hatası sürücünün mesajıyla bildirilir", () => {
    expect(() => linkFullscreenProgram(brokenGl("ERROR: 0:12: 'uLyra' : undeclared"), "")).toThrow(
      /köşe gölgelendiricisi derlenemedi: ERROR: 0:12: 'uLyra' : undeclared/,
    );
  });

  it("kurulum hatası tuvale işlenir, günlüğe yazılır; düzelince silinir", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const canvas = document.createElement("canvas");
    expect(
      retrySetup(canvas, "Gece göğü", () => {
        throw new Error("parça gölgelendiricisi derlenemedi: X");
      }),
    ).toBeNull();
    expect(canvas.dataset.webglError).toBe("parça gölgelendiricisi derlenemedi: X");
    expect(logged.messages).toEqual([
      "Gece göğü sahnesi ekran kartında çizilemedi: parça gölgelendiricisi derlenemedi: X",
    ]);
    expect(retrySetup(canvas, "Gece göğü", () => 42)).toBe(42);
    expect(canvas.dataset.webglError).toBeUndefined();
  });

  it("WebGL2 yoksa neden bildirilir", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const canvas = document.createElement("canvas");
    vi.spyOn(canvas, "getContext").mockImplementation(() => null);
    expect(openScene(canvas, "Gece otoyolu", () => 1)).toBeNull();
    expect(canvas.dataset.webglError).toMatch(/WebGL2 açılamadı/);
    expect(logged.messages[0]).toMatch(/^Gece otoyolu sahnesi ekran kartında çizilemedi/);
  });
});
