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
  setBitPerfect: (on: boolean) => Promise<void>;
}

/**
 * Çalma seçenekleri. Ayarlar kaydedilir; program her açılışta hatırlar.
 * - Ses yüksekliği eşitlemesi: bütün şarkılar aynı yükseklikte çalar, ekolayzer sesi
 *   kısmadan yükseltebilir.
 * - Bit-perfect: ses aygıta hiç değişmeden gider (özel mod); çalan şarkı kaldığı yerden sürer.
 */
export function usePlaybackOptions(): PlaybackOptionsControls {
  const [options, setOptions] = useState<PlaybackOptions>({ normalize: true, bitPerfect: false });
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

  const update = useCallback(
    async (change: Partial<PlaybackOptions>) => {
      const next = { ...options, ...change };
      setOptions(next); // düğme hemen tepki versin
      try {
        setOptions(await setPlaybackOptions(next));
        setError(null);
      } catch (e) {
        // Ayar kaydedildi ama uygulanamadı (ör. aygıt yeniden açılamadı): düğme yeni
        // durumda kalır, neden yazılır.
        setError(errorMessage(e));
      }
    },
    [options],
  );

  const setNormalize = useCallback((normalize: boolean) => update({ normalize }), [update]);
  const setBitPerfect = useCallback((bitPerfect: boolean) => update({ bitPerfect }), [update]);

  return { options, error, setNormalize, setBitPerfect };
}
