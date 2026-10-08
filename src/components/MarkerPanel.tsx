import type { MarkerControls } from "../hooks/useMarker";
import { openAnnotationFolder } from "../lib/backend";
import { formatTime } from "../lib/format";
import { tapBpm } from "../lib/marks";
import { timelineLayout } from "../lib/timeline";

export interface MarkerPanelProps {
  marker: MarkerControls;
  /** Çalan şarkının adı; şarkı yoksa `null`. */
  trackTitle: string | null;
  playing: boolean;
  /** Şarkının süresi (şarkı haritası şeridi için). */
  durationSecs: number | null;
}

/** Şarkı haritası şeridi: bölümler (enerjiye göre koyuluk), programın ve sizin droplarınız. */
function SongTimeline({ marker, durationSecs }: { marker: MarkerControls; durationSecs: number }) {
  const map = marker.songMap;
  const layout = timelineLayout({
    durationSecs,
    sections: map?.sections ?? [],
    programDrops: map?.drops ?? [],
    markedDrops: marker.marks.drops,
  });
  if (!layout) return null;
  const W = 1000;
  return (
    <figure className="timeline">
      <svg
        viewBox={`0 0 ${W} 44`}
        preserveAspectRatio="none"
        role="img"
        aria-label={`Şarkı haritası: ${map ? `${map.sections.length} bölüm, programın bulduğu ${map.drops.length} drop` : "analiz sürüyor"}, sizin ${marker.marks.drops.length} drop işaretiniz`}
      >
        {layout.sections.map((s, i) => (
          <rect
            key={i}
            className="timeline__section"
            x={s.x * W + 1}
            y={12}
            width={Math.max(0, s.width * W - 2)}
            height={20}
            rx={3}
            style={{ opacity: 0.15 + 0.6 * s.energy }}
          />
        ))}
        {layout.programDrops.map((x, i) => (
          <polygon
            key={`p${i}`}
            className="timeline__drop"
            points={`${x * W - 6},0 ${x * W + 6},0 ${x * W},10`}
          />
        ))}
        {layout.markedDrops.map((x, i) => (
          <polygon
            key={`m${i}`}
            className="timeline__mark"
            points={`${x * W - 6},44 ${x * W + 6},44 ${x * W},34`}
          />
        ))}
      </svg>
      <figcaption className="timeline__legend">
        {map
          ? `Şarkı haritası: ${map.sections.length} bölüm · ${map.meter}/4 ölçü · ▼ programın dropları · ▲ sizin droplarınız`
          : "Şarkı haritası: analiz sürüyor… · ▲ sizin droplarınız"}
      </figcaption>
    </figure>
  );
}

const DECIMAL = new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 1 });
const SECONDS = new Intl.NumberFormat("tr-TR", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

/** Yüzde biçimi: 0,873 → "%87,3". */
function percent(x: number): string {
  return `%${DECIMAL.format(x * 100)}`;
}

function bpmText(bpm: number | null): string {
  return bpm === null ? "—" : DECIMAL.format(bpm);
}

/**
 * İşaretleme aracı: şarkı çalarken Boşluk ile beat'lere, D ile droplara vurulur.
 * Sonuç şarkı başına bir dosyaya kendiliğinden kaydedilir (ses içermez) ve
 * programın analiziyle karşılaştırılabilir.
 */
export function MarkerPanel({ marker, trackTitle, playing, durationSecs }: MarkerPanelProps) {
  const { marks, recording, evaluation } = marker;
  const bpm = tapBpm(marks.beats);
  const lastBeats = marks.beats.slice(-6);

  return (
    <section className="marker" aria-label="İşaretleme">
      <header className="library__header marker__header">
        <h2 className="library__title">İşaretle</h2>
        <button
          type="button"
          className={`hw-button${recording ? " is-recording" : ""}`}
          aria-pressed={recording}
          disabled={!trackTitle}
          onClick={() => marker.setRecording(!recording)}
        >
          <span className={`marker__rec${recording ? " is-on" : ""}`} aria-hidden />
          {recording ? "İşaretlemeyi bitir" : "İşaretlemeye başla"}
        </button>
        <span className="marker__track">{trackTitle ?? "Önce bir şarkı açın"}</span>
      </header>

      <div className="marker__body">
        <div className="marker__help">
          {recording ? (
            <p>
              {playing ? "Şarkı çalıyor. " : "Şarkıyı başlatın. "}
              <kbd>Boşluk</kbd> her vuruşta (beat) · <kbd>D</kbd> drop anında · <kbd>Geri</kbd> son
              işareti sil · <kbd>Esc</kbd> bitir
            </p>
          ) : (
            <p>
              Şarkıyı çalarken ayağınızla tempo tutar gibi her vuruşta <kbd>Boşluk</kbd> tuşuna,
              şarkının patladığı (drop) anlarda <kbd>D</kbd> tuşuna basın. İşaretler kendiliğinden
              kaydedilir; ses dosyası kaydedilmez.
            </p>
          )}
        </div>

        <div className="marker__counts" aria-live="polite">
          <div className="marker__stat">
            <span
              key={marker.lastTap?.kind === "beat" ? marker.lastTap.id : "beat"}
              className={`marker__lamp${marker.lastTap?.kind === "beat" ? " is-flash" : ""}`}
              aria-hidden
            />
            <strong>{marks.beats.length}</strong> beat
            {bpm !== null && <span className="marker__bpm"> · ~{bpmText(bpm)} BPM</span>}
          </div>
          <div className="marker__stat">
            <span
              key={marker.lastTap?.kind === "drop" ? marker.lastTap.id : "drop"}
              className={`marker__lamp marker__lamp--drop${
                marker.lastTap?.kind === "drop" ? " is-flash" : ""
              }`}
              aria-hidden
            />
            <strong>{marks.drops.length}</strong> drop
            {marks.drops.length > 0 && (
              <span className="marker__bpm">
                {" "}
                · {marks.drops.map((t) => formatTime(t)).join(", ")}
              </span>
            )}
          </div>
          {lastBeats.length > 0 && (
            <div className="marker__recent">
              Son vuruşlar: {lastBeats.map((t) => SECONDS.format(t)).join(" · ")}
            </div>
          )}
        </div>

        {durationSecs !== null && <SongTimeline marker={marker} durationSecs={durationSecs} />}

        <div className="marker__actions">
          <button
            type="button"
            className="chip chip--button"
            disabled={marker.marks.history.length === 0}
            onClick={marker.undo}
          >
            Son işareti geri al
          </button>
          <button
            type="button"
            className="chip chip--button"
            disabled={marks.beats.length + marks.drops.length === 0}
            onClick={marker.clear}
          >
            Hepsini sil
          </button>
          <button
            type="button"
            className="chip chip--button"
            disabled={marks.beats.length < 8 || marker.savedFile === null || marker.evaluating}
            title="Programın bulduğu vuruşları sizin işaretlerinizle karşılaştırır (birkaç saniye sürer)."
            onClick={() => void marker.evaluate()}
          >
            {marker.evaluating ? "Ölçülüyor…" : "Doğruluğu ölç"}
          </button>
          <button
            type="button"
            className="chip chip--button"
            onClick={() => void openAnnotationFolder().catch(() => {})}
          >
            İşaret klasörünü aç
          </button>
          <span className="marker__saved">
            {marker.saving
              ? "Kaydediliyor…"
              : marker.savedFile
                ? "Kaydedildi"
                : marks.history.length > 0
                  ? "Değişiklik var"
                  : ""}
          </span>
        </div>

        {evaluation && (
          <dl className="marker__result" aria-label="Doğruluk sonucu">
            <div>
              <dt>Beat doğruluğu (F-ölçüsü)</dt>
              <dd>{percent(evaluation.fMeasure)}</dd>
            </div>
            <div>
              <dt>Parmak gecikmesi</dt>
              <dd>{Math.round(evaluation.tapOffsetMs)} ms</dd>
            </div>
            <div>
              <dt>Gecikme düzeltilince</dt>
              <dd>{percent(evaluation.fMeasureAligned)}</dd>
            </div>
            {evaluation.markedDrops > 0 && (
              <div>
                <dt>Drop isabeti</dt>
                <dd>
                  {evaluation.dropHits} / {evaluation.markedDrops}
                </dd>
              </div>
            )}
            <div>
              <dt>Tempo (program / siz)</dt>
              <dd>
                {bpmText(evaluation.detectedBpm)} / {bpmText(evaluation.markedBpm)} BPM
              </dd>
            </div>
          </dl>
        )}

        {marker.error && <p className="library__notice library__problem">{marker.error}</p>}
      </div>
    </section>
  );
}
