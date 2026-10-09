import { useCallback, useEffect, useRef } from "react";
import type { VisualFrame } from "../lib/backend";
import {
  SKY_AT_REST,
  bandEnergies,
  stepSky,
  type DirectorInput,
  type SkyLook,
  type SkyState,
} from "../lib/sky";
import { createSkyFallback } from "../lib/skyFallback";
import { createSkyRenderer, type SkyRenderer } from "../lib/skyRenderer";
import { usePrefersReducedMotion } from "../lib/usePrefersReducedMotion";
import { useVisualFeed } from "../hooks/useVisualFeed";

export interface SkySceneProps {
  playing: boolean;
  /** Epilepsi güvenli modu: parlaklık yarı hızla değişir. */
  safe?: boolean;
  /** Manzara: göl, korona ya da karlı vadi. */
  look?: SkyLook;
}

/**
 * "Gece göğü" sahnesi: dağların üstünde kuzey ışıkları ve yıldızlar, köşede Lyra
 * takımyıldızı; manzara göl, korona ya da karlı vadi (Ayarlar). Bas, orta ve tiz üç ayrı ışık perdesini yavaşça
 * güçlendirir; vuruşlar ışıkların akışını hızlandırır. Görsel Yönetmen şarkıyı
 * önceden bildiği için perdelerin rengini bölüme göre seçer, droptan önce
 * ışıkları toplar ve drop anında açar. Ekran kartında (WebGL2) çizilir; WebGL2
 * açılamazsa (neden hata günlüğüne yazılır) aynı gök 2D yedek çizimle görünür,
 * o da olmazsa durgun bir gök.
 */
export function SkyScene({ playing, safe = false, look = "lake" }: SkySceneProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const fallbackRef = useRef<HTMLCanvasElement>(null);
  const renderer = useRef<Pick<SkyRenderer, "draw"> | null>(null);
  const sky = useRef<SkyState>(SKY_AT_REST);
  const reducedMotion = usePrefersReducedMotion();

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const created = createSkyRenderer(canvas, look);
    canvas.dataset.webgl = created ? "on" : "off";
    renderer.current = created;
    // Görünüm değişince yeni manzara hemen görünsün (duraklatılmışken de).
    created?.draw(sky.current);
    // Ekran kartı çizimi açılamadıysa: ayrı bir tuvalde 2D yedek çizim (WebGL bağlamı
    // alınmış bir tuvalde 2D çizilemez).
    const spare = fallbackRef.current;
    const fallback = !created && spare ? createSkyFallback(spare) : null;
    if (fallback && spare) {
      renderer.current = fallback;
      canvas.hidden = true;
      spare.hidden = false;
      spare.dataset.webgl = "off";
      spare.dataset.fallback = "2d";
      if (canvas.dataset.webglError) spare.dataset.webglError = canvas.dataset.webglError;
      fallback.draw(sky.current); // oynatma başlamadan da gök görünsün
    }
    return () => {
      created?.dispose();
      renderer.current = null;
    };
  }, [look]);

  // Her ekran karesinde gök ilerletilip doğrudan çizilir; React yeniden çizim yapmaz.
  const onTick = useCallback(
    (frame: VisualFrame | null, dt: number) => {
      sky.current = stepSky(
        sky.current,
        frame ? bandEnergies(frame.bands) : null,
        dt,
        reducedMotion,
        frame ? directorInput(frame) : null,
        safe,
      );
      renderer.current?.draw(sky.current);
    },
    [reducedMotion, safe],
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
      <canvas
        ref={fallbackRef}
        className="sky-scene__canvas"
        role="img"
        aria-label="Gece göğü: müziğe göre dalgalanan kuzey ışıkları ve Lyra takımyıldızı"
        hidden
      />
    </div>
  );
}

/** Yönetmen notunun göğün kullandığı kısmı; analiz bitmediyse `null`. */
function directorInput(frame: VisualFrame): DirectorInput | null {
  const d = frame.director;
  if (!d) return null;
  return {
    mood: d.atmosphere.mood,
    theme: d.atmosphere.theme,
    pulse: d.rhythm.pulse,
    accent: d.rhythm.accent,
    anticipation: d.rhythm.anticipation,
    release: d.rhythm.release,
  };
}
