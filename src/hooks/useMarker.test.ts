import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useMarker } from "./useMarker";

vi.mock("../lib/backend", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/backend")>();
  return {
    ...actual,
    getAnnotation: async () => null,
    saveAnnotation: async () => "kayit.json",
  };
});

const TRACK = "C:/Müzik/gece.flac";

describe("işaretleme ve ses gecikmesi", () => {
  it("Bluetooth gecikmesi işaretten düşülür: işaret duyulan vuruşun şarkıdaki anına düşer", async () => {
    // Oynatıcı 10,0 sn diyor; ses aygıtı 200 ms geç çalıyor (Senkron ayarı). Kullanıcı o an
    // şarkının 9,8. saniyesini duyuyor.
    const { result } = renderHook(() => useMarker(TRACK, true, () => 10, null, 200));
    await waitFor(() => expect(result.current.marks.beats).toEqual([]));
    act(() => result.current.mark("beat"));
    expect(result.current.marks.beats).toHaveLength(1);
    expect(result.current.marks.beats[0]).toBeCloseTo(9.8, 9);
    act(() => result.current.mark("drop"));
    expect(result.current.marks.drops[0]).toBeCloseTo(9.8, 9);
  });

  it("gecikme yoksa işaret oynatıcının konumundadır", async () => {
    const { result } = renderHook(() => useMarker(TRACK, true, () => 10, null, 0));
    await waitFor(() => expect(result.current.marks.beats).toEqual([]));
    act(() => result.current.mark("beat"));
    expect(result.current.marks.beats[0]).toBeCloseTo(10, 9);
  });

  it("Boşluk tuşuyla işaretlerken de gecikme düşülür", async () => {
    const { result } = renderHook(() => useMarker(TRACK, true, () => 10, null, 150));
    await waitFor(() => expect(result.current.marks.beats).toEqual([]));
    act(() => result.current.setRecording(true));
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { code: "Space", bubbles: true }));
    });
    expect(result.current.marks.beats).toHaveLength(1);
    // Tuşa basıştan olayın işlenmesine kadar geçen süre de (en fazla 0,25 sn) düşülür.
    expect(result.current.marks.beats[0]).toBeGreaterThan(9.6);
    expect(result.current.marks.beats[0]).toBeLessThanOrEqual(9.85);
  });
});
