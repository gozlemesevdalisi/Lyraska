import { useEffect, useRef, useState } from "react";
import { DotMatrix } from "./DotMatrix";
import { spectrumColumns } from "./SpectrumDemo";
import { getVisualFrame } from "../lib/backend";
import { emptyMeter, resampleBands, stepMeter, type MeterState } from "../lib/meter";

export interface SpectrumViewProps {
  bands: number;
  rows: number;
  /** Çalıyor mu? Değilse çubuklar yumuşakça söner. */
  playing: boolean;
  className?: string;
}

/**
 * Gerçek spektrum: her ekran karesinde Rust'tan o an duyulan anın frekans
 * bantlarını alır (şarkı önceden analiz edildiği için tam eş zamanlıdır) ve
 * ölçer davranışıyla (hızlı yükseliş, yavaş iniş, tepe tutma) gösterir.
 */
export function SpectrumView({ bands, rows, playing, className }: SpectrumViewProps) {
  const [meter, setMeter] = useState<MeterState>(() => emptyMeter(bands));
  const targets = useRef<number[]>(new Array<number>(bands).fill(0));
  const inFlight = useRef(false);

  useEffect(() => {
    if (typeof window.requestAnimationFrame !== "function") return;
    let raf = 0;
    let last = performance.now();
    let cancelled = false;

    const tick = (now: number) => {
      if (cancelled) return;
      raf = window.requestAnimationFrame(tick);
      const dt = (now - last) / 1000;
      last = now;

      if (!playing) {
        targets.current = targets.current.map(() => 0);
      } else if (!inFlight.current) {
        // Önceki istek dönmeden yenisini gönderme: IPC kuyruğu birikmesin.
        inFlight.current = true;
        getVisualFrame()
          .then((frame) => {
            if (frame) targets.current = resampleBands(frame.bands, bands);
          })
          .catch(() => {
            /* Geçici hata: bir sonraki karede yeniden denenir. */
          })
          .finally(() => {
            inFlight.current = false;
          });
      }
      setMeter((prev) => {
        const next = stepMeter(prev, targets.current, dt);
        // Her şey sıfırdaysa yeniden çizme (duraklatılmışken boşuna iş yapma).
        const idle = next.peaks.every((p) => p === 0) && prev.peaks.every((p) => p === 0);
        return idle ? prev : next;
      });
    };

    raf = window.requestAnimationFrame(tick);
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(raf);
    };
  }, [bands, playing]);

  return (
    <DotMatrix
      columns={spectrumColumns(meter.levels, meter.peaks, rows)}
      rows={rows}
      label="Spektrum"
      className={className}
    />
  );
}
