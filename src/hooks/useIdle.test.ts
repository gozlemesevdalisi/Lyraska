import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useIdle } from "./useIdle";

describe("useIdle", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("dokunulmayınca süre sonunda boşta sayılır, fare kıpırdayınca uyanır", () => {
    const { result } = renderHook(() => useIdle(4000));
    expect(result.current).toBe(false);
    act(() => vi.advanceTimersByTime(3999));
    expect(result.current).toBe(false);
    act(() => vi.advanceTimersByTime(1));
    expect(result.current).toBe(true);

    act(() => {
      window.dispatchEvent(new Event("pointermove"));
    });
    expect(result.current).toBe(false);
    // Her hareket süreyi baştan başlatır.
    act(() => vi.advanceTimersByTime(3000));
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "a" }));
    });
    act(() => vi.advanceTimersByTime(3000));
    expect(result.current).toBe(false);
    act(() => vi.advanceTimersByTime(1000));
    expect(result.current).toBe(true);
  });
});
