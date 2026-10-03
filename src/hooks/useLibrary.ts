import { useCallback, useEffect, useRef, useState } from "react";
import {
  EMPTY_LIBRARY,
  addLibraryFolder,
  errorMessage,
  getLibraryStatus,
  isDesktop,
  pickFolder,
  removeLibraryFolder,
  rescanLibrary,
  searchLibrary,
  type LibraryStatus,
  type LibraryTrack,
} from "../lib/backend";

/** Tarama sürerken durum sorgulama aralığı. */
const SCAN_POLL_MS = 600;
/** Yazmayı bitirmeyi beklemeden aramaya başlamak için kısa gecikme. */
const SEARCH_DELAY_MS = 120;

export interface LibraryControls {
  status: LibraryStatus;
  tracks: LibraryTrack[];
  query: string;
  setQuery: (query: string) => void;
  /** İlk yükleme sürüyor. */
  loading: boolean;
  error: string | null;
  addFolder: () => Promise<void>;
  removeFolder: (id: number) => Promise<void>;
  rescan: () => Promise<void>;
}

/**
 * Kütüphane durumu ve arama. Tarama sürerken durumu düzenli sorar ve şarkı
 * sayısı değiştikçe listeyi tazeler.
 */
export function useLibrary(): LibraryControls {
  const available = isDesktop();
  const [status, setStatus] = useState<LibraryStatus>(EMPTY_LIBRARY);
  const [tracks, setTracks] = useState<LibraryTrack[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(available);
  const [error, setError] = useState<string | null>(null);
  const searchSeq = useRef(0);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await getLibraryStatus());
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  // Açılışta durumu al.
  useEffect(() => {
    if (!available) return;
    getLibraryStatus()
      .then(setStatus)
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, [available]);

  // Arama: yazı ya da şarkı sayısı değişince (tarama ilerledikçe) listeyi tazele.
  // Yalnızca en son aramanın sonucu uygulanır.
  useEffect(() => {
    if (!available) return;
    const seq = ++searchSeq.current;
    const timer = window.setTimeout(() => {
      searchLibrary(query)
        .then((result) => {
          if (seq === searchSeq.current) setTracks(result);
        })
        .catch((e) => setError(errorMessage(e)));
    }, SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [available, query, status.trackCount]);

  // Tarama sürerken ilerlemeyi izle.
  const scanning = status.scan.scanning;
  useEffect(() => {
    if (!available || !scanning) return;
    const timer = window.setInterval(() => void refreshStatus(), SCAN_POLL_MS);
    return () => window.clearInterval(timer);
  }, [available, scanning, refreshStatus]);

  const apply = useCallback(async (action: () => Promise<LibraryStatus>) => {
    try {
      setStatus(await action());
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  const addFolder = useCallback(async () => {
    if (!available) return;
    try {
      const path = await pickFolder();
      if (path) await apply(() => addLibraryFolder(path));
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [available, apply]);

  const removeFolder = useCallback((id: number) => apply(() => removeLibraryFolder(id)), [apply]);
  const rescan = useCallback(() => apply(rescanLibrary), [apply]);

  return { status, tracks, query, setQuery, loading, error, addFolder, removeFolder, rescan };
}
