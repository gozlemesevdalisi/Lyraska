import { render, renderHook, screen, waitFor } from "@testing-library/react";
import { createElement } from "react";
import { ErrorBoundary } from "../components/ErrorBoundary";
import type { VisualFrame } from "../lib/backend";

const backend = vi.hoisted(() => ({
  frame: null as VisualFrame | null,
  calls: 0,
  logged: [] as string[],
}));

vi.mock("../lib/backend", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/backend")>()),
  getVisualFrame: async () => {
    backend.calls += 1;
    return backend.frame;
  },
  logFrontendError: async (message: string) => {
    backend.logged.push(message);
  },
}));

const { useVisualFeed } = await import("./useVisualFeed");

const frame: VisualFrame = {
  positionSecs: 12,
  bands: Array.from({ length: 32 }, () => 0.5),
  rmsDb: [-20, -20],
  peakDb: [-10, -10],
  vuReferenceDb: -16,
  beat: null,
  barBeat: null,
  energy: null,
  section: null,
  director: null,
} as unknown as VisualFrame;

describe("useVisualFeed", () => {
  beforeEach(() => {
    backend.frame = frame;
    backend.calls = 0;
    backend.logged = [];
  });

  it("çekirdek o an için veri vermeyince eski kareyi göstermez", async () => {
    const onTick = vi.fn();
    const { unmount } = renderHook(() => useVisualFeed(true, onTick));
    const last = () => onTick.mock.calls.at(-1)?.[0] as VisualFrame | null | undefined;
    await waitFor(() => expect(last()?.positionSecs).toBe(12));

    // Ör. şarkı değişti ve yeni analiz henüz o ana yetişmedi: sahne dinlenmeye geçmeli,
    // önceki şarkının son karesinde donup kalmamalı.
    backend.frame = null;
    await waitFor(() => expect(last()).toBeNull());
    unmount();
  });

  it("kaç bileşen dinlese de ekran karesi başına tek istek gider (#77)", async () => {
    // Gece sürüşünde dış sahne ve içindeki otoyol, çekmecedeki vuruş göstergesi: üç dinleyici.
    // Eskiden her biri kendi döngüsüyle ayrı ayrı `visual_frame` istiyordu.
    const ticks = [vi.fn(), vi.fn(), vi.fn()];
    const hooks = ticks.map((tick) => renderHook(() => useVisualFeed(true, tick)));
    await waitFor(() => expect(ticks[2]!.mock.calls.length).toBeGreaterThanOrEqual(12));
    const frames = Math.max(...ticks.map((t) => t.mock.calls.length));
    expect(backend.calls).toBeLessThanOrEqual(frames + 1);
    // Hepsi aynı kareyi alır.
    for (const tick of ticks) {
      expect((tick.mock.calls.at(-1)?.[0] as VisualFrame | null)?.positionSecs).toBe(12);
    }
    hooks.forEach((h) => h.unmount());
    // Dinleyici kalmayınca istek de kalmaz.
    const after = backend.calls;
    await new Promise((resolve) => setTimeout(resolve, 80));
    expect(backend.calls).toBe(after);
  });

  it("çalmayan dinleyici kare almaz, çalan alır", async () => {
    const idle = vi.fn();
    const active = vi.fn();
    const a = renderHook(() => useVisualFeed(false, idle));
    const b = renderHook(() => useVisualFeed(true, active));
    await waitFor(() =>
      expect((active.mock.calls.at(-1)?.[0] as VisualFrame | null)?.positionSecs).toBe(12),
    );
    expect(idle.mock.calls.every(([frame]) => frame === null)).toBe(true);
    a.unmount();
    b.unmount();
  });

  it("sahne döngüsündeki hata hata sınırına ulaşır (#77)", async () => {
    // Beklenmeyen bir `VisualFrame` sahnenin `tick`inde hata verirse eskiden hata her karede
    // yeniden fırlatılır, sahne donar, "Sahne çizilemedi" yedeği hiç çıkmazdı.
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    function Fragile() {
      useVisualFeed(true, (frame) => {
        if (frame) throw new TypeError("Cannot read properties of null (reading 'pulse')");
      });
      return createElement("p", null, "Sahne çalışıyor");
    }
    render(
      createElement(ErrorBoundary, {
        name: "Sahne",
        fallback: () => createElement("p", { role: "alert" }, "Sahne çizilemedi"),
        children: createElement(Fragile),
      }),
    );
    expect(screen.getByText("Sahne çalışıyor")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Sahne çizilemedi"));
    expect(backend.logged.join("\n")).toContain("Sahne çizilemedi");
    expect(backend.logged.join("\n")).toContain("pulse");
    vi.restoreAllMocks();
  });
});
