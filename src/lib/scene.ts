/** Ekrandaki görsel sahneler. */
export type Scene = "sky" | "highway" | "vu" | "spectrum";

/** Üst çubuktaki sırayla; 1–4 tuşları da bu sırayı izler. */
export const SCENES: { id: Scene; name: string }[] = [
  { id: "sky", name: "Gece göğü" },
  { id: "highway", name: "Gece otoyolu" },
  { id: "vu", name: "VU ibreleri" },
  { id: "spectrum", name: "Spektrum" },
];

const STORAGE_KEY = "lyraska.scene";
const DEFAULT_SCENE: Scene = "sky";

/** Klavyede 1–4 tuşunun seçtiği sahne; başka tuşta `null`. */
export function sceneForKey(key: string): Scene | null {
  const index = /^[1-9]$/.test(key) ? Number(key) - 1 : -1;
  return SCENES[index]?.id ?? null;
}

/** Son seçilen sahne. Kayıt yoksa ya da okunamazsa gece göğü. */
export function loadScene(): Scene {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    return SCENES.some((s) => s.id === saved) ? (saved as Scene) : DEFAULT_SCENE;
  } catch {
    return DEFAULT_SCENE;
  }
}

export function saveScene(scene: Scene): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, scene);
  } catch {
    /* Kaydedilemezse bir sonraki açılışta gece göğüyle başlar. */
  }
}
