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

/** Rust tarafındaki `visual_bridge::VisualFrame`. */
export interface VisualFrame {
  positionSecs: number;
  /** Logaritmik aralıklı frekans bantları (bastan tize), 0..1. */
  bands: number[];
}

/** Rust tarafındaki `library::FolderRow`. */
export interface LibraryFolder {
  id: number;
  path: string;
}

/** Rust tarafındaki `library::TrackRow`. */
export interface LibraryTrack {
  id: number;
  path: string;
  title: string;
  artist: string | null;
  album: string | null;
  trackNumber: number | null;
  durationSecs: number | null;
  codec: string;
}

/** Rust tarafındaki `library::scan::ScanProgress`. */
export interface ScanProgress {
  scanning: boolean;
  found: number;
  processed: number;
  current: string | null;
}

/** Rust tarafındaki `library::LibraryStatus`. */
export interface LibraryStatus {
  folders: LibraryFolder[];
  trackCount: number;
  scan: ScanProgress;
  problems: string[];
}

export const EMPTY_LIBRARY: LibraryStatus = {
  folders: [],
  trackCount: 0,
  scan: { scanning: false, found: 0, processed: 0, current: null },
  problems: [],
};

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

export function seekPlayback(seconds: number): Promise<PlaybackStatus> {
  return invoke<PlaybackStatus>("seek_playback", { seconds });
}

/** Şu an duyulan anın görsel verisi; analiz o ana yetişmediyse `null`. */
export function getVisualFrame(): Promise<VisualFrame | null> {
  return invoke<VisualFrame | null>("visual_frame");
}

export function getPlaybackStatus(): Promise<PlaybackStatus> {
  return invoke<PlaybackStatus>("playback_status");
}

/** Klasör seçme penceresini gösterir; seçilen klasörün yolunu ya da `null` döndürür. */
export async function pickFolder(): Promise<string | null> {
  const selected = await open({ title: "Müzik klasörü seç", multiple: false, directory: true });
  return typeof selected === "string" ? selected : null;
}

export function getLibraryStatus(): Promise<LibraryStatus> {
  return invoke<LibraryStatus>("library_status");
}

export function addLibraryFolder(path: string): Promise<LibraryStatus> {
  return invoke<LibraryStatus>("library_add_folder", { path });
}

export function removeLibraryFolder(id: number): Promise<LibraryStatus> {
  return invoke<LibraryStatus>("library_remove_folder", { id });
}

export function rescanLibrary(): Promise<LibraryStatus> {
  return invoke<LibraryStatus>("library_rescan");
}

export function searchLibrary(query: string): Promise<LibraryTrack[]> {
  return invoke<LibraryTrack[]>("library_search", { query });
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
