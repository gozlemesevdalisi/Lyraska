import { renderHook, waitFor } from "@testing-library/react";
import type { VisualFrame } from "../lib/backend";

const backend = vi.hoisted(() => ({ frame: null as VisualFrame | null }));

vi.mock("../lib/backend", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/backend")>()),
  getVisualFrame: async () => backend.frame,
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
  it("çekirdek o an için veri vermeyince eski kareyi göstermez", async () => {
    backend.frame = frame;
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
});
