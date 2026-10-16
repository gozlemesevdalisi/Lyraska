import { useCallback, useRef, useState } from "react";
import { DotMatrix } from "./DotMatrix";
import { HighwayScene } from "./HighwayScene";
import { Marquee } from "./Marquee";
import { spectrumColumns } from "./SpectrumDemo";
import { VuMeter } from "./VuMeter";
import { AT_REST, stepMeters, type Meters } from "./VuScene";
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
import { isAtRest } from "../lib/vu";
import { usePrefersReducedMotion } from "../lib/usePrefersReducedMotion";
import { useVisualFeed } from "../hooks/useVisualFeed";

/** Teybin ekranındaki spektrum: bant ve satır sayısı. */
const DECK_BANDS = 16;
const DECK_ROWS = 5;
/** Kayan yazı spektrumla aynı genişlikte: iki satırın noktaları aynı aralıkta (~8 harf). */
const MARQUEE_COLUMNS = spectrumColumns(
  new Array<number>(DECK_BANDS).fill(0),
  [],
  DECK_ROWS,
).length;

export interface DashState {
  meters: Meters;
  spectrum: MeterState;
}

export const DASH_AT_REST: DashState = { meters: AT_REST, spectrum: emptyMeter(DECK_BANDS) };

/**
 * Torpidodaki ölçerleri `dt` saniye ilerletir (saf; test edilebilir): VU ibreleri ve teybin
 * ekranındaki spektrum. Spektrumun tepe noktaları sahne ışığına uyar ([`stagePeaks`]).
 */
export function stepDash(
  prev: DashState,
  frame: VisualFrame | null,
  dt: number,
  light: StageLight,
): DashState {
  const targets = frame
    ? resampleBands(frame.bands, DECK_BANDS)
    : new Array<number>(DECK_BANDS).fill(0);
  return {
    meters: stepMeters(prev.meters, frame, dt),
    spectrum: stepMeter(prev.spectrum, targets, dt, stagePeaks(light)),
  };
}

/** Her şey durgun mu (yeniden çizmeye gerek yok)? */
function dashAtRest(state: DashState): boolean {
  return (
    state.meters.needles.every(isAtRest) &&
    state.meters.lamps.every((l) => l === 0) &&
    state.spectrum.peaks.every((p) => p === 0)
  );
}

export interface DriveSceneProps {
  playing: boolean;
  /** Epilepsi güvenli modu: yol ve torpidonun ışığı yarı hızla değişir. */
  safe?: boolean;
  /** Teybin ekranında kayan yazı (çalan şarkı); şarkı yoksa programın adı. */
  title?: string | null;
}

/** Işık halkalı düğmeler (süs): yalnızca "çal" düğmesinin halkası çalarken yanar. */
const BUTTONS: { id: string; glyph: string }[] = [
  { id: "previous", glyph: "◀◀" },
  { id: "play", glyph: "▶" },
  { id: "pause", glyph: "❚❚" },
  { id: "next", glyph: "▶▶" },
  { id: "eject", glyph: "▲" },
];

/** Kaset makarası: göbek ve altı dişli yuva (özgün çizim). */
function Reel({ cx }: { cx: number }) {
  return (
    <g className="deck__reel">
      <circle cx={cx} cy={34} r={15} className="deck__reel-tape" />
      <circle cx={cx} cy={34} r={8} className="deck__reel-hub" />
      {Array.from({ length: 6 }, (_, i) => {
        const a = (i * Math.PI) / 3;
        return (
          <rect
            key={i}
            x={cx - 1.2}
            y={34 - 8}
            width={2.4}
            height={4}
            className="deck__reel-tooth"
            transform={`rotate(${(a * 180) / Math.PI} ${cx} 34)`}
          />
        );
      })}
    </g>
  );
}

/**
 * "Gece sürüşü" nostalji modu: ön camdan gece otoyolu (aynı çizim: çizgiler vuruşla, lambalar
 * ölçü başıyla), torpidoda özgün tasarım bir teyp: dönen makaralı kaset, nokta matris ekranda
 * kayan şarkı adı ve spektrum, arkadan aydınlatmalı VU ibreleri, ışık halkalı düğmeler.
 * Torpidonun ışığı Görsel Yönetmen'i izler (sahne ışığı: bölüm teması, ruh hâli, drop).
 * Hiçbir markanın adı, logosu ya da yazı tipi kullanılmaz.
 */
export function DriveScene({ playing, safe = false, title = null }: DriveSceneProps) {
  const [dash, setDash] = useState<DashState>(DASH_AT_REST);
  const dashRef = useRef<HTMLDivElement>(null);
  const light = useRef<StageLight>(STAGE_LIGHT_AT_REST);
  const reducedMotion = usePrefersReducedMotion();

  const onTick = useCallback(
    (frame: VisualFrame | null, dt: number) => {
      light.current = stepStageLight(
        light.current,
        stageLightInput(frame?.director ?? null),
        dt,
        reducedMotion,
        safe,
      );
      setDash((prev) => {
        const next = stepDash(prev, playing ? frame : null, dt, light.current);
        return dashAtRest(prev) && dashAtRest(next) ? prev : next;
      });
      const element = dashRef.current;
      if (element) {
        for (const [name, value] of Object.entries(stageLightStyle(light.current))) {
          element.style.setProperty(name, value);
        }
      }
    },
    [playing, reducedMotion, safe],
  );
  useVisualFeed(playing, onTick);

  const spinning = playing && !reducedMotion;
  return (
    <div className="drive-scene">
      <div className="drive-scene__windshield">
        <HighwayScene playing={playing} safe={safe} />
        <div className="drive-scene__frame" aria-hidden />
      </div>
      <div className="drive-scene__dash" ref={dashRef}>
        <section className="deck" aria-label="Teyp">
          <svg
            className={`deck__cassette${spinning ? " is-spinning" : ""}`}
            viewBox="0 0 120 68"
            role="img"
            aria-label={spinning ? "Kaset dönüyor" : "Kaset duruyor"}
          >
            <rect x={2} y={2} width={116} height={64} rx={6} className="deck__shell" />
            <rect x={14} y={10} width={92} height={14} rx={2} className="deck__label" />
            <rect x={30} y={24} width={60} height={22} rx={10} className="deck__window" />
            <Reel cx={42} />
            <Reel cx={78} />
          </svg>
          <div className="deck__display vfd vfd--primary">
            <Marquee text={title || "Lyraska"} width={MARQUEE_COLUMNS} />
            <DotMatrix
              columns={spectrumColumns(dash.spectrum.levels, dash.spectrum.peaks, DECK_ROWS)}
              rows={DECK_ROWS}
              label="Teybin spektrumu"
            />
          </div>
          <div className="deck__meters">
            <VuMeter
              channel="L"
              position={dash.meters.needles[0].position}
              lamp={dash.meters.lamps[0] > 0}
            />
            <VuMeter
              channel="R"
              position={dash.meters.needles[1].position}
              lamp={dash.meters.lamps[1] > 0}
            />
          </div>
          <div className="deck__buttons" aria-hidden>
            {BUTTONS.map((b) => (
              <span
                key={b.id}
                className={`deck__button${b.id === "play" && playing ? " is-lit" : ""}`}
              >
                {b.glyph}
              </span>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
