import { useCallback, useEffect, useRef, useState } from "react";
import {
  errorMessage,
  getEqualizer,
  setEqualizer,
  type EqSettings,
  type EqState,
} from "../lib/backend";
import {
  EQ_BANDS_HZ,
  previewEqState,
  snapBass,
  snapDepth,
  snapPunch,
  snapGain,
  type EqPreset,
} from "../lib/eq";

export interface EqualizerControls {
  /** Sürgülerin gösterdiği ayar (dokunulduğu anda güncellenir). */
  settings: EqSettings;
  /** Çekirdeğin hesapladığı eğri ve ön kazanç. */
  state: EqState;
  /** Ekolayzer sesi şu an değiştiriyor mu? (Ekrandaki "EQ" ışığı.) */
  active: boolean;
  error: string | null;
  setGain: (band: number, db: number) => void;
  setEnabled: (enabled: boolean) => void;
  /** Bas düğmesi (0–18 dB; 12 dB üstü kulüp bölgesi). */
  setBass: (db: number) => void;
  /** Küçük hoparlör bası. */
  setSmallSpeaker: (on: boolean) => void;
  /** Derinlik (0–1): alt oktav. */
  setDepth: (depth: number) => void;
  /** Vuruş (0–1): davul vuruşlarının ilk anı. */
  setPunch: (punch: number) => void;
  /** Hazır ayarı (bantlar ve bas) uygular ve ekolayzeri açar. */
  applyPreset: (preset: EqPreset) => void;
}

const INITIAL: EqSettings = {
  enabled: true,
  gainsDb: EQ_BANDS_HZ.map(() => 0),
  bassDb: 0,
  smallSpeaker: false,
  bassDepth: 0,
  bassPunch: 0,
};

/**
 * Ekolayzer ayarları. Sürgü sürüklenirken her değişiklik beklemeden ekrana
 * yansır; çekirdeğe aynı anda tek istek gider, araya gelen değişikliklerden
 * yalnızca en sonuncusu gönderilir (hiçbiri kaybolmaz, istekler birikmez).
 */
export function useEqualizer(): EqualizerControls {
  const [settings, setSettings] = useState<EqSettings>(INITIAL);
  const [state, setState] = useState<EqState>(() => previewEqState(INITIAL));
  const [error, setError] = useState<string | null>(null);
  const settingsRef = useRef(settings);
  const inFlight = useRef(false);
  const pending = useRef<EqSettings | null>(null);
  /** Kullanıcı açılıştaki yükleme bitmeden dokunduysa yüklenen ayar onu ezmesin. */
  const touched = useRef(false);

  useEffect(() => {
    let cancelled = false;
    getEqualizer()
      .then((loaded) => {
        if (cancelled || touched.current) return;
        const next = {
          enabled: loaded.enabled,
          gainsDb: loaded.gainsDb,
          bassDb: loaded.bassDb,
          smallSpeaker: loaded.smallSpeaker,
          bassDepth: loaded.bassDepth,
          bassPunch: loaded.bassPunch,
        };
        settingsRef.current = next;
        setSettings(next);
        setState(loaded);
      })
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, []);

  const send = useCallback(async (next: EqSettings) => {
    if (inFlight.current) {
      pending.current = next;
      return;
    }
    inFlight.current = true;
    let target: EqSettings | null = next;
    while (target) {
      try {
        const result = await setEqualizer(target);
        // Bu sırada yeni bir değişiklik geldiyse eski sonucu gösterme.
        if (!pending.current) setState(result);
        setError(null);
      } catch (e) {
        setError(errorMessage(e));
      }
      target = pending.current;
      pending.current = null;
    }
    inFlight.current = false;
  }, []);

  const update = useCallback(
    (change: (current: EqSettings) => EqSettings) => {
      touched.current = true;
      const next = change(settingsRef.current);
      settingsRef.current = next;
      setSettings(next);
      void send(next);
    },
    [send],
  );

  const setGain = useCallback(
    (band: number, db: number) =>
      update((current) => ({
        ...current,
        gainsDb: current.gainsDb.map((g, i) => (i === band ? snapGain(db) : g)),
      })),
    [update],
  );

  const setEnabled = useCallback(
    (enabled: boolean) => update((current) => ({ ...current, enabled })),
    [update],
  );

  const setBass = useCallback(
    (db: number) => update((current) => ({ ...current, bassDb: snapBass(db) })),
    [update],
  );

  const setSmallSpeaker = useCallback(
    (smallSpeaker: boolean) => update((current) => ({ ...current, smallSpeaker })),
    [update],
  );

  const setDepth = useCallback(
    (depth: number) => update((current) => ({ ...current, bassDepth: snapDepth(depth) })),
    [update],
  );

  const setPunch = useCallback(
    (punch: number) => update((current) => ({ ...current, bassPunch: snapPunch(punch) })),
    [update],
  );

  const applyPreset = useCallback(
    (preset: EqPreset) =>
      update(() => ({
        enabled: true,
        gainsDb: EQ_BANDS_HZ.map((_, i) => snapGain(preset.gains[i] ?? 0)),
        bassDb: snapBass(preset.bassDb),
        smallSpeaker: preset.smallSpeaker,
        bassDepth: snapDepth(preset.bassDepth),
        bassPunch: snapPunch(preset.bassPunch),
      })),
    [update],
  );

  const active =
    settings.enabled &&
    (settings.gainsDb.some((g) => g !== 0) ||
      settings.bassDb > 0 ||
      settings.bassDepth > 0 ||
      settings.bassPunch > 0 ||
      settings.smallSpeaker);
  return {
    settings,
    state,
    active,
    error,
    setGain,
    setEnabled,
    setBass,
    setSmallSpeaker,
    setDepth,
    setPunch,
    applyPreset,
  };
}
