import { useCallback, useState } from "react";
import { loadSkyLook, saveSkyLook, type SkyLook } from "../lib/sky";

export interface SkyLookControls {
  look: SkyLook;
  setLook: (look: SkyLook) => void;
}

/** Gece göğünün görünümü (göl, korona, karlı vadi); seçim hatırlanır. */
export function useSkyLook(): SkyLookControls {
  const [look, setLookState] = useState<SkyLook>(loadSkyLook);
  const setLook = useCallback((next: SkyLook) => {
    setLookState(next);
    saveSkyLook(next);
  }, []);
  return { look, setLook };
}
