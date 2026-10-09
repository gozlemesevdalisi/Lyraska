import { HighwayScene } from "./HighwayScene";
import { SkyScene } from "./SkyScene";
import { SpectrumDemo } from "./SpectrumDemo";
import { SpectrumView } from "./SpectrumView";
import { VuScene } from "./VuScene";
import type { Scene } from "../lib/scene";

const SPECTRUM_BANDS = 16;
const SPECTRUM_ROWS = 10;

export interface SceneStageProps {
  scene: Scene;
  playing: boolean;
  hasTrack: boolean;
  /** Epilepsi güvenli modu. */
  safe: boolean;
}

/** Tüm pencereyi kaplayan görsel sahne. Sahne değişince yenisi yavaşça belirir. */
export function SceneStage({ scene, playing, hasTrack, safe }: SceneStageProps) {
  return (
    <div className="stage" data-scene={scene}>
      {scene === "vu" ? (
        <VuScene playing={playing} />
      ) : scene === "highway" ? (
        <HighwayScene playing={playing} safe={safe} />
      ) : scene === "spectrum" ? (
        <div className="spectrum-scene">
          {/* Şarkı açıkken gerçek spektrum; boştayken gösteri animasyonu. */}
          {hasTrack ? (
            <SpectrumView
              bands={SPECTRUM_BANDS}
              rows={SPECTRUM_ROWS}
              playing={playing}
              className="vfd vfd--primary"
            />
          ) : (
            <SpectrumDemo
              bands={SPECTRUM_BANDS}
              rows={SPECTRUM_ROWS}
              className="vfd vfd--primary"
            />
          )}
        </div>
      ) : (
        <SkyScene playing={playing} safe={safe} />
      )}
    </div>
  );
}
