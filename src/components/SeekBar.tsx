import { useRef, useState, type PointerEvent } from "react";
import { formatTime, progress } from "../lib/format";
import { dropLabels } from "../lib/songMap";
import type { TimelineLayout } from "../lib/timeline";

export interface SeekBarProps {
  positionSecs: number;
  durationSecs: number | null;
  disabled: boolean;
  onSeek: (seconds: number) => void;
  /** Şarkı haritası: verilirse çubuk bölümleri, enerjiyi ve drop'ları gösterir. */
  layout?: TimelineLayout | null;
}

/** "DROP" yazıları arasında en az bu kadar (çubuk genişliğine oranla) boşluk kalır. */
const DROP_LABEL_GAP = 0.07;

/** İşaretçinin çubuk üzerindeki oranı (0..1). */
export function pointerRatio(clientX: number, left: number, width: number): number {
  if (width <= 0) return 0;
  return Math.min(1, Math.max(0, (clientX - left) / width));
}

/**
 * İlerleme çubuğu ve tutamacı. Şarkı haritası hazırsa çubuk, şarkının bölümlerini
 * (yükseklik enerji, renk bölüm teması) ve drop'larını gösterir; çalınan kısım parlak,
 * gelecek soluktur. Fareyle üstüne gelince o noktanın süresini gösterir; tıklayınca
 * oraya atlar; tutamaç (ya da çubuk) sürüklenince gidilecek süreyi gösterir ve
 * bırakınca atlar. Klavyede ←/→ (genel kısayol).
 */
export function SeekBar({
  positionSecs,
  durationSecs,
  disabled,
  onSeek,
  layout = null,
}: SeekBarProps) {
  const [dragRatio, setDragRatio] = useState<number | null>(null);
  const [hoverRatio, setHoverRatio] = useState<number | null>(null);
  // Çizim için state, olaylar için ref: çok hızlı bas-bırak'ta bırakma olayı
  // yeniden çizimden önce gelse bile sürükleme bilgisi kaybolmaz.
  const dragging = useRef(false);
  const seekable = !disabled && durationSecs !== null && durationSecs > 0;
  const ratio = dragRatio ?? progress(positionSecs, durationSecs);
  // İpucu sürüklerken gidilecek yeri, yoksa fare altındaki noktayı gösterir.
  const tipRatio = dragRatio ?? (seekable ? hoverRatio : null);

  const ratioOf = (event: PointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return pointerRatio(event.clientX, rect.left, rect.width);
  };

  return (
    <div
      className={`seekbar${layout ? " seekbar--map" : ""}${seekable ? " is-seekable" : ""}${dragRatio !== null ? " is-dragging" : ""}`}
      role="slider"
      tabIndex={seekable ? 0 : -1}
      aria-label="Şarkıda konum"
      aria-disabled={!seekable}
      aria-valuemin={0}
      aria-valuemax={Math.round(durationSecs ?? 0)}
      aria-valuenow={Math.round(positionSecs)}
      aria-valuetext={`${formatTime(positionSecs)} / ${formatTime(durationSecs ?? 0)}`}
      onPointerDown={(event) => {
        if (!seekable || event.button !== 0) return;
        event.currentTarget.setPointerCapture?.(event.pointerId);
        dragging.current = true;
        setDragRatio(ratioOf(event));
      }}
      onPointerMove={(event) => {
        if (dragging.current) setDragRatio(ratioOf(event));
        else if (seekable) setHoverRatio(ratioOf(event));
      }}
      onPointerLeave={() => setHoverRatio(null)}
      onPointerUp={(event) => {
        if (!dragging.current || !durationSecs) return;
        dragging.current = false;
        setDragRatio(null);
        onSeek(ratioOf(event) * durationSecs);
      }}
      onPointerCancel={() => {
        dragging.current = false;
        setDragRatio(null);
      }}
    >
      <div className="seekbar__track">
        {layout ? (
          <SongMapTrack layout={layout} ratio={ratio} />
        ) : (
          <div className="seekbar__line">
            <div className="seekbar__fill" style={{ transform: `scaleX(${ratio})` }} />
          </div>
        )}
        {seekable ? (
          <span className="seekbar__thumb" style={{ left: `${ratio * 100}%` }} aria-hidden />
        ) : null}
      </div>
      {tipRatio !== null && durationSecs ? (
        <span className="seekbar__tip" style={{ left: `${tipRatio * 100}%` }}>
          {formatTime(tipRatio * durationSecs)}
        </span>
      ) : null}
    </div>
  );
}

const percent = (ratio: number) => `${ratio * 100}%`;

/** Şarkı haritası şeridi: soluk tüm şarkı, üstünde çalınan kısma kadar kırpılmış parlak kopya. */
function SongMapTrack({ layout, ratio }: { layout: TimelineLayout; ratio: number }) {
  const sections = layout.sections.map((section, index) => (
    <span
      key={index}
      className={`songmap__section songmap__section--theme-${section.theme}`}
      style={{
        left: percent(section.x),
        width: `calc(${percent(section.width)} - 3px)`,
        height: percent(0.3 + 0.7 * section.energy),
      }}
    />
  ));
  const labels = dropLabels(layout.programDrops, DROP_LABEL_GAP);
  return (
    <div className="songmap" aria-hidden>
      <div className="songmap__layer songmap__layer--future">{sections}</div>
      <div
        className="songmap__layer songmap__layer--past"
        style={{ clipPath: `inset(0 ${percent(1 - ratio)} 0 0)` }}
      >
        {sections}
      </div>
      {layout.programDrops.map((x, index) => (
        <span key={index} className="songmap__drop" style={{ left: percent(x) }}>
          {labels[index] ? "DROP" : null}
        </span>
      ))}
    </div>
  );
}
