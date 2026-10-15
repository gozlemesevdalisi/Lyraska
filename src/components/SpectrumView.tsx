import { useCallback, useRef, useState } from "react";
import { DotMatrix } from "./DotMatrix";
import { spectrumColumns } from "./SpectrumDemo";
import type { VisualFrame } from "../lib/backend";
import { emptyMeter, resampleBands, stepMeter, type MeterState } from "../lib/meter";
import {
  STAGE_LIGHT_AT_REST,
  stageLightInput,
  stageLightStyle,
  stagePeaks,
  stepStageLight,
  type StageLight,
} from "../lib/stageLight";
import { usePrefersReducedMotion } from "../lib/usePrefersReducedMotion";
import { useVisualFeed } from "../hooks/useVisualFeed";

export interface SpectrumViewProps {
  bands: number;
  rows: number;
  /** Çalıyor mu? Değilse çubuklar yumuşakça söner. */
  playing: boolean;
  /** Epilepsi güvenli modu: ekranın ışıması yarı hızla değişir. */
  safe?: boolean;
  className?: string;
}

/**
 * Gerçek spektrum: her ekran karesinde Rust'tan o an duyulan anın frekans
 * bantlarını alır (şarkı önceden analiz edildiği için tam eş zamanlıdır) ve
 * ölçer davranışıyla (hızlı yükseliş, yavaş iniş, tepe tutma) gösterir.
 *
 * Ekranın ışıması Görsel Yönetmen'i izler ([`stepStageLight`]): rengi bölüm teması, gücü ruh
 * hâli; droptan önce tepe noktaları asılı kalır ve ışıma toplanır, drop'ta açılır.
 */
export function SpectrumView({ bands, rows, playing, safe = false, className }: SpectrumViewProps) {
  const [meter, setMeter] = useState<MeterState>(() => emptyMeter(bands));
  const targets = useRef<number[]>(new Array<number>(bands).fill(0));
  const frameRef = useRef<HTMLDivElement>(null);
  const light = useRef<StageLight>(STAGE_LIGHT_AT_REST);
  const reducedMotion = usePrefersReducedMotion();

  const onTick = useCallback(
    (frame: VisualFrame | null, dt: number) => {
      if (!playing) targets.current = targets.current.map(() => 0);
      else if (frame) targets.current = resampleBands(frame.bands, bands);
      light.current = stepStageLight(
        light.current,
        stageLightInput(frame?.director ?? null),
        dt,
        reducedMotion,
        safe,
      );
      const peaks = stagePeaks(light.current);
      setMeter((prev) => {
        const next = stepMeter(prev, targets.current, dt, peaks);
        // Her şey sıfırdaysa yeniden çizme (duraklatılmışken boşuna iş yapma).
        const idle = next.peaks.every((p) => p === 0) && prev.peaks.every((p) => p === 0);
        return idle ? prev : next;
      });
      // Işıma React'i beklemeden doğrudan CSS değişkenleriyle çizilir.
      const element = frameRef.current;
      if (element) {
        for (const [name, value] of Object.entries(stageLightStyle(light.current))) {
          element.style.setProperty(name, value);
        }
      }
    },
    [bands, playing, reducedMotion, safe],
  );
  useVisualFeed(playing, onTick);

  return (
    <div className="vfd-frame" ref={frameRef}>
      <DotMatrix
        columns={spectrumColumns(meter.levels, meter.peaks, rows)}
        rows={rows}
        label="Spektrum"
        className={className}
      />
    </div>
  );
}
