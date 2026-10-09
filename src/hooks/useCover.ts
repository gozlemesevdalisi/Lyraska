import { useEffect, useState } from "react";
import { getTrackCover } from "../lib/backend";

/** Son kapaklar akılda tutulur: aynı şarkı (ör. sıradaki → çalan) için dosya yeniden okunmaz. */
const CACHE_LIMIT = 24;
const cache = new Map<string, string | null>();

function remember(path: string, url: string | null) {
  cache.delete(path);
  cache.set(path, url);
  if (cache.size > CACHE_LIMIT) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
}

/** Testler arasında önbelleği temizler. */
export function clearCoverCache() {
  cache.clear();
}

/**
 * Şarkının içindeki kapak resmi (`data:` adresi). Kapak yoksa, okunamazsa ya da şarkı
 * yoksa `null` (o zaman özgün renk kapağı gösterilir). Şarkı değişince önceki şarkının
 * kapağı hemen bırakılır.
 */
export function useCover(trackPath: string | null): string | null {
  const [loaded, setLoaded] = useState<{ path: string; url: string | null } | null>(null);

  useEffect(() => {
    if (!trackPath || cache.has(trackPath)) return;
    let cancelled = false;
    getTrackCover(trackPath)
      .catch(() => null)
      .then((url) => {
        remember(trackPath, url);
        if (!cancelled) setLoaded({ path: trackPath, url });
      });
    return () => {
      cancelled = true;
    };
  }, [trackPath]);

  if (!trackPath) return null;
  if (cache.has(trackPath)) return cache.get(trackPath) ?? null;
  return loaded && loaded.path === trackPath ? loaded.url : null;
}
