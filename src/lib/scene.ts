/** Ekrandaki görsel sahneler. */
export type Scene = "spectrum" | "vu" | "sky" | "highway";

export const SCENES: { id: Scene; name: string }[] = [
  { id: "spectrum", name: "Nokta matris spektrum" },
  { id: "vu", name: "VU ibreleri" },
  { id: "sky", name: "Gece göğü" },
  { id: "highway", name: "Gece otoyolu" },
];

const STORAGE_KEY = "lyraska.scene";

/** Sıradaki sahne (sonuncudan sonra başa döner). */
export function nextScene(current: Scene): Scene {
  const index = SCENES.findIndex((s) => s.id === current);
  return SCENES[(index + 1) % SCENES.length]!.id;
}

export function sceneName(scene: Scene): string {
  return SCENES.find((s) => s.id === scene)?.name ?? scene;
}

/** Son seçilen sahne. Kayıt yoksa ya da okunamazsa spektrum. */
export function loadScene(): Scene {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    return SCENES.some((s) => s.id === saved) ? (saved as Scene) : "spectrum";
  } catch {
    return "spectrum";
  }
}

export function saveScene(scene: Scene): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, scene);
  } catch {
    /* Kaydedilemezse bir sonraki açılışta spektrumla başlar. */
  }
}
