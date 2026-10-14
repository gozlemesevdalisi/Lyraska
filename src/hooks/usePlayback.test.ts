import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackStatus, TrackInfo } from "../lib/backend";
import { usePlayback } from "./usePlayback";

// Şarkı açmak çekirdekte zaman alır: her açılış, test "bitir" diyene kadar bekler.
const backend = vi.hoisted(() => ({
  current: null as string | null,
  opens: [] as { path: string; finish: () => void }[],
  next: [] as (string | null)[],
}));

const track = (path: string): TrackInfo => ({
  path,
  fileName: path,
  title: path,
  artist: null,
  album: null,
  albumArtist: null,
  trackNumber: null,
  discNumber: null,
  codec: "flac",
  sampleRate: 44100,
  channels: 2,
  durationSecs: 200,
});

const status = (): PlaybackStatus => ({
  state: backend.current ? "playing" : "idle",
  track: backend.current ? track(backend.current) : null,
  positionSecs: 0,
  underruns: 0,
  error: null,
  bpm: null,
  output: null,
});

vi.mock("../lib/backend", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/backend")>();
  return {
    ...actual,
    isDesktop: () => true,
    openTrack: (path: string) =>
      new Promise<TrackInfo>((resolve) =>
        backend.opens.push({
          path,
          finish: () => {
            backend.current = path;
            resolve(track(path));
          },
        }),
      ),
    getPlaybackStatus: async () => status(),
    setNextTrack: async (path: string | null) => {
      backend.next.push(path);
    },
  };
});

/** Bekleyen ilk açılışı bitirir ve arayüzün durumu almasını bekler. */
async function finishOpen(expected: string) {
  const open = backend.opens.shift();
  expect(open?.path).toBe(expected);
  await act(async () => {
    open!.finish();
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

beforeEach(() => {
  backend.current = null;
  backend.opens = [];
  backend.next = [];
});

describe("çalma sırası", () => {
  it("'Önceki' sırayı bozmaz: önceki şarkı sıradaki yerinde çalar, sonra devam eder (#51)", async () => {
    const { result } = renderHook(() => usePlayback(["flac"]));
    act(() => result.current.playList(["A", "B", "C"], "B"));
    await finishOpen("B");
    expect(result.current.queueLabel).toBe("2 / 3");

    // Şarkının başında (ilk 3 sn) "Önceki": A açılır. A açılana kadar B çalmaya devam eder.
    act(() => result.current.previous());
    await finishOpen("A");
    expect(result.current.player.status.track?.path).toBe("A");
    expect(result.current.queueLabel).toBe("1 / 3");
    expect(result.current.canNext).toBe(true);
    expect(result.current.upcoming).toBe("B");
    expect(backend.next.at(-1)).toBe("B");
  });

  it("listede çalanın hemen öncesindeki şarkı seçilince de sıra o şarkıdadır (#51)", async () => {
    const { result } = renderHook(() => usePlayback(["flac"]));
    act(() => result.current.playList(["X", "A", "Y"], "A"));
    await finishOpen("A");
    act(() => result.current.playList(["X", "A", "Y"], "X"));
    await finishOpen("X");
    expect(result.current.queueLabel).toBe("1 / 3");
    expect(result.current.upcoming).toBe("A");
  });

  it("boşluksuz geçişte sıra çekirdekle birlikte ilerler (art arda iki geçiş)", async () => {
    const { result } = renderHook(() => usePlayback(["flac"]));
    act(() => result.current.playList(["A", "B", "C"], "A"));
    await finishOpen("A");
    expect(result.current.upcoming).toBe("B");
    // Çekirdek A bitince B'ye kendisi geçer; sonra C'ye.
    for (const [path, label, upcoming] of [
      ["B", "2 / 3", "C"],
      ["C", "3 / 3", null],
    ] as const) {
      backend.current = path;
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 300));
      });
      expect(result.current.queueLabel).toBe(label);
      expect(result.current.upcoming).toBe(upcoming);
    }
  });

  it("şarkı açılırken seçilen ikinci şarkı kaybolmaz: son seçim çalar (#52)", async () => {
    const { result } = renderHook(() => usePlayback(["flac"]));
    act(() => result.current.playList(["A", "B", "C"], "B"));
    // B açılırken C seçildi.
    act(() => result.current.playList(["A", "B", "C"], "C"));
    await finishOpen("B");
    await finishOpen("C");
    expect(backend.opens).toHaveLength(0);
    expect(result.current.player.status.track?.path).toBe("C");
    expect(result.current.queueLabel).toBe("3 / 3");
  });

  it("açılış sürerken art arda seçilenlerden yalnızca sonuncusu açılır", async () => {
    const { result } = renderHook(() => usePlayback(["flac"]));
    act(() => result.current.playList(["A", "B", "C"], "A"));
    act(() => result.current.playList(["A", "B", "C"], "B"));
    act(() => result.current.playList(["A", "B", "C"], "C"));
    await finishOpen("A");
    await finishOpen("C");
    expect(backend.opens).toHaveLength(0);
    expect(result.current.queueLabel).toBe("3 / 3");
  });
});
