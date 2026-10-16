import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  errorMessage,
  exportParametric,
  getParametric,
  importParametric,
  pickParametricFile,
  pickParametricSavePath,
  setParametric,
  setParametricCompare,
  type ParametricSettings,
  type ParametricState,
} from "../lib/backend";
import { DEFAULT_PARAMETRIC, parametricIsActive, previewParametricState } from "../lib/parametric";

export interface ParametricControls {
  /** Ekranda gösterilen ayar (dokunulduğu anda güncellenir). */
  settings: ParametricSettings;
  /** Eğriler ve koruma: çekirdeğin hesabı; sürüklerken anında yaklaşık önizleme. */
  state: ParametricState;
  /** Ekolayzer sesi şu an değiştiriyor mu? */
  active: boolean;
  /** Karşılaştırma (A/B) açık mı (ses ekolayzersiz, eşit yükseklikte). */
  comparing: boolean;
  busy: boolean;
  error: string | null;
  /** Ayarı değiştirir; çekirdeğe aynı anda tek istek gider, araya gelenlerden en sonuncusu. */
  update: (change: (current: ParametricSettings) => ParametricSettings) => void;
  setCompare: (compare: boolean) => void;
  /** Equalizer APO dosyası seçip yükler. */
  importFile: () => Promise<void>;
  /** Equalizer APO dosyasına kaydeder. */
  exportFile: () => Promise<void>;
  /** Düz beş banda döner. */
  reset: () => void;
}

const same = (a: ParametricSettings, b: ParametricSettings) =>
  JSON.stringify(a) === JSON.stringify(b);

/**
 * Elle parametrik ekolayzer. Bant sürüklenirken her değişiklik beklemeden ekrana yansır
 * (analog önizleme); çekirdeğe aynı anda tek istek gider, araya gelen değişikliklerden yalnızca
 * en sonuncusu gönderilir. Çekirdeğin cevabı gelince eğri sayısal tasarımın gerçek eğrisidir.
 */
export function useParametric(): ParametricControls {
  const [settings, setSettings] = useState<ParametricSettings>(DEFAULT_PARAMETRIC);
  const [core, setCore] = useState<ParametricState | null>(null);
  const [comparing, setComparing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const settingsRef = useRef(settings);
  const inFlight = useRef(false);
  const pending = useRef<ParametricSettings | null>(null);
  /** Kullanıcı açılıştaki yükleme bitmeden dokunduysa yüklenen ayar onu ezmesin. */
  const touched = useRef(false);

  /** Çekirdeğin cevabı; `canonical` ise ekrandaki ayar da onun (sınırlara çekilmiş) ayarı olur. */
  const adopt = useCallback((state: ParametricState, canonical: boolean) => {
    setCore(state);
    setComparing(state.comparing);
    if (canonical) {
      settingsRef.current = state.settings;
      setSettings(state.settings);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    getParametric()
      .then((loaded) => {
        if (cancelled || !loaded || touched.current) return;
        settingsRef.current = loaded.settings;
        setSettings(loaded.settings);
        setCore(loaded);
        setComparing(loaded.comparing);
      })
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, []);

  const send = useCallback(
    async (next: ParametricSettings) => {
      if (inFlight.current) {
        pending.current = next;
        return;
      }
      inFlight.current = true;
      let target: ParametricSettings | null = next;
      while (target) {
        try {
          const result = await setParametric(target);
          // Bu sırada yeni bir değişiklik geldiyse eski sonucu gösterme.
          if (result && !pending.current) adopt(result, true);
          setError(null);
        } catch (e) {
          setError(errorMessage(e));
        }
        target = pending.current;
        pending.current = null;
      }
      inFlight.current = false;
    },
    [adopt],
  );

  const update = useCallback(
    (change: (current: ParametricSettings) => ParametricSettings) => {
      touched.current = true;
      const next = change(settingsRef.current);
      settingsRef.current = next;
      setSettings(next);
      void send(next);
    },
    [send],
  );

  const setCompare = useCallback(
    (compare: boolean) => {
      setComparing(compare && parametricIsActive(settingsRef.current));
      setParametricCompare(compare)
        .then((result) => result && adopt(result, !inFlight.current && !pending.current))
        .catch((e) => setError(errorMessage(e)));
    },
    [adopt],
  );

  const importFile = useCallback(async () => {
    setBusy(true);
    try {
      const path = await pickParametricFile();
      if (path) {
        touched.current = true;
        const result = await importParametric(path);
        adopt(result, true);
      }
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }, [adopt]);

  const exportFile = useCallback(async () => {
    setBusy(true);
    try {
      const path = await pickParametricSavePath();
      if (path) await exportParametric(path);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }, []);

  const reset = useCallback(() => update(() => DEFAULT_PARAMETRIC), [update]);

  const state = useMemo(
    () =>
      core && same(core.settings, settings) ? core : previewParametricState(settings, comparing),
    [core, settings, comparing],
  );

  return {
    settings,
    state,
    active: parametricIsActive(settings),
    comparing: comparing && parametricIsActive(settings),
    busy,
    error,
    update,
    setCompare,
    importFile,
    exportFile,
    reset,
  };
}
