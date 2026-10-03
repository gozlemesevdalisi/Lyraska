import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import type { EqualizerControls } from "../hooks/useEqualizer";
import {
  EQ_MAX_DB,
  EQ_PRESETS,
  EQ_STEP_DB,
  curvePath,
  formatGain,
  formatHz,
  frequencyToRatio,
  matchPreset,
} from "../lib/eq";

/** Eğri alanının dikey aralığı (±dB): sürgü yolu bunun 12/15'ini kaplar. */
const PLOT_RANGE_DB = EQ_MAX_DB + 3;
const PLOT_WIDTH = 1000;
const PLOT_HEIGHT = 200;
const GRID_DB = [12, 6, 0, -6, -12];

/** dB değerinin eğri alanındaki dikey konumu (üstten, yüzde). */
function dbToTopPercent(db: number): number {
  return 50 - (db / PLOT_RANGE_DB) * 50;
}

/** İşaretçinin sürgü yolundaki konumundan kazanç (dB). */
export function pointerToGain(clientY: number, top: number, height: number): number {
  if (height <= 0) return 0;
  const ratio = 1 - (clientY - top) / height;
  return -EQ_MAX_DB + Math.min(1, Math.max(0, ratio)) * 2 * EQ_MAX_DB;
}

interface EqSliderProps {
  hz: number;
  value: number;
  dimmed: boolean;
  onChange: (db: number) => void;
  /** ←/→ ile komşu banda geçiş. */
  onNeighbor: (direction: -1 | 1) => void;
  sliderRef: (element: HTMLDivElement | null) => void;
}

function EqSlider({ hz, value, dimmed, onChange, onNeighbor, sliderRef }: EqSliderProps) {
  const track = useRef<HTMLDivElement | null>(null);
  const dragging = useRef(false);
  const [active, setActive] = useState(false);
  const ratio = (value + EQ_MAX_DB) / (2 * EQ_MAX_DB);
  const label = `${formatHz(hz)}${hz >= 1000 ? "Hz" : " Hz"}`;

  const fromPointer = (event: PointerEvent<HTMLDivElement>) => {
    const rect = track.current?.getBoundingClientRect();
    if (rect) onChange(pointerToGain(event.clientY, rect.top, rect.height));
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const steps: Record<string, number> = {
      ArrowUp: EQ_STEP_DB,
      ArrowDown: -EQ_STEP_DB,
      PageUp: 3,
      PageDown: -3,
    };
    let handled = true;
    if (event.key in steps) onChange(value + steps[event.key]!);
    else if (event.key === "Home") onChange(EQ_MAX_DB);
    else if (event.key === "End") onChange(-EQ_MAX_DB);
    else if (event.key === "Delete" || event.key === "Backspace" || event.key === "0") onChange(0);
    else if (event.key === "ArrowLeft") onNeighbor(-1);
    else if (event.key === "ArrowRight") onNeighbor(1);
    else handled = false;
    if (handled) {
      // Ok tuşları şarkıyı sarmasın.
      event.preventDefault();
      event.stopPropagation();
    }
  };

  return (
    <div
      className={`eq-slider${dimmed ? " is-dimmed" : ""}${active ? " is-active" : ""}`}
      style={{ left: `${frequencyToRatio(hz) * 100}%` }}
    >
      <span className="eq-slider__value" aria-hidden>
        {formatGain(value)}
      </span>
      <div
        ref={(element) => {
          track.current = element;
          sliderRef(element);
        }}
        className="eq-slider__track"
        role="slider"
        tabIndex={0}
        aria-label={label}
        aria-orientation="vertical"
        aria-valuemin={-EQ_MAX_DB}
        aria-valuemax={EQ_MAX_DB}
        aria-valuenow={value}
        aria-valuetext={`${formatGain(value)} dB`}
        title={`${label}: ${formatGain(value)} dB (çift tıklayınca sıfırlanır)`}
        onKeyDown={onKeyDown}
        onDoubleClick={() => onChange(0)}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          dragging.current = true;
          setActive(true);
          event.currentTarget.setPointerCapture?.(event.pointerId);
          event.currentTarget.focus();
          fromPointer(event);
        }}
        onPointerMove={(event) => dragging.current && fromPointer(event)}
        onPointerUp={() => {
          dragging.current = false;
          setActive(false);
        }}
        onPointerCancel={() => {
          dragging.current = false;
          setActive(false);
        }}
      >
        <span
          className="eq-slider__fill"
          style={{
            bottom: `${Math.min(ratio, 0.5) * 100}%`,
            top: `${(1 - Math.max(ratio, 0.5)) * 100}%`,
          }}
        />
        <span className="eq-slider__thumb" style={{ bottom: `${ratio * 100}%` }} />
      </div>
    </div>
  );
}

export interface EqualizerPanelProps {
  equalizer: EqualizerControls;
}

/** 10 bantlı ekolayzer: hazır ayarlar, sürgüler ve gerçekten uygulanan eğri. */
export function EqualizerPanel({ equalizer }: EqualizerPanelProps) {
  const { settings, state, error } = equalizer;
  const sliders = useRef<(HTMLDivElement | null)[]>([]);
  const preset = matchPreset(settings.gainsDb);
  const enabled = settings.enabled;
  const path = curvePath(state.curveHz, state.curveDb, PLOT_WIDTH, PLOT_HEIGHT, PLOT_RANGE_DB);
  const zeroY = PLOT_HEIGHT / 2;
  const area = path ? `${path}L${PLOT_WIDTH},${zeroY}L0,${zeroY}Z` : "";

  const focusBand = (index: number) => {
    sliders.current[Math.max(0, Math.min(state.bandsHz.length - 1, index))]?.focus();
  };

  return (
    <section className="eq" aria-label="Ekolayzer">
      <header className="library__header eq__header">
        <h2 className="library__title">Ekolayzer</h2>
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          className={`switch${enabled ? " is-on" : ""}`}
          onClick={() => equalizer.setEnabled(!enabled)}
          title="Ekolayzeri aç / kapat (ayarlar kaybolmaz)"
        >
          <span className="switch__knob" aria-hidden />
          <span>{enabled ? "Açık" : "Kapalı"}</span>
        </button>
        <span className="eq__preset">{preset ?? "Özel ayar"}</span>
        <span
          className="eq__preamp"
          title="Yükseltilen frekanslar sesi bozmasın diye ses, en yüksek bant kadar kısılır."
        >
          Bozulma koruması: {formatGain(enabled ? state.preampDb : 0)} dB
        </span>
      </header>

      <div className="eq__presets" role="group" aria-label="Hazır ayarlar">
        {EQ_PRESETS.map((p) => (
          <button
            key={p.name}
            type="button"
            className={`chip chip--button${enabled && preset === p.name ? " is-selected" : ""}`}
            aria-pressed={enabled && preset === p.name}
            title={p.hint}
            onClick={() => equalizer.applyGains(p.gains)}
          >
            {p.name}
          </button>
        ))}
      </div>

      {error && <p className="library__notice library__problem">{error}</p>}

      <div className={`eq__board${enabled ? "" : " is-off"}`}>
        <div className="eq__scale" aria-hidden>
          {GRID_DB.map((db) => (
            <span key={db} style={{ top: `${dbToTopPercent(db)}%` }}>
              {formatGain(db, 0)}
            </span>
          ))}
        </div>
        <div className="eq__plot">
          <svg
            className="eq__curve"
            viewBox={`0 0 ${PLOT_WIDTH} ${PLOT_HEIGHT}`}
            preserveAspectRatio="none"
            aria-hidden
          >
            {GRID_DB.map((db) => (
              <line
                key={db}
                className={db === 0 ? "eq__grid eq__grid--zero" : "eq__grid"}
                x1={0}
                x2={PLOT_WIDTH}
                y1={(dbToTopPercent(db) / 100) * PLOT_HEIGHT}
                y2={(dbToTopPercent(db) / 100) * PLOT_HEIGHT}
              />
            ))}
            <path className="eq__area" d={area} />
            <path className="eq__line" d={path} />
          </svg>
          {state.bandsHz.map((hz, i) => (
            <EqSlider
              key={hz}
              hz={hz}
              value={settings.gainsDb[i] ?? 0}
              dimmed={!enabled}
              onChange={(db) => equalizer.setGain(i, db)}
              onNeighbor={(direction) => focusBand(i + direction)}
              sliderRef={(element) => {
                sliders.current[i] = element;
              }}
            />
          ))}
        </div>
        <div className="eq__axis" aria-hidden>
          {state.bandsHz.map((hz) => (
            <span key={hz} style={{ left: `${frequencyToRatio(hz) * 100}%` }}>
              {formatHz(hz)}
            </span>
          ))}
        </div>
      </div>
    </section>
  );
}
