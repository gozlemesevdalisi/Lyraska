import { useCallback, useEffect, useRef, type ReactNode } from "react";
import type { VisualFrame } from "../lib/backend";
import { HIGHWAY_AT_REST, stepHighway, type HighwayInput, type HighwayState } from "../lib/highway";
import { createHighwayRenderer, type HighwayRenderer } from "../lib/highwayRenderer";
import { bandEnergies } from "../lib/sky";
import { usePrefersReducedMotion } from "../lib/usePrefersReducedMotion";
import { useVisualFeed } from "../hooks/useVisualFeed";

export interface HighwaySceneProps {
  playing: boolean;
  /** Epilepsi güvenli modu: parlaklık yarı hızla değişir, yol iki kat hızlanmaz. */
  safe?: boolean;
  /** Görüntünün önünde gösterilecek içerik (süre). */
  center: ReactNode;
}

/**
 * "Gece otoyolu" sahnesi: ufukta şehir ışıkları olan boş bir yolda gece sürüşü.
 * Şerit çizgileri vuruşlarla, sokak lambaları ölçü başlarıyla geçer; ufuk
 * parıltısı müziğin enerjisiyle güçlenir, rengi bölüme göre değişir. Droptan
 * önce parıltı ufka çekilir, drop'ta yükselir ve yol hızlanır. Ekran kartında
 * (WebGL2) çizilir; WebGL2 yoksa durgun bir görüntü kalır.
 */
export function HighwayScene({ playing, safe = false, center }: HighwaySceneProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const renderer = useRef<HighwayRenderer | null>(null);
  const road = useRef<HighwayState>(HIGHWAY_AT_REST);
  const reducedMotion = usePrefersReducedMotion();

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const created = createHighwayRenderer(canvas);
    renderer.current = created;
    canvas.dataset.webgl = created ? "on" : "off";
    return () => {
      created?.dispose();
      renderer.current = null;
    };
  }, []);

  // Her ekran karesinde yol ilerletilip doğrudan çizilir; React yeniden çizim yapmaz.
  const onTick = useCallback(
    (frame: VisualFrame | null, dt: number) => {
      road.current = stepHighway(
        road.current,
        frame ? highwayInput(frame) : null,
        dt,
        reducedMotion,
        safe,
      );
      renderer.current?.draw(road.current);
    },
    [reducedMotion, safe],
  );
  useVisualFeed(playing, onTick);

  return (
    <div className="sky-scene">
      <canvas
        ref={canvasRef}
        className="sky-scene__canvas highway-scene__canvas"
        role="img"
        aria-label="Gece otoyolu: vuruşlarla akan yol, ölçü başlarında geçen sokak lambaları ve ufukta şehir ışıkları"
      />
      <div className="sky-scene__center">{center}</div>
    </div>
  );
}

/** Görsel verinin yolun kullandığı kısmı. */
export function highwayInput(frame: VisualFrame): HighwayInput {
  const d = frame.director;
  return {
    energies: bandEnergies(frame.bands),
    beat: frame.beat ? { bpm: frame.beat.bpm, phase: frame.beat.phase } : null,
    director: d
      ? {
          mood: d.atmosphere.mood,
          theme: d.atmosphere.theme,
          barPhase: d.rhythm.barPhase,
          anticipation: d.rhythm.anticipation,
          release: d.rhythm.release,
        }
      : null,
  };
}
