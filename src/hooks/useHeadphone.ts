import { useCallback, useEffect, useState } from "react";
import {
  NO_HEADPHONE,
  clearHeadphone,
  errorMessage,
  getHeadphone,
  importHeadphoneProfile,
  pickHeadphoneProfile,
  setHeadphoneEnabled,
  type HeadphoneState,
} from "../lib/backend";

export interface HeadphoneControls {
  state: HeadphoneState;
  /** Düzeltme sesi şu an değiştiriyor mu? */
  active: boolean;
  busy: boolean;
  error: string | null;
  /** Dosya seçme penceresini açar ve seçilen profili yükler. */
  importProfile: () => Promise<void>;
  setEnabled: (enabled: boolean) => Promise<void>;
  clear: () => Promise<void>;
}

/** Kulaklık düzeltmesi (AutoEq profili): yükleme, açma/kapama, kaldırma. */
export function useHeadphone(): HeadphoneControls {
  const [state, setState] = useState<HeadphoneState>(NO_HEADPHONE);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getHeadphone()
      .then((loaded) => !cancelled && setState(loaded))
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, []);

  const run = useCallback(async (action: () => Promise<HeadphoneState | null>) => {
    setBusy(true);
    try {
      const next = await action();
      if (next) setState(next);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }, []);

  const importProfile = useCallback(
    () =>
      run(async () => {
        const path = await pickHeadphoneProfile();
        return path ? importHeadphoneProfile(path) : null;
      }),
    [run],
  );
  const setEnabled = useCallback(
    (enabled: boolean) => run(() => setHeadphoneEnabled(enabled)),
    [run],
  );
  const clear = useCallback(() => run(() => clearHeadphone()), [run]);

  return {
    state,
    active: state.enabled && state.profile !== null,
    busy,
    error,
    importProfile,
    setEnabled,
    clear,
  };
}
