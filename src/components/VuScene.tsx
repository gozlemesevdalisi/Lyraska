import { useCallback, useRef, useState } from "react";
import { VuMeter } from "./VuMeter";
import type { VisualFrame } from "../lib/backend";
import {
  DEFAULT_REFERENCE_DB,
  NEEDLE_AT_REST,
  isAtRest,
  levelToPosition,
  stepLamp,
  stepNeedle,
  type Needle,
} from "../lib/vu";
import {
  STAGE_LIGHT_AT_REST,
  stageLightInput,
  stageLightStyle,
  stepStageLight,
  type StageLight,
} from "../lib/stageLight";
import { usePrefersReducedMotion } from "../lib/usePrefersReducedMotion";
import { useVisualFeed } from "../hooks/useVisualFeed";

interface Meters {
  needles: [Needle, Needle];
  lamps: [number, number];
}

const AT_REST: Meters = { needles: [NEEDLE_AT_REST, NEEDLE_AT_REST], lamps: [0, 0] };

/** Ölçerleri `dt` saniye ilerletir (saf; test edilebilir). */
export function stepMeters(prev: Meters, frame: VisualFrame | null, dt: number): Meters {
  const reference = frame?.vuReferenceDb ?? DEFAULT_REFERENCE_DB;
  const needles = prev.needles.map((needle, i) =>
    stepNeedle(needle, frame ? levelToPosition(frame.rmsDb[i] ?? -Infinity, reference) : 0, dt),
  ) as [Needle, Needle];
  const lamps = prev.lamps.map((lamp, i) => stepLamp(lamp, needles[i]!, dt)) as [number, number];
  return { needles, lamps };
}

export interface VuSceneProps {
  playing: boolean;
  /** Epilepsi güvenli modu: arka ışık yarı hızla değişir. */
  safe?: boolean;
}

/**
 * VU ibreleri sahnesi: sol ve sağ kanal için iki analog ölçer. Seviyeler şarkı
 * önceden analiz edilerek hesaplanır ve 0 VU şarkının yüksek bölümlerine göre
 * ayarlanır; ibreler gerçek VU ölçer gibi yaylı hareket eder.
 *
 * Arka ışık Görsel Yönetmen'i izler ([`stepStageLight`]): ölçerlerin arkasındaki hale bölüm
 * temasının renginde, ruh hâliyle güçlenir; droptan önce daralır, drop'ta açılır.
 */
export function VuScene({ playing, safe = false }: VuSceneProps) {
  const [meters, setMeters] = useState<Meters>(AT_REST);
  const sceneRef = useRef<HTMLDivElement>(null);
  const light = useRef<StageLight>(STAGE_LIGHT_AT_REST);
  const reducedMotion = usePrefersReducedMotion();

  const onTick = useCallback(
    (frame: VisualFrame | null, dt: number) => {
      setMeters((prev) => {
        const next = stepMeters(prev, frame, dt);
        // Her şey durgunsa yeniden çizme.
        const still = (m: Meters) => m.needles.every(isAtRest) && m.lamps.every((l) => l === 0);
        return still(prev) && still(next) ? prev : next;
      });
      // Arka ışık React'i beklemeden doğrudan CSS değişkenleriyle çizilir.
      light.current = stepStageLight(
        light.current,
        stageLightInput(frame?.director ?? null),
        dt,
        reducedMotion,
        safe,
      );
      const element = sceneRef.current;
      if (element) {
        for (const [name, value] of Object.entries(stageLightStyle(light.current))) {
          element.style.setProperty(name, value);
        }
      }
    },
    [reducedMotion, safe],
  );
  useVisualFeed(playing, onTick);

  return (
    <div className="vu-scene" ref={sceneRef}>
      <VuMeter channel="L" position={meters.needles[0].position} lamp={meters.lamps[0] > 0} />
      <VuMeter channel="R" position={meters.needles[1].position} lamp={meters.lamps[1] > 0} />
    </div>
  );
}
