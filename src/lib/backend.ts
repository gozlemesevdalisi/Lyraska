import { invoke, isTauri } from "@tauri-apps/api/core";

/** Rust tarafındaki `commands::AppInfo` yapısının TypeScript karşılığı. */
export interface AppInfo {
  name: string;
  version: string;
  phase: string;
  audioEngine: string;
  analysis: string;
  visualBridge: string;
}

/** Tarayıcıda (Tauri dışında) geliştirme yaparken kullanılan yedek bilgi. */
export const BROWSER_FALLBACK: AppInfo = {
  name: "Lyraska",
  version: __APP_VERSION__,
  phase: "Faz 0",
  audioEngine: "tarayıcı önizlemesi",
  analysis: "tarayıcı önizlemesi",
  visualBridge: "tarayıcı önizlemesi",
};

/** Program bilgisini Rust çekirdeğinden alır. */
export async function getAppInfo(): Promise<AppInfo> {
  if (!isTauri()) return BROWSER_FALLBACK;
  return invoke<AppInfo>("app_info");
}
