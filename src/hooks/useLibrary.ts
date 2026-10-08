import { useCallback, useEffect, useRef, useState } from "react";
import {
  EMPTY_LIBRARY,
  addLibraryFolder,
  errorMessage,
  getLibraryStatus,
  isDesktop,
  pickAudioFiles,
  pickFolder,
  removeLibraryFolder,
  rescanLibrary,
  searchLibrary,
  type LibraryStatus,
  type LibraryTrack,
} from "../lib/backend";
import { analysisPending, analysisRefreshKey } from "../lib/analysis";

/** Tarama sürerken durum sorgulama aralığı. */
const SCAN_POLL_MS = 600;
/** Arka plan analizi sürerken durum sorgulama aralığı (BPM sütunu dolsun). */
const ANALYSIS_POLL_MS = 3000;
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
  /** Şarkı seçme penceresini açar; seçilen şarkıları tek tek kütüphaneye ekler. */
  addFiles: (extensions: string[]) => Promise<void>;
  /** Yolu bilinen klasörü ekler (ör. pencereye sürüklenen). */
  addFolderPath: (path: string) => Promise<void>;
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

  // Analiz ilerledikçe BPM sütunu dolsun diye liste ara ara tazelenir.
  const analysisKey = analysisRefreshKey(status.analyzed, status.trackCount);

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
  }, [available, query, status.trackCount, analysisKey]);

  // Tarama sürerken ilerlemeyi izle.
  const scanning = status.scan.scanning;
  useEffect(() => {
    if (!available || !scanning) return;
    const timer = window.setInterval(() => void refreshStatus(), SCAN_POLL_MS);
    return () => window.clearInterval(timer);
  }, [available, scanning, refreshStatus]);

  // Arka plan analizi sürerken ilerlemeyi izle (tarama yokken, daha seyrek).
  const analyzing = analysisPending(status.analyzed, status.trackCount);
  useEffect(() => {
    if (!available || scanning || !analyzing) return;
    const timer = window.setInterval(() => void refreshStatus(), ANALYSIS_POLL_MS);
    return () => window.clearInterval(timer);
  }, [available, scanning, analyzing, refreshStatus]);

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

  const addFolderPath = useCallback((path: string) => apply(() => addLibraryFolder(path)), [apply]);

  const addFiles = useCallback(
    async (extensions: string[]) => {
      if (!available) return;
      let paths: string[];
      try {
        paths = await pickAudioFiles(extensions);
      } catch (e) {
        setError(errorMessage(e));
        return;
      }
      // Biri eklenemese de (ör. zaten kütüphanede) diğerleri eklenir; son sorun gösterilir.
      let problem: string | null = null;
      for (const path of paths) {
        try {
          setStatus(await addLibraryFolder(path));
        } catch (e) {
          problem = errorMessage(e);
        }
      }
      if (paths.length > 0) setError(problem);
    },
    [available],
  );

  const removeFolder = useCallback((id: number) => apply(() => removeLibraryFolder(id)), [apply]);
  const rescan = useCallback(() => apply(rescanLibrary), [apply]);

  return {
    status,
    tracks,
    query,
    setQuery,
    loading,
    error,
    addFolder,
    addFiles,
    addFolderPath,
    removeFolder,
    rescan,
  };
}
