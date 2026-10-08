import { useEffect, useState } from "react";

const WAKE_EVENTS = ["pointermove", "pointerdown", "keydown", "wheel"] as const;

/**
 * Kullanıcı `delayMs` boyunca fareye ve klavyeye dokunmadıysa `true`. Sinema görünümü
 * için: müzik çalarken düğmeler bir süre sonra çekilir, fare kıpırdayınca geri gelir.
 */
export function useIdle(delayMs: number): boolean {
  const [idle, setIdle] = useState(false);

  useEffect(() => {
    let timer = window.setTimeout(() => setIdle(true), delayMs);
    const wake = () => {
      setIdle(false);
      window.clearTimeout(timer);
      timer = window.setTimeout(() => setIdle(true), delayMs);
    };
    for (const name of WAKE_EVENTS) window.addEventListener(name, wake, { passive: true });
    return () => {
      window.clearTimeout(timer);
      for (const name of WAKE_EVENTS) window.removeEventListener(name, wake);
    };
  }, [delayMs]);

  return idle;
}
