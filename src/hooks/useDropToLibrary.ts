import { useCallback, useEffect, useState } from "react";
import { errorMessage, logFrontendError, onDragDrop, type DropOutcome } from "../lib/backend";
import { DROP_NOTICE_MS, DROP_PROBLEM_MS, dropSummary, type DropNotice } from "../lib/drop";
import { describeError } from "../lib/errorReporting";

export interface DropControls {
  /** Dosyalar pencerenin üstünde sürükleniyor. */
  dragging: boolean;
  /** Son bırakmanın sonucu (kısa süre gösterilir). */
  notice: DropNotice | null;
}

/**
 * Pencereye bırakılanlar: şarkılar ve klasörler kütüphaneye eklenir; bırakılan şarkılar
 * `onTracks` ile çalınır. Dosya mı klasör mü olduğuna çekirdek diskte bakar. Sonuç
 * (sorunlar dahil) hangi panel açık olursa olsun kısa bir bildirimle gösterilir.
 */
export function useDropToLibrary(
  addDropped: (paths: string[]) => Promise<DropOutcome>,
  onTracks: (tracks: string[]) => void,
): DropControls {
  const [dragging, setDragging] = useState(false);
  const [notice, setNotice] = useState<DropNotice | null>(null);

  const handleDropped = useCallback(
    async (paths: string[]) => {
      let next: DropNotice;
      try {
        const outcome = await addDropped(paths);
        if (outcome.tracks.length > 0) onTracks(outcome.tracks);
        next = dropSummary(outcome);
      } catch (e) {
        next = { text: errorMessage(e), problem: true };
      }
      setNotice(next);
    },
    [addDropped, onTracks],
  );

  useEffect(() => {
    if (!notice) return;
    const ms = notice.problem ? DROP_PROBLEM_MS : DROP_NOTICE_MS;
    const timer = window.setTimeout(() => setNotice(null), ms);
    return () => window.clearTimeout(timer);
  }, [notice]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    onDragDrop({ onHover: setDragging, onDrop: (paths) => void handleDropped(paths) })
      .then((fn) => (cancelled ? fn() : (unlisten = fn)))
      .catch((e) => {
        // Dinlenemezse bırakmalar sessizce kaybolurdu: nedeni hata günlüğüne yazılır.
        logFrontendError(`Sürükle-bırak dinlenemedi: ${describeError(e)}`).catch(() => {
          /* Günlüğe yazılamazsa yapılacak bir şey yok. */
        });
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [handleDropped]);

  return { dragging, notice };
}
