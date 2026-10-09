import { SeekBar } from "./SeekBar";
import { EjectIcon, NextIcon, PauseIcon, PlayIcon, PreviousIcon, StopIcon } from "./icons";
import { formatTime } from "../lib/format";
import type { TimelineLayout } from "../lib/timeline";
import type { PlayerControls } from "../hooks/usePlayer";

export interface DockProps {
  player: PlayerControls;
  /** Şarkı haritası (analiz bitmediyse `null`: düz çubuk). */
  layout: TimelineLayout | null;
  canNext: boolean;
  onPrevious: () => void;
  onNext: () => void;
}

/** Alttaki cam: şarkı haritası şeridi, süre ve çalma düğmeleri. */
export function Dock({ player, layout, canNext, onPrevious, onNext }: DockProps) {
  const { status, position, available } = player;
  const hasTrack = status.track !== null;
  const playing = status.state === "playing";
  const durationSecs = status.track?.durationSecs ?? null;

  return (
    <footer className="dock chrome">
      <SeekBar
        positionSecs={position}
        durationSecs={durationSecs}
        disabled={!available}
        onSeek={(seconds) => void player.seek(seconds)}
        layout={layout}
      />
      <div className="dock__row">
        <time className="dock__time">{hasTrack ? formatTime(position) : "--:--"}</time>
        <div className="transport">
          <button
            type="button"
            className="round-button"
            onClick={() => void player.stop()}
            disabled={!available || !hasTrack}
            title="Durdur ve başa dön"
            aria-label="Durdur"
          >
            <StopIcon />
          </button>
          <button
            type="button"
            className="round-button round-button--large"
            onClick={onPrevious}
            disabled={!available || !hasTrack}
            title="Önceki şarkı (şarkının ortasındaysa başa sarar)"
            aria-label="Önceki"
          >
            <PreviousIcon />
          </button>
          <button
            type="button"
            className="play-button"
            onClick={() => void player.toggle()}
            disabled={!available || !hasTrack}
            title="Çal / Duraklat (Boşluk)"
            aria-label={playing ? "Duraklat" : "Çal"}
          >
            {playing ? <PauseIcon /> : <PlayIcon />}
          </button>
          <button
            type="button"
            className="round-button round-button--large"
            onClick={onNext}
            disabled={!available || !canNext}
            title="Sonraki şarkı"
            aria-label="Sonraki"
          >
            <NextIcon />
          </button>
          <button
            type="button"
            className="round-button"
            onClick={() => void player.openFile()}
            disabled={!available}
            title="Dosya aç (Ctrl+O)"
            aria-label="Dosya aç"
          >
            <EjectIcon />
          </button>
        </div>
        <time className="dock__time dock__time--end">
          {hasTrack ? formatTime(durationSecs ?? 0) : "--:--"}
        </time>
      </div>
    </footer>
  );
}
