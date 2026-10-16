import { useEffect, useState } from "react";
import { getSongKey, type KeyMap } from "../lib/backend";

/** Ton analizi hazır olana kadar bu aralıkla sorulur; en fazla 2 dakika. */
const POLL_MS = 1000;
const MAX_TRIES = 120;

/**
 * Çalan şarkının tonu ve tonu farklı bölümleri. Analiz bitince gelir: hazır olana kadar ara
 * ara sorulur. Şarkı değişince önceki şarkının tonu hemen bırakılır. Şarkı haritası ton
 * analiziyle birlikte bitmeyebilir: harita gelince bölümlerin tonları için bir kez daha sorulur.
 */
export function useSongKey(trackPath: string | null, sectionCount: number): KeyMap | null {
  const [loaded, setLoaded] = useState<{ path: string; map: KeyMap } | null>(null);

  useEffect(() => {
    if (!trackPath) return;
    let cancelled = false;
    let tries = 0;
    let timer = 0;
    const ask = () => {
      getSongKey()
        .then((map) => {
          if (cancelled) return;
          if (map) setLoaded({ path: trackPath, map });
          else if (++tries < MAX_TRIES) timer = window.setTimeout(ask, POLL_MS);
        })
        .catch(() => {
          /* Ton yoksa gösterilmez. */
        });
    };
    ask();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [trackPath, sectionCount]);

  return loaded && loaded.path === trackPath ? loaded.map : null;
}
