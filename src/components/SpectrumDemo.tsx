import { useEffect, useRef, useState } from "react";
import { DotMatrix } from "./DotMatrix";
import { usePrefersReducedMotion } from "../lib/usePrefersReducedMotion";

export interface SpectrumDemoProps {
  bands?: number;
  rows?: number;
  /** false ise hareket durur ve son görüntü donar (ör. duraklatılmışken). */
  active?: boolean;
  className?: string;
}

const BAR_WIDTH = 2;
const FRAME_MS = 40; // ~25 fps; eski ekranların ağır tazelenme hissi
const PEAK_HOLD_FRAMES = 12;
const PEAK_FALL = 0.035;

/**
 * Bant seviyelerini (0..1) ve tepe noktalarını nokta sütunlarına çevirir.
 * Her bant BAR_WIDTH sütun genişliğindedir, bantlar arasında bir boş sütun bulunur.
 */
export function spectrumColumns(levels: number[], peaks: number[], rows: number): boolean[][] {
  const columns: boolean[][] = [];
  levels.forEach((level, band) => {
    const litRows = Math.round(clamp01(level) * rows);
    const peakRow = Math.round(clamp01(peaks[band] ?? 0) * rows);
    const column: boolean[] = [];
    for (let y = 0; y < rows; y++) {
      const fromBottom = rows - y; // 1 = en alt satır
      column.push(fromBottom <= litRows || (peakRow > 0 && fromBottom === peakRow));
    }
    for (let w = 0; w < BAR_WIDTH; w++) columns.push(column);
    if (band < levels.length - 1) columns.push(new Array<boolean>(rows).fill(false));
  });
  return columns;
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

/** Basit, sabit tohumlu sözde rastgele sayı üreteci (her açılışta aynı gösteri). */
function createRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 0x100000000;
  };
}

/** Durgun bir şarkıyı andıran sabit spektrum (animasyon kapalıyken). */
function staticLevels(bands: number): number[] {
  return Array.from(
    { length: bands },
    (_, i) => 0.25 + 0.5 * Math.exp(-((i - bands * 0.3) ** 2) / (bands * 2)),
  );
}

/**
 * Karşılama ekranındaki "gösteri modu" spektrumu. Gerçek ses verisi değildir;
 * müziğe benzeyen yumuşak bir hareket üretir. Ani parlama içermez.
 */
export function SpectrumDemo({
  bands = 16,
  rows = 10,
  active = true,
  className,
}: SpectrumDemoProps) {
  const reducedMotion = usePrefersReducedMotion();
  const [state, setState] = useState(() => ({
    levels: staticLevels(bands),
    peaks: staticLevels(bands),
  }));
  const sim = useRef({
    levels: staticLevels(bands),
    targets: staticLevels(bands),
    peaks: staticLevels(bands),
    holds: new Array<number>(bands).fill(0),
    frame: 0,
    random: createRandom(2026),
  });

  useEffect(() => {
    if (!active || reducedMotion || typeof window.requestAnimationFrame !== "function") return;
    let raf = 0;
    let last = 0;

    const tick = (now: number) => {
      raf = window.requestAnimationFrame(tick);
      if (now - last < FRAME_MS) return;
      last = now;

      const s = sim.current;
      s.frame++;
      const beat = s.frame % 12 === 0; // ~120 BPM'lik bir vuruş hissi
      for (let i = 0; i < bands; i++) {
        const tilt = 1 - (i / bands) * 0.55; // basslar daha güçlü
        if (beat || s.random() < 0.18) {
          const kick = beat && i < bands / 3 ? 0.35 : 0;
          s.targets[i] = clamp01((0.2 + s.random() * 0.65) * tilt + kick);
        }
        const current = s.levels[i]!;
        const target = s.targets[i]!;
        // Hızlı yükselme, yavaş düşme (gerçek ölçerlerdeki gibi)
        s.levels[i] = current + (target - current) * (target > current ? 0.55 : 0.12);
        s.targets[i] = target * 0.94;

        if (s.levels[i]! >= s.peaks[i]!) {
          s.peaks[i] = s.levels[i]!;
          s.holds[i] = PEAK_HOLD_FRAMES;
        } else if (s.holds[i]! > 0) {
          s.holds[i]!--;
        } else {
          s.peaks[i] = Math.max(s.levels[i]!, s.peaks[i]! - PEAK_FALL);
        }
      }
      setState({ levels: [...s.levels], peaks: [...s.peaks] });
    };

    raf = window.requestAnimationFrame(tick);
    return () => window.cancelAnimationFrame(raf);
  }, [active, bands, reducedMotion]);

  return (
    <DotMatrix
      columns={spectrumColumns(state.levels, state.peaks, rows)}
      rows={rows}
      className={className}
    />
  );
}
