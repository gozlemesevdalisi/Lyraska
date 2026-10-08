import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSync } from "./useSync";

const backend = vi.hoisted(() => ({
  delay: 0,
  saved: [] as number[],
}));

const CLICKS = Array.from({ length: 24 }, (_, i) => 2 + i * 0.6);
const TRACK = "C:/Veri/kalibrasyon/tiklama.wav";

vi.mock("../lib/backend", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/backend")>();
  return {
    ...actual,
    getAudioDelay: async () => backend.delay,
    setAudioDelay: async (ms: number) => {
      backend.saved.push(ms);
      backend.delay = ms;
      return ms;
    },
    getCalibrationTrack: async () => ({ path: TRACK, clicks: CLICKS }),
  };
});

function space() {
  window.dispatchEvent(new KeyboardEvent("keydown", { code: "Space", bubbles: true }));
}

describe("senkron ayarı", () => {
  beforeEach(() => {
    backend.delay = 40;
    backend.saved = [];
  });
  afterEach(() => vi.useRealTimers());

  it("kayıtlı ayarla başlar; kaydırınca yalnızca son değer kaydedilir", async () => {
    const { result } = renderHook(() =>
      useSync(
        async () => {},
        () => 0,
        null,
      ),
    );
    await waitFor(() => expect(result.current.delayMs).toBe(40));
    act(() => result.current.setDelay(100));
    act(() => result.current.setDelay(150));
    act(() => result.current.setDelay(9999));
    expect(result.current.delayMs).toBe(400);
    await waitFor(() => expect(backend.saved).toEqual([400]));
  });

  it("tıklamalara basılınca gecikmeyi ölçer, uygular ve geri alınabilir", async () => {
    let position = 0;
    const openPath = vi.fn(async () => {});
    const { result } = renderHook(() => useSync(openPath, () => position, null));
    await waitFor(() => expect(result.current.delayMs).toBe(40));

    await act(() => result.current.startCalibration());
    expect(openPath).toHaveBeenCalledWith(TRACK);
    expect(result.current.calibration.phase).toBe("listening");

    // Kullanıcı her tıklamayı 180 ms geç duyuyor (Bluetooth).
    for (const click of CLICKS) {
      position = click + 0.18;
      act(() => space());
    }
    const calibration = result.current.calibration;
    expect(calibration.phase).toBe("done");
    if (calibration.phase !== "done") return;
    expect(calibration.estimate.delayMs).toBe(180);
    expect(calibration.previousMs).toBe(40);
    expect(result.current.delayMs).toBe(180);
    await waitFor(() => expect(backend.saved).toContain(180));

    act(() => result.current.undoCalibration());
    expect(result.current.delayMs).toBe(40);
    expect(result.current.calibration.phase).toBe("idle");
  });

  it("kayıt bitince yetersiz basışta neden yazılır; ayar değişmez", async () => {
    let ended: string | null = null;
    let position = 0;
    const { result, rerender } = renderHook(() =>
      useSync(
        async () => {},
        () => position,
        ended,
      ),
    );
    await waitFor(() => expect(result.current.delayMs).toBe(40));
    await act(() => result.current.startCalibration());
    for (const click of CLICKS.slice(0, 3)) {
      position = click;
      act(() => space());
    }
    ended = TRACK;
    rerender();
    expect(result.current.calibration.phase).toBe("failed");
    expect(result.current.delayMs).toBe(40);
  });

  it("ölçüm sırasında yazı alanındaki Boşluk basış sayılmaz", async () => {
    const { result } = renderHook(() =>
      useSync(
        async () => {},
        () => 2,
        null,
      ),
    );
    await waitFor(() => expect(result.current.delayMs).toBe(40));
    await act(() => result.current.startCalibration());
    const input = document.createElement("input");
    document.body.append(input);
    act(() => {
      input.dispatchEvent(new KeyboardEvent("keydown", { code: "Space", bubbles: true }));
    });
    act(() => space());
    const calibration = result.current.calibration;
    expect(calibration.phase === "listening" && calibration.taps).toEqual([2]);
    input.remove();
  });

  it("ölçüm yokken Boşluk tuşuna karışmaz", async () => {
    const { result } = renderHook(() =>
      useSync(
        async () => {},
        () => 0,
        null,
      ),
    );
    await waitFor(() => expect(result.current.delayMs).toBe(40));
    const event = new KeyboardEvent("keydown", { code: "Space", cancelable: true });
    window.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });
});
