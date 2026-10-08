import { useCallback, useRef } from "react";
import type { SyncControls } from "../hooks/useSync";
import { useVisualFeed } from "../hooks/useVisualFeed";
import type { VisualFrame } from "../lib/backend";
import { MAX_AUDIO_DELAY_MS, MIN_AUDIO_DELAY_MS, beatSwing } from "../lib/sync";

export interface SyncPanelProps {
  sync: SyncControls;
  playing: boolean;
  /** Ses çalınabilir mi (tarayıcı önizlemesinde hayır). */
  available: boolean;
}

/**
 * Vuruş göstergesi: çalan şarkının her vuruşunda nokta bir kenara değer. Ses
 * gecikmesi doğru ayarlıysa nokta tam vuruşu duyduğunuz anda kenara değer.
 * Yalnızca hareket eder, parlamaz.
 */
function BeatIndicator({ playing }: { playing: boolean }) {
  const dot = useRef<HTMLSpanElement>(null);
  const onTick = useCallback((frame: VisualFrame | null) => {
    const el = dot.current;
    if (!el) return;
    const beat = frame?.beat;
    el.dataset.active = beat ? "on" : "off";
    const x = beat ? beatSwing(beat.index, beat.phase) : 0.5;
    el.style.left = `${(x * 100).toFixed(2)}%`;
  }, []);
  useVisualFeed(playing, onTick);
  return (
    <div className="sync__track" role="img" aria-label="Vuruş göstergesi">
      <span className="sync__edge sync__edge--left" aria-hidden />
      <span ref={dot} className="sync__dot" data-active="off" aria-hidden />
      <span className="sync__edge sync__edge--right" aria-hidden />
    </div>
  );
}

/** Ses–görüntü senkronu: ses gecikmesi ayarı, tıklamayla ölçüm ve vuruş göstergesi. */
export function SyncPanel({ sync, playing, available }: SyncPanelProps) {
  const { delayMs, calibration } = sync;
  const listening = calibration.phase === "listening";

  return (
    <section className="marker sync" aria-label="Senkron">
      <header className="library__header marker__header">
        <h2 className="library__title">Senkron</h2>
        <span className="marker__track">
          Görüntü vuruşa göre geride ya da ileride kalıyorsa buradan düzeltin.
        </span>
      </header>

      <div className="marker__body">
        <div className="marker__help">
          <p>
            Bazı kulaklık ve hoparlörler (özellikle <strong>Bluetooth</strong>) sesi biraz geç
            çalar; görseller o zaman vuruştan önce gelir. <strong>Ses gecikmesi</strong> bunu
            düzeltir: Bluetooth'ta genelde 150–250 ms, kablolu kulaklıkta 0 ms.
          </p>
        </div>

        <div className="sync__setting">
          <label htmlFor="sync-delay">Ses gecikmesi</label>
          <input
            id="sync-delay"
            type="range"
            min={MIN_AUDIO_DELAY_MS}
            max={MAX_AUDIO_DELAY_MS}
            step={5}
            value={delayMs}
            onChange={(e) => sync.setDelay(Number(e.currentTarget.value))}
            aria-valuetext={`${delayMs} milisaniye`}
          />
          <output htmlFor="sync-delay" className="sync__value">
            {delayMs} ms
          </output>
          <button
            type="button"
            className="chip chip--button"
            onClick={() => sync.setDelay(delayMs - 10)}
            aria-label="10 ms azalt"
          >
            −10
          </button>
          <button
            type="button"
            className="chip chip--button"
            onClick={() => sync.setDelay(delayMs + 10)}
            aria-label="10 ms artır"
          >
            +10
          </button>
          <button
            type="button"
            className="chip chip--button"
            disabled={delayMs === 0}
            onClick={() => sync.setDelay(0)}
          >
            Sıfırla
          </button>
        </div>

        <div className="sync__check">
          <p className="sync__caption">
            Bir şarkı çalarken nokta her vuruşta bir kenara değmeli. Kenara vuruştan önce değiyorsa
            gecikmeyi artırın, sonra değiyorsa azaltın.
          </p>
          <BeatIndicator playing={playing} />
        </div>

        <div className="marker__help sync__measure" aria-live="polite">
          {listening ? (
            <p>
              Tıklamaları dinleyin ve her birini duyduğunuz anda <kbd>Boşluk</kbd> tuşuna basın (
              {calibration.taps.length} / {calibration.clicks.length}).
            </p>
          ) : calibration.phase === "done" ? (
            <p>
              Ölçüldü: ses yaklaşık <strong>{calibration.estimate.delayMs} ms</strong> geç
              duyuluyor; ayar buna göre yapıldı ({calibration.estimate.used} vuruş).
              {calibration.estimate.reliable
                ? " Nokta artık vuruşla birlikte olmalı."
                : " Basışlar biraz dağınıktı; isterseniz ölçümü tekrarlayın."}
            </p>
          ) : calibration.phase === "failed" ? (
            <p className="library__problem">{calibration.message}</p>
          ) : (
            <p>
              <strong>Otomatik ölçüm:</strong> program yaklaşık 15 saniyelik kısık bir tıklama kaydı
              çalar (çalan şarkı durur). Her tıklamada <kbd>Boşluk</kbd>'a basarsınız, gecikme
              kendiliğinden ayarlanır.
            </p>
          )}
        </div>

        <div className="marker__actions">
          {listening ? (
            <button
              type="button"
              className="hw-button is-recording"
              onClick={sync.finishCalibration}
            >
              <span className="marker__rec is-on" aria-hidden />
              Ölçümü bitir
            </button>
          ) : (
            <button
              type="button"
              className="hw-button"
              disabled={!available}
              onClick={() => void sync.startCalibration()}
            >
              {calibration.phase === "idle" ? "Ölçümü başlat" : "Yeniden ölç"}
            </button>
          )}
          {calibration.phase === "done" && (
            <button type="button" className="chip chip--button" onClick={sync.undoCalibration}>
              Eski ayara dön ({calibration.previousMs} ms)
            </button>
          )}
        </div>

        {sync.error && <p className="library__notice library__problem">{sync.error}</p>}
      </div>
    </section>
  );
}
