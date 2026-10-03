import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { previewEqState } from "./eq";

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
  /** Şarkının temposu; analiz bitene kadar ya da belirgin ritim yoksa `null`. */
  bpm: number | null;
  /** Sesin aygıta giden yolu; şarkı açık değilse `null`. */
  output: SignalPath | null;
}

/** Rust tarafındaki `audio::player::SignalPath`. */
export interface SignalPath {
  /** Aygıtın adı; bilinmiyorsa boş. */
  deviceName: string;
  /** Aygıta giden örnekleme hızı ve kanal sayısı. */
  sampleRate: number;
  channels: number;
  /** Şarkı aygıtın hızına Lyraska'nın dönüştürücüsüyle çevriliyor mu? */
  resampled: boolean;
}

/** Rust tarafındaki `visual_bridge::VisualFrame`. */
export interface VisualFrame {
  positionSecs: number;
  /** Logaritmik aralıklı frekans bantları (bastan tize), 0..1. */
  bands: number[];
  /** Sol/sağ etkin (RMS) seviye, dBFS; sessizlik −60. */
  rmsDb: [number, number];
  /** Sol/sağ tepe seviye, dBFS; sessizlik −60. */
  peakDb: [number, number];
  /** 0 VU'ya denk gelen seviye (dBFS), şarkıya göre; analiz bitene kadar `null`. */
  vuReferenceDb: number | null;
  /** Tempo ve vuruş konumu; analiz bitene kadar ya da ritim yoksa `null`. */
  beat: BeatFrame | null;
}

/** Rust tarafındaki `visual_bridge::BeatFrame`. */
export interface BeatFrame {
  bpm: number;
  /** Son vuruşun sırası (0'dan başlar). */
  index: number;
  /** Son vuruştan bu yana geçen süre, vuruş aralığına oranla (0..1). */
  phase: number;
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

/** Rust tarafındaki `audio::eq::EqSettings`. */
export interface EqSettings {
  enabled: boolean;
  /** Bant başına kazanç (dB, ±12), bastan tize 10 bant. */
  gainsDb: number[];
}

/** Rust tarafındaki `audio::eq::EqState`. */
export interface EqState extends EqSettings {
  bandsHz: number[];
  maxGainDb: number;
  /** Kırpılmayı önlemek için düşürülen kazanç (dB, ≤ 0). */
  preampDb: number;
  /** Gerçekten uygulanan eğri (ön kazanç hariç). */
  curveHz: number[];
  curveDb: number[];
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
  bpm: null,
  output: null,
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

/**
 * Şarkı seçme penceresini gösterir (birden fazla seçilebilir); seçilen dosyaların
 * yollarını döndürür (vazgeçilirse boş liste).
 */
export async function pickAudioFiles(extensions: string[]): Promise<string[]> {
  const selected = await open({
    title: "Kütüphaneye eklenecek şarkıları seçin (birden fazla seçebilirsiniz)",
    multiple: true,
    directory: false,
    filters: extensions.length > 0 ? [{ name: "Ses dosyaları", extensions }] : undefined,
  });
  if (Array.isArray(selected)) return selected;
  return typeof selected === "string" ? [selected] : [];
}

/**
 * Klasör seçme penceresinin başlığı. Windows bu pencerede dosyaları göstermez;
 * yalnızca şarkı içeren bir klasöre girince "öğe yok" yazar. Kullanıcı bunu hata
 * sanmasın diye ne yapacağı başlıkta yazar.
 */
export const PICK_FOLDER_TITLE =
  "Müzik klasörünü seçin — şarkılar bu pencerede görünmez; klasöre girip “Klasör seç”e basın";

/** Klasör seçme penceresini gösterir; seçilen klasörün yolunu ya da `null` döndürür. */
export async function pickFolder(): Promise<string | null> {
  const selected = await open({ title: PICK_FOLDER_TITLE, multiple: false, directory: true });
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

/** Ekolayzer durumu. Tarayıcı önizlemesinde yaklaşık bir eğri hesaplanır. */
export async function getEqualizer(): Promise<EqState> {
  if (!isTauri()) return previewEqState({ enabled: true, gainsDb: Array(10).fill(0) });
  return invoke<EqState>("equalizer_get");
}

/** Ekolayzer ayarlarını uygular ve kaydeder; uygulanan eğriyi döndürür. */
export async function setEqualizer(settings: EqSettings): Promise<EqState> {
  if (!isTauri()) return previewEqState(settings);
  return invoke<EqState>("equalizer_set", { settings });
}

/** Rust tarafındaki `audio::peq::PeqFilter`. */
export interface HeadphoneFilter {
  kind: "peaking" | "lowShelf" | "highShelf";
  freqHz: number;
  gainDb: number;
  q: number;
}

/** Rust tarafındaki `audio::peq::HeadphoneProfile`. */
export interface HeadphoneProfile {
  name: string;
  preampDb: number;
  filters: HeadphoneFilter[];
}

/** Rust tarafındaki `audio::peq::HeadphoneState`. */
export interface HeadphoneState {
  enabled: boolean;
  profile: HeadphoneProfile | null;
  /** Düzeltme eğrisi (ön kazanç dahil); profil yoksa boş. */
  curveHz: number[];
  curveDb: number[];
}

export const NO_HEADPHONE: HeadphoneState = {
  enabled: false,
  profile: null,
  curveHz: [],
  curveDb: [],
};

/** Kulaklık düzeltmesi. Tarayıcı önizlemesinde profil yoktur. */
export async function getHeadphone(): Promise<HeadphoneState> {
  if (!isTauri()) return NO_HEADPHONE;
  return invoke<HeadphoneState>("headphone_get");
}

/** AutoEq profil dosyasını (ParametricEQ.txt) yükler ve düzeltmeyi açar. */
export async function importHeadphoneProfile(path: string): Promise<HeadphoneState> {
  return invoke<HeadphoneState>("headphone_import", { path });
}

export async function setHeadphoneEnabled(enabled: boolean): Promise<HeadphoneState> {
  return invoke<HeadphoneState>("headphone_set_enabled", { enabled });
}

export async function clearHeadphone(): Promise<HeadphoneState> {
  return invoke<HeadphoneState>("headphone_clear");
}

/** Kulaklık profili seçme penceresi; seçilen dosyanın yolu ya da `null`. */
export async function pickHeadphoneProfile(): Promise<string | null> {
  const selected = await open({
    title: "AutoEq kulaklık profilini seçin (ParametricEQ.txt)",
    multiple: false,
    directory: false,
    filters: [{ name: "AutoEq parametrik profil", extensions: ["txt"] }],
  });
  return typeof selected === "string" ? selected : null;
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
