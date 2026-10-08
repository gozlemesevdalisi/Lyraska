import type { DropNotice } from "../lib/drop";

interface DropOverlayProps {
  /** Dosyalar pencerenin üstünde sürükleniyor. */
  dragging: boolean;
  /** Son bırakmanın sonucu (kısa süre gösterilir). */
  notice: DropNotice | null;
}

/**
 * Sürüklerken pencereyi çerçeveleyen "bırakın" işareti ve bırakınca sonucu bildiren
 * satır. Hangi sekme açık olursa olsun görünür: bırakılan bir şey sessizce kaybolmaz.
 */
export function DropOverlay({ dragging, notice }: DropOverlayProps) {
  return (
    <>
      <div className={`drop-zone${dragging ? " is-active" : ""}`} aria-hidden={!dragging}>
        <p className="drop-zone__label">
          Bırakın: şarkılar kütüphaneye eklenip çalınır, klasörler kütüphaneye eklenir
        </p>
      </div>
      {/* Canlı bölge baştan vardır; ekran okuyucu yeni bildirimi okur. */}
      <div className="drop-notice" role="status" aria-live="polite">
        {notice && (
          <p className={`drop-notice__text${notice.problem ? " is-problem" : ""}`}>{notice.text}</p>
        )}
      </div>
    </>
  );
}
