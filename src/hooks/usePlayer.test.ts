import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaybackStatus } from "../lib/backend";
import { usePlayer } from "./usePlayer";

const backend = vi.hoisted(() => ({
  statusCalls: 0,
  /** İlk yanıttan sonra çekirdek meşgul: durum sorguları yanıtlanmaz (ör. şarkı açılıyor). */
  busy: false,
}));

const playing: PlaybackStatus = {
  state: "playing",
  track: {
    path: "C:/Müzik/gece.flac",
    fileName: "gece",
    title: "Gece",
    artist: null,
    album: null,
    albumArtist: null,
    trackNumber: null,
    discNumber: null,
    codec: "flac",
    sampleRate: 44100,
    channels: 2,
    durationSecs: 200,
  },
  positionSecs: 1,
  underruns: 0,
  error: null,
  bpm: null,
  output: null,
};

vi.mock("../lib/backend", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/backend")>();
  return {
    ...actual,
    isDesktop: () => true,
    openTrack: async () => playing.track,
    getPlaybackStatus: () => {
      backend.statusCalls++;
      return backend.busy ? new Promise<PlaybackStatus>(() => {}) : Promise.resolve(playing);
    },
  };
});

describe("oynatıcının durum sorgusu", () => {
  afterEach(() => vi.useRealTimers());

  it("çekirdek meşgulken sorgular birikmez: yanıt gelmeden yenisi gönderilmez", async () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => usePlayer(["flac"]));
    await act(async () => {
      await result.current.openPath("C:/Müzik/gece.flac");
    });
    expect(result.current.status.track).not.toBeNull();
    backend.busy = true;
    backend.statusCalls = 0;
    // Çekirdek 3 sn meşgul (şarkı açılırken kilit tutuluyor): her 250 ms'de bir sorgu
    // gönderilseydi 12 istek birikip çekirdeğin iş parçacıklarını bekletirdi.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(backend.statusCalls).toBe(1);
  });
});
