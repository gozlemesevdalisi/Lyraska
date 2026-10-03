import { useCallback, useRef, useState } from "react";
import { DotMatrix } from "./DotMatrix";
import { spectrumColumns } from "./SpectrumDemo";
import type { VisualFrame } from "../lib/backend";
import { emptyMeter, resampleBands, stepMeter, type MeterState } from "../lib/meter";
import { useVisualFeed } from "../hooks/useVisualFeed";

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

  const onTick = useCallback(
    (frame: VisualFrame | null, dt: number) => {
      if (!playing) targets.current = targets.current.map(() => 0);
      else if (frame) targets.current = resampleBands(frame.bands, bands);
      setMeter((prev) => {
        const next = stepMeter(prev, targets.current, dt);
        // Her şey sıfırdaysa yeniden çizme (duraklatılmışken boşuna iş yapma).
        const idle = next.peaks.every((p) => p === 0) && prev.peaks.every((p) => p === 0);
        return idle ? prev : next;
      });
    },
    [bands, playing],
  );
  useVisualFeed(playing, onTick);

  return (
    <DotMatrix
      columns={spectrumColumns(meter.levels, meter.peaks, rows)}
      rows={rows}
      label="Spektrum"
      className={className}
    />
  );
}
