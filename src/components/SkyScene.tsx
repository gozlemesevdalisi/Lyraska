import { useCallback, useEffect, useRef, type ReactNode } from "react";
import type { VisualFrame } from "../lib/backend";
import { SKY_AT_REST, bandEnergies, stepSky, type SkyState } from "../lib/sky";
import { createSkyRenderer, type SkyRenderer } from "../lib/skyRenderer";
import { usePrefersReducedMotion } from "../lib/usePrefersReducedMotion";
import { useVisualFeed } from "../hooks/useVisualFeed";

export interface SkySceneProps {
  playing: boolean;
  /** Gökyüzünün önünde gösterilecek içerik (süre). */
  center: ReactNode;
}

/**
 * "Gece göğü" sahnesi: uzak tepelerin üstünde kuzey ışıkları ve yıldızlar,
 * köşede Lyra takımyıldızı. Bas, orta ve tiz üç ayrı ışık perdesini yavaşça
 * güçlendirir; vuruşlar ışıkların akışını hızlandırır. Ekran kartında (WebGL2)
 * çizilir; WebGL2 yoksa durgun bir gök görünür.
 */
export function SkyScene({ playing, center }: SkySceneProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const renderer = useRef<SkyRenderer | null>(null);
  const sky = useRef<SkyState>(SKY_AT_REST);
  const reducedMotion = usePrefersReducedMotion();

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const created = createSkyRenderer(canvas);
    renderer.current = created;
    canvas.dataset.webgl = created ? "on" : "off";
    return () => {
      created?.dispose();
      renderer.current = null;
    };
  }, []);

  // Her ekran karesinde gök ilerletilip doğrudan çizilir; React yeniden çizim yapmaz.
  const onTick = useCallback(
    (frame: VisualFrame | null, dt: number) => {
      sky.current = stepSky(
        sky.current,
        frame ? bandEnergies(frame.bands) : null,
        dt,
        reducedMotion,
      );
      renderer.current?.draw(sky.current);
    },
    [reducedMotion],
  );
  useVisualFeed(playing, onTick);

  return (
    <div className="sky-scene">
      <canvas
        ref={canvasRef}
        className="sky-scene__canvas"
        role="img"
        aria-label="Gece göğü: müziğe göre dalgalanan kuzey ışıkları ve Lyra takımyıldızı"
      />
      <div className="sky-scene__center">{center}</div>
    </div>
  );
}
