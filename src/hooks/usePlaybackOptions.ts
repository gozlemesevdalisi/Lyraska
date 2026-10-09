import { useCallback, useEffect, useState } from "react";
import {
  errorMessage,
  getPlaybackOptions,
  setPlaybackOptions,
  type PlaybackOptions,
} from "../lib/backend";

export interface PlaybackOptionsControls {
  options: PlaybackOptions;
  error: string | null;
  setNormalize: (on: boolean) => Promise<void>;
}

/**
 * Çalma seçenekleri. Ses yüksekliği eşitlemesi: bütün şarkılar aynı yükseklikte çalar,
 * ekolayzer sesi kısmadan yükseltebilir. Ayar kaydedilir; program her açılışta hatırlar.
 */
export function usePlaybackOptions(): PlaybackOptionsControls {
  const [options, setOptions] = useState<PlaybackOptions>({ normalize: true });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getPlaybackOptions()
      .then((value) => !cancelled && setOptions(value))
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, []);

  const setNormalize = useCallback(
    async (normalize: boolean) => {
      const next = { ...options, normalize };
      setOptions(next); // düğme hemen tepki versin
      try {
        setOptions(await setPlaybackOptions(next));
        setError(null);
      } catch (e) {
        setError(errorMessage(e));
      }
    },
    [options],
  );

  return { options, error, setNormalize };
}
