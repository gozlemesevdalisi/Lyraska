import { useCallback, useEffect, useState } from "react";
import { loadScene, saveScene, sceneForKey, type Scene } from "../lib/scene";

/** Seçili sahne (hatırlanır); 1–4 tuşları da sahne seçer (yazı yazılırken değil). */
export function useScene(): [Scene, (scene: Scene) => void] {
  const [scene, setScene] = useState<Scene>(loadScene);
  const choose = useCallback((next: Scene) => {
    setScene(next);
    saveScene(next);
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("input, textarea, select, [contenteditable]")) return;
      const picked = sceneForKey(event.key);
      if (picked) choose(picked);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [choose]);

  return [scene, choose];
}
