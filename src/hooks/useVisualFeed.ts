import { useEffect, useRef } from "react";
import { getVisualFrame, type VisualFrame } from "../lib/backend";

/**
 * Her ekran karesinde `onTick(frame, dt)` çağırır. Çalarken `frame`, Rust'tan
 * alınan en son görsel veridir (o an duyulan anın önceden yapılmış analizi);
 * çalmıyorken `null`. Aynı anda tek istek gönderilir: IPC kuyruğu birikmez.
 */
export function useVisualFeed(
  playing: boolean,
  onTick: (frame: VisualFrame | null, dt: number) => void,
): void {
  const tick = useRef(onTick);
  useEffect(() => {
    tick.current = onTick;
  }, [onTick]);

  useEffect(() => {
    if (typeof window.requestAnimationFrame !== "function") return;
    let raf = 0;
    let last = performance.now();
    let cancelled = false;
    let inFlight = false;
    let latest: VisualFrame | null = null;

    const loop = (now: number) => {
      if (cancelled) return;
      raf = window.requestAnimationFrame(loop);
      const dt = (now - last) / 1000;
      last = now;
      if (playing && !inFlight) {
        inFlight = true;
        getVisualFrame()
          .then((frame) => {
            if (!cancelled && frame) latest = frame;
          })
          .catch(() => {
            /* Geçici hata: bir sonraki karede yeniden denenir. */
          })
          .finally(() => {
            inFlight = false;
          });
      }
      tick.current(playing ? latest : null, dt);
    };

    raf = window.requestAnimationFrame(loop);
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(raf);
    };
  }, [playing]);
}
