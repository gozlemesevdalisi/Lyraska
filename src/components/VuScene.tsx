import { useCallback, useState } from "react";
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
}

/**
 * VU ibreleri sahnesi: sol ve sağ kanal için iki analog ölçer. Seviyeler şarkı
 * önceden analiz edilerek hesaplanır ve 0 VU şarkının yüksek bölümlerine göre
 * ayarlanır; ibreler gerçek VU ölçer gibi yaylı hareket eder.
 */
export function VuScene({ playing }: VuSceneProps) {
  const [meters, setMeters] = useState<Meters>(AT_REST);

  const onTick = useCallback((frame: VisualFrame | null, dt: number) => {
    setMeters((prev) => {
      const next = stepMeters(prev, frame, dt);
      // Her şey durgunsa yeniden çizme.
      const still = (m: Meters) => m.needles.every(isAtRest) && m.lamps.every((l) => l === 0);
      return still(prev) && still(next) ? prev : next;
    });
  }, []);
  useVisualFeed(playing, onTick);

  return (
    <div className="vu-scene">
      <VuMeter channel="L" position={meters.needles[0].position} lamp={meters.lamps[0] > 0} />
      <VuMeter channel="R" position={meters.needles[1].position} lamp={meters.lamps[1] > 0} />
    </div>
  );
}
