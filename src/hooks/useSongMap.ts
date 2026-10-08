import { useEffect, useState } from "react";
import { getSongMap, type SongMap } from "../lib/backend";

/** Şarkı haritası (analiz) hazır olana kadar bu aralıkla sorulur; en fazla 2 dakika. */
const POLL_MS = 1000;
const MAX_TRIES = 120;

/**
 * Çalan şarkının haritası (bölümler, droplar, enerji). Analiz bitince gelir: hazır
 * olana kadar ara ara sorulur. Şarkı değişince önceki şarkının haritası hemen bırakılır.
 */
export function useSongMap(trackPath: string | null): SongMap | null {
  const [loaded, setLoaded] = useState<{ path: string; map: SongMap } | null>(null);

  useEffect(() => {
    if (!trackPath) return;
    let cancelled = false;
    let tries = 0;
    let timer = 0;
    const ask = () => {
      getSongMap()
        .then((map) => {
          if (cancelled) return;
          if (map) setLoaded({ path: trackPath, map });
          else if (++tries < MAX_TRIES) timer = window.setTimeout(ask, POLL_MS);
        })
        .catch(() => {
          /* Analiz yoksa harita gösterilmez. */
        });
    };
    ask();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [trackPath]);

  return loaded && loaded.path === trackPath ? loaded.map : null;
}
