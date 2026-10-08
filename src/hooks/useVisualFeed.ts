import { useEffect, useRef } from "react";
import { getVisualFrame, type VisualFrame } from "../lib/backend";
import { extrapolateFrame } from "../lib/sync";

/**
 * Her ekran karesinde `onTick(frame, dt)` çağırır. Çalarken `frame`, Rust'tan
 * alınan en son görsel veridir (o an duyulan anın önceden yapılmış analizi);
 * çalmıyorken `null`. Aynı anda tek istek gönderilir: IPC kuyruğu birikmez.
 *
 * Veri ekran karelerine tam denk gelmez (bazen aynı veri iki karede kullanılır).
 * Bu yüzden her karede verinin hesaplandığı andan bu yana geçen süre ölçülür ve
 * vuruşa bağlı değerler o kadar ileri alınır (`extrapolateFrame`): hareketler
 * takılmadan, tam vuruşunda akar.
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
    /** En son verinin hesaplandığı an (istek ile yanıtın ortası, ms). */
    let latestAt = 0;

    const loop = (now: number) => {
      if (cancelled) return;
      raf = window.requestAnimationFrame(loop);
      const dt = (now - last) / 1000;
      last = now;
      if (playing && !inFlight) {
        inFlight = true;
        const sentAt = performance.now();
        getVisualFrame()
          .then((frame) => {
            if (!cancelled && frame) {
              latest = frame;
              latestAt = (sentAt + performance.now()) / 2;
            }
          })
          .catch(() => {
            /* Geçici hata: bir sonraki karede yeniden denenir. */
          })
          .finally(() => {
            inFlight = false;
          });
      }
      // Uzun bir takılmadan sonra (sekme arka plandaydı) en fazla 250 ms ileri alınır.
      const age = Math.max(0, Math.min(0.25, (now - latestAt) / 1000));
      tick.current(playing && latest ? extrapolateFrame(latest, age) : null, dt);
    };

    raf = window.requestAnimationFrame(loop);
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(raf);
    };
  }, [playing]);
}
