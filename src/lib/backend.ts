import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/** Rust tarafındaki `commands::AppInfo` yapısının TypeScript karşılığı. */
export interface AppInfo {
  name: string;
  version: string;
  phase: string;
  audioEngine: string;
  analysis: string;
  visualBridge: string;
  supportedExtensions: string[];
}

/** Rust tarafındaki `audio::decode::TrackInfo`. */
export interface TrackInfo {
  path: string;
  fileName: string;
  title: string | null;
  artist: string | null;
  codec: string;
  sampleRate: number;
  channels: number;
  durationSecs: number | null;
}

/** Rust tarafındaki `audio::player::PlaybackState`. */
export type PlaybackState = "idle" | "playing" | "paused" | "ended" | "error";

/** Rust tarafındaki `audio::player::PlaybackStatus`. */
export interface PlaybackStatus {
  state: PlaybackState;
  track: TrackInfo | null;
  positionSecs: number;
  underruns: number;
  error: string | null;
}

export const IDLE_STATUS: PlaybackStatus = {
  state: "idle",
  track: null,
  positionSecs: 0,
  underruns: 0,
  error: null,
};

/** Tarayıcıda (Tauri dışında) geliştirme yaparken kullanılan yedek bilgi. */
export const BROWSER_FALLBACK: AppInfo = {
  name: "Lyraska",
  version: __APP_VERSION__,
  phase: "Faz 1",
  audioEngine: "tarayıcı önizlemesi",
  analysis: "tarayıcı önizlemesi",
  visualBridge: "tarayıcı önizlemesi",
  supportedExtensions: [],
};

/** Program Tauri penceresinde mi çalışıyor? (Tarayıcı önizlemesinde ses çalınamaz.) */
export function isDesktop(): boolean {
  return isTauri();
}

/** Program bilgisini Rust çekirdeğinden alır. */
export async function getAppInfo(): Promise<AppInfo> {
  if (!isTauri()) return BROWSER_FALLBACK;
  return invoke<AppInfo>("app_info");
}

/** "Dosya aç" penceresini gösterir; seçilen dosyanın yolunu ya da `null` döndürür. */
export async function pickAudioFile(extensions: string[]): Promise<string | null> {
  const selected = await open({
    title: "Şarkı seç",
    multiple: false,
    directory: false,
    filters: extensions.length > 0 ? [{ name: "Ses dosyaları", extensions }] : undefined,
  });
  return typeof selected === "string" ? selected : null;
}

export function openTrack(path: string): Promise<TrackInfo> {
  return invoke<TrackInfo>("open_track", { path });
}

export function togglePlayback(): Promise<PlaybackStatus> {
  return invoke<PlaybackStatus>("toggle_playback");
}

export function stopPlayback(): Promise<PlaybackStatus> {
  return invoke<PlaybackStatus>("stop_playback");
}

export function getPlaybackStatus(): Promise<PlaybackStatus> {
  return invoke<PlaybackStatus>("playback_status");
}

/** Pencereye bırakılan dosyaları dinler. Dinlemeyi bırakmak için dönen fonksiyon çağrılır. */
export async function onFileDrop(handler: (paths: string[]) => void): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  return getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === "drop") handler(event.payload.paths);
  });
}

/** Tauri komutlarından gelen hatayı (genellikle Türkçe metin) okunur hale getirir. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Bilinmeyen bir hata oluştu.";
}
