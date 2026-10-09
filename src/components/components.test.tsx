import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { PlaybackStatus, TrackInfo } from "../lib/backend";
import { marqueeWindow } from "./Marquee";
import { spectrumColumns } from "./SpectrumDemo";
import { textToColumns } from "../lib/dotFont";

// Rust çekirdeğini taklit eden sahte arka uç.
const backend = vi.hoisted(() => ({
  desktop: false,
  /** Çekirdeğin program bilgisini (desteklenen uzantılar) geciktirmesi için. */
  appInfoGate: null as Promise<void> | null,
  status: null as PlaybackStatus | null,
  toggle: vi.fn(),
  stop: vi.fn(),
  seek: vi.fn(),
  bands: null as number[] | null,
  library: null as import("../lib/backend").LibraryStatus | null,
  tracks: [] as import("../lib/backend").LibraryTrack[],
  search: vi.fn(),
  addFolder: vi.fn(),
  removeFolder: vi.fn(),
  pickFolder: vi.fn(),
  pickFiles: vi.fn(),
  open: vi.fn(),
  pick: vi.fn(),
  drag: null as import("../lib/backend").DragDropHandlers | null,
  /** Sürükle-bırak dinlemesi kurulamazsa verilecek hata. */
  dragFailure: null as Error | null,
  dropped: vi.fn(),
  dropOutcome: null as import("../lib/backend").DropOutcome | null,
  logError: vi.fn(),
  setEq: vi.fn(),
  headphone: null as import("../lib/backend").HeadphoneState | null,
  pickProfile: vi.fn(),
  importProfile: vi.fn(),
  setNext: vi.fn(),
  openLog: vi.fn(),
  saveMarks: vi.fn(),
  annotation: null as import("../lib/backend").Annotation | null,
  songMap: null as import("../lib/backend").SongMap | null,
  safe: false,
  setSafe: vi.fn(),
  /** Çekirdek ekolayzer durumunu eksik alanlarla döndürür (0.0.29'daki sahne denetimi olayı). */
  brokenEqualizer: false,
}));

vi.mock("../lib/backend", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/backend")>();
  const current = () => backend.status ?? actual.IDLE_STATUS;
  return {
    ...actual,
    isDesktop: () => backend.desktop,
    getAppInfo: async () => {
      if (backend.appInfoGate) await backend.appInfoGate;
      return { ...actual.BROWSER_FALLBACK, supportedExtensions: ["mp3", "flac"] };
    },
    getPlaybackStatus: async () => current(),
    togglePlayback: async () => {
      backend.toggle();
      return current();
    },
    stopPlayback: async () => {
      backend.stop();
      return current();
    },
    seekPlayback: async (seconds: number) => {
      backend.seek(seconds);
      // Gerçek çekirdek gibi: yeni konumdan devam eder.
      if (backend.status) backend.status = { ...backend.status, positionSecs: seconds };
      return current();
    },
    getVisualFrame: async () =>
      backend.bands ? { positionSecs: current().positionSecs, bands: backend.bands } : null,
    openTrack: async (path: string) => {
      backend.open(path);
      return current().track;
    },
    pickAudioFile: async (extensions: string[]) => backend.pick(extensions),
    onDragDrop: async (handlers: import("../lib/backend").DragDropHandlers) => {
      if (backend.dragFailure) throw backend.dragFailure;
      backend.drag = handlers;
      return () => {
        if (backend.drag === handlers) backend.drag = null;
      };
    },
    addDroppedPaths: async (paths: string[]) => {
      backend.dropped(paths);
      return (
        backend.dropOutcome ?? {
          tracks: [],
          addedTracks: 0,
          addedFolders: 0,
          already: 0,
          problems: [],
        }
      );
    },
    logFrontendError: async (message: string) => backend.logError(message),
    getEqualizer: async () =>
      backend.brokenEqualizer
        ? ({
            enabled: true,
            gainsDb: Array(10).fill(0),
          } as unknown as import("../lib/backend").EqState)
        : actual.getEqualizer(),
    getLibraryStatus: async () => backend.library ?? actual.EMPTY_LIBRARY,
    searchLibrary: async (query: string) => {
      backend.search(query);
      return backend.tracks;
    },
    pickFolder: async () => backend.pickFolder(),
    pickAudioFiles: async (extensions: string[]) => backend.pickFiles(extensions),
    addLibraryFolder: async (path: string) => {
      backend.addFolder(path);
      if (path.includes("zaten")) throw 'Bu zaten kütüphanede: "C:\\Müzik" klasörünün içinde.';
      return backend.library ?? actual.EMPTY_LIBRARY;
    },
    removeLibraryFolder: async (id: number) => {
      backend.removeFolder(id);
      return backend.library ?? actual.EMPTY_LIBRARY;
    },
    rescanLibrary: async () => backend.library ?? actual.EMPTY_LIBRARY,
    setEqualizer: async (settings: import("../lib/backend").EqSettings) => {
      backend.setEq(settings);
      return actual.setEqualizer(settings);
    },
    openLog: async () => backend.openLog(),
    getVisualSafe: async () => backend.safe,
    setVisualSafe: async (enabled: boolean) => {
      backend.setSafe(enabled);
      backend.safe = enabled;
      return enabled;
    },
    getAnnotation: async () => backend.annotation,
    getSongMap: async () => backend.songMap,
    saveAnnotation: async (path: string, beats: number[], drops: number[]) => {
      backend.saveMarks(path, beats, drops);
      return "C:\\Veri\\isaretler\\Lyra - Gece Otoyolu.1234abcd.json";
    },
    evaluateAnnotation: async () => ({
      savedAt: "",
      beats: [],
      drops: [],
      evaluation: {
        fMeasure: 0.873,
        tapOffsetMs: 42,
        fMeasureAligned: 0.951,
        detectedBpm: 128,
        markedBpm: 127.9,
        detectedCount: 400,
        markedCount: 380,
        detectedDrops: [61.5],
        dropHits: 1,
        markedDrops: 1,
      },
    }),
    setNextTrack: async (path: string | null) => {
      backend.setNext(path);
    },
    getHeadphone: async () => backend.headphone ?? actual.NO_HEADPHONE,
    pickHeadphoneProfile: async () => backend.pickProfile(),
    importHeadphoneProfile: async (path: string) => {
      backend.importProfile(path);
      if (path.includes("bozuk")) throw "Dosyada kulaklık düzeltmesi bulunamadı.";
      backend.headphone = {
        enabled: true,
        profile: {
          name: "Sennheiser HD 600",
          preampDb: -6.4,
          filters: [{ kind: "peaking", freqHz: 1000, gainDb: 3, q: 1 }],
        },
        curveHz: [20, 1000, 20000],
        curveDb: [-6.4, -3.4, -6.4],
      };
      return backend.headphone;
    },
    setHeadphoneEnabled: async (enabled: boolean) => {
      backend.headphone = { ...(backend.headphone ?? actual.NO_HEADPHONE), enabled };
      return backend.headphone;
    },
    clearHeadphone: async () => {
      backend.headphone = null;
      return actual.NO_HEADPHONE;
    },
  };
});

// Testler bileşeni sahte arka uçla birlikte yükler.
const { default: App } = await import("../App");
const { stateLabel } = await import("./NowPlaying");
const { pointerRatio } = await import("./SeekBar");

const track: TrackInfo = {
  path: "C:\\Müzik\\gece.flac",
  fileName: "gece",
  title: "Gece Otoyolu",
  artist: "Lyra",
  codec: "flac",
  sampleRate: 44100,
  channels: 2,
  durationSecs: 225,
  album: null,
  albumArtist: null,
  trackNumber: null,
  discNumber: null,
};

const playing: PlaybackStatus = {
  state: "playing",
  track,
  positionSecs: 83.4,
  underruns: 0,
  error: null,
  bpm: null,
  output: null,
};

beforeEach(() => {
  backend.desktop = false;
  backend.status = null;
  backend.bands = null;
  backend.library = null;
  backend.tracks = [];
  backend.drag = null;
  backend.dragFailure = null;
  backend.dropOutcome = null;
  backend.songMap = null;
  backend.annotation = null;
  window.localStorage.clear();
  vi.clearAllMocks();
});

/** Alttaki şeridin erişilebilir süre metni: "01:23 / 03:45". */
const seekText = () =>
  screen.getByRole("slider", { name: "Şarkıda konum" }).getAttribute("aria-valuetext");

describe("oynatıcı ekranı", () => {
  it("tarayıcı önizlemesinde program adını gösterir, düğmeler kapalıdır", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { level: 1, name: "Lyraska" })).toBeInTheDocument();
    expect(screen.getByText("Hoş geldiniz")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Dosya aç/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Çal/ })).toBeDisabled();
    expect(screen.getByText(/Windows'ta açın/)).toBeInTheDocument();
    // Program bilgisi ayarlar panelinde.
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Ayarlar" })));
    await waitFor(() => expect(screen.getByText(/Faz 1 · Sürüm/)).toBeInTheDocument());
  });

  it("dosya seçilince şarkıyı açar ve süreyi gösterir", async () => {
    backend.desktop = true;
    backend.pick.mockResolvedValue("C:\\Müzik\\gece.flac");
    render(<App />);

    const open = screen.getByRole("button", { name: /Dosya aç/ });
    await waitFor(() => expect(open).toBeEnabled());
    backend.status = playing;
    await act(async () => fireEvent.click(open));

    await waitFor(() => expect(backend.open).toHaveBeenCalledWith("C:\\Müzik\\gece.flac"));
    expect(backend.pick).toHaveBeenCalledWith(["mp3", "flac"]);
    // Büyük başlıkta şarkının adı ve sanatçısı, altta süre.
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1, name: "Gece Otoyolu" })).toBeInTheDocument(),
    );
    expect(screen.getByText("Şimdi çalıyor")).toBeInTheDocument();
    expect(screen.getByText("Lyra", { selector: ".now__byline" })).toBeInTheDocument();
    expect(seekText()).toBe("01:23 / 03:45");
    expect(screen.getByText("01:23", { selector: ".dock__time" })).toBeInTheDocument();
    expect(screen.getByText("03:45", { selector: ".dock__time" })).toBeInTheDocument();
    expect(screen.getByText("FLAC 44,1 kHz")).toBeInTheDocument();
    expect(screen.getByText("Kesinti: 0")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Duraklat/ })).toBeEnabled();
  });

  it("Boşluk tuşu çal/duraklat komutunu gönderir", async () => {
    backend.desktop = true;
    backend.status = { ...playing, state: "paused" };
    render(<App />);
    await act(async () => {
      fireEvent.keyDown(window, { code: "Space", key: " " });
    });
    expect(backend.toggle).toHaveBeenCalledTimes(1);
  });
});

describe("sarma ve spektrum", () => {
  it("ok tuşları ekranda görülen konumdan 5 saniye ileri/geri sarar", async () => {
    backend.desktop = true;
    backend.status = { ...playing, state: "paused" };
    render(<App />);
    // Durum ilk komutla gelir; oynatıcıyı uyandırmak için bir kez Boşluk.
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    await act(async () => fireEvent.keyDown(window, { key: "ArrowRight", code: "ArrowRight" }));
    expect(backend.seek).toHaveBeenLastCalledWith(88.4);
    await act(async () => fireEvent.keyDown(window, { key: "ArrowLeft", code: "ArrowLeft" }));
    expect(backend.seek).toHaveBeenLastCalledWith(83.4);
  });

  it("art arda ok basışları birikir, hiçbiri kaybolmaz", async () => {
    backend.desktop = true;
    backend.status = { ...playing, state: "paused" };
    render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    // Üç basış, ilk sarma cevabı gelmeden: ekran hemen 83,4 + 15 sn'yi göstermeli.
    await act(async () => {
      fireEvent.keyDown(window, { key: "ArrowRight", code: "ArrowRight" });
      fireEvent.keyDown(window, { key: "ArrowRight", code: "ArrowRight" });
      fireEvent.keyDown(window, { key: "ArrowRight", code: "ArrowRight" });
    });
    await waitFor(() => expect(backend.seek).toHaveBeenLastCalledWith(98.4));
    expect(seekText()).toBe("01:38 / 03:45");
  });

  it("çubuğa tıklayınca süre ses motorunu beklemeden hemen değişir", async () => {
    backend.desktop = true;
    backend.status = { ...playing, state: "paused" };
    render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    const bar = await screen.findByRole("slider", { name: "Şarkıda konum" });
    bar.getBoundingClientRect = () => ({ left: 0, width: 225 }) as DOMRect;
    act(() => {
      fireEvent.pointerDown(bar, { button: 0, clientX: 150, pointerId: 1 });
      fireEvent.pointerUp(bar, { button: 0, clientX: 150, pointerId: 1 });
    });
    // Henüz ses motoru cevap vermeden: ekran 02:30'u göstermeli, eski yere sekmemeli.
    expect(seekText()).toBe("02:30 / 03:45");
    expect(screen.getByText("02:30", { selector: ".dock__time" })).toBeInTheDocument();
    await waitFor(() => expect(backend.seek).toHaveBeenCalledWith(150));
  });

  it("ilerleme çubuğuna tıklayınca o noktaya atlar", async () => {
    backend.desktop = true;
    backend.status = playing;
    render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    const bar = await screen.findByRole("slider", { name: "Şarkıda konum" });
    bar.getBoundingClientRect = () => ({ left: 100, width: 400 }) as DOMRect;
    await act(async () => {
      fireEvent.pointerDown(bar, { button: 0, clientX: 200, pointerId: 1 });
      fireEvent.pointerUp(bar, { button: 0, clientX: 200, pointerId: 1 });
    });
    // 200 px → çubuğun %25'i → 225 sn'lik şarkıda 56,25 sn
    expect(backend.seek).toHaveBeenCalledWith(56.25);
  });

  it("fareyle çubuğun üstüne gelince o noktanın süresini gösterir", async () => {
    backend.desktop = true;
    backend.status = playing;
    render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    const bar = await screen.findByRole("slider", { name: "Şarkıda konum" });
    bar.getBoundingClientRect = () => ({ left: 0, width: 225 }) as DOMRect;
    await act(async () => fireEvent.pointerMove(bar, { clientX: 150 }));
    expect(screen.getByText("02:30")).toBeInTheDocument();
    await act(async () => fireEvent.pointerLeave(bar));
    expect(screen.queryByText("02:30")).not.toBeInTheDocument();
    expect(backend.seek).not.toHaveBeenCalled();
  });

  it("çalarken spektrum gerçek veriyle yanar", async () => {
    backend.desktop = true;
    backend.status = playing;
    backend.bands = new Array<number>(32).fill(0.9);
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Spektrum" })));
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    const spectrum = await screen.findByRole("img", { name: "Spektrum" });
    await waitFor(() => {
      const lit = Number(
        spectrum.querySelector(".dot-matrix__lit")!.getAttribute("data-lit-count"),
      );
      expect(lit).toBeGreaterThan(100);
    });
  });
});

describe("kütüphane", () => {
  const song = (id: number, title: string) => ({
    id,
    path: `C:\\Müzik\\${id}.flac`,
    title,
    artist: "Şebnem Ferah",
    album: "Kelimeler",
    trackNumber: id,
    durationSecs: 200,
    codec: "flac",
    // 1. şarkı 128 BPM, 2. ritimsiz, 3. henüz analiz edilmedi.
    bpm: id === 1 ? 128.4 : null,
    analyzed: id !== 3,
  });
  const library = {
    folders: [{ id: 7, path: "C:\\Müzik" }],
    trackCount: 3,
    analyzed: 2,
    scan: { scanning: false, found: 3, processed: 3, current: null },
    problems: [],
  };

  it("klasör yokken klasör eklemeye davet eder ve seçilen klasörü ekler", async () => {
    backend.desktop = true;
    backend.pickFolder.mockResolvedValue("D:\\Arşiv");
    render(<App />);
    expect(await screen.findByText("Müziğinizi ekleyin")).toBeInTheDocument();
    const [, add] = screen.getAllByRole("button", { name: /Klasör ekle/ });
    await act(async () => fireEvent.click(add!));
    expect(backend.addFolder).toHaveBeenCalledWith("D:\\Arşiv");
  });

  it("şarkı ekle: seçilen mp3'leri tek tek ekler, eklenemeyeni bildirir", async () => {
    backend.desktop = true;
    backend.pickFiles.mockResolvedValue([
      "C:\\İndirilenler\\Gülümse.mp3",
      "C:\\Müzik\\zaten.MP3",
      "C:\\İndirilenler\\Bu Akşam.mp3",
    ]);
    render(<App />);
    await screen.findByText("Müziğinizi ekleyin");
    // Uzantılar yüklenince pencere ses dosyalarını gösterir.
    await waitFor(() =>
      expect(screen.getAllByRole("button", { name: /Şarkı ekle/ })).toHaveLength(2),
    );
    const [add] = screen.getAllByRole("button", { name: /Şarkı ekle/ });
    await act(async () => fireEvent.click(add!));
    await waitFor(() => expect(backend.addFolder).toHaveBeenCalledTimes(3));
    expect(backend.pickFiles).toHaveBeenCalledWith(["mp3", "flac"]);
    expect(backend.addFolder).toHaveBeenLastCalledWith("C:\\İndirilenler\\Bu Akşam.mp3");
    expect(await screen.findByText(/Bu zaten kütüphanede/)).toBeInTheDocument();
  });

  it("sürüklerken pencerede 'bırakın' çerçevesi görünür", async () => {
    backend.desktop = true;
    const { container } = render(<App />);
    await waitFor(() => expect(backend.drag).not.toBeNull());
    const zone = container.querySelector(".drop-zone")!;
    expect(zone).not.toHaveClass("is-active");
    act(() => backend.drag!.onHover(true));
    expect(zone).toHaveClass("is-active");
    expect(zone).toHaveTextContent(/Bırakın/);
    act(() => backend.drag!.onHover(false));
    expect(zone).not.toHaveClass("is-active");
  });

  it("bırakılanları kütüphaneye ekler, şarkıları sırayla çalar ve sonucu bildirir", async () => {
    backend.desktop = true;
    backend.dropOutcome = {
      tracks: ["C:\\İndirilenler\\a.mp3", "C:\\İndirilenler\\b.MP3"],
      addedTracks: 2,
      addedFolders: 1,
      already: 0,
      problems: [],
    };
    render(<App />);
    await waitFor(() => expect(backend.drag).not.toBeNull());
    const paths = ["C:\\Müzik\\Rock", "C:\\İndirilenler\\a.mp3", "C:\\İndirilenler\\b.MP3"];
    await act(async () => backend.drag!.onDrop(paths));
    // Dosya mı klasör mü olduğuna çekirdek karar verir: hepsi olduğu gibi gönderilir.
    expect(backend.dropped).toHaveBeenCalledWith(paths);
    await waitFor(() => expect(backend.open).toHaveBeenCalledWith("C:\\İndirilenler\\a.mp3"));
    expect(backend.open).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("status")).toHaveTextContent(
      "2 şarkı kütüphaneye eklendi · 1 klasör kütüphaneye eklendi, şarkıları taranıyor",
    );
  });

  it("program bilgisi gelmeden bırakılan şarkı da hemen eklenip çalınır", async () => {
    backend.desktop = true;
    let release = () => {};
    backend.appInfoGate = new Promise<void>((resolve) => (release = resolve));
    backend.dropOutcome = {
      tracks: ["C:\\İndirilenler\\a.mp3"],
      addedTracks: 1,
      addedFolders: 0,
      already: 0,
      problems: [],
    };
    render(<App />);
    await waitFor(() => expect(backend.drag).not.toBeNull());
    await act(async () => backend.drag!.onDrop(["C:\\İndirilenler\\a.mp3"]));
    await waitFor(() => expect(backend.open).toHaveBeenCalledWith("C:\\İndirilenler\\a.mp3"));
    await act(async () => release());
    backend.appInfoGate = null;
  });

  it("eklenemeyen dosyayı hangi sekmede olursa olsun bildirir", async () => {
    backend.desktop = true;
    backend.dropOutcome = {
      tracks: [],
      addedTracks: 0,
      addedFolders: 0,
      already: 0,
      problems: ["Bu dosya türü desteklenmiyor: Eski Kayıt.wma"],
    };
    render(<App />);
    await act(async () => fireEvent.click(await screen.findByRole("tab", { name: /Senkron/ })));
    await waitFor(() => expect(backend.drag).not.toBeNull());
    await act(async () => backend.drag!.onDrop(["C:\\Eski Kayıt.wma"]));
    const notice = await screen.findByText("Bu dosya türü desteklenmiyor: Eski Kayıt.wma");
    expect(notice).toHaveClass("is-problem");
    expect(backend.open).not.toHaveBeenCalled();
  });

  it("sürükle-bırak dinlenemezse nedeni hata günlüğüne yazılır", async () => {
    backend.desktop = true;
    backend.dragFailure = new Error("izin yok: event.listen");
    render(<App />);
    await waitFor(() =>
      expect(backend.logError).toHaveBeenCalledWith(
        expect.stringContaining("Sürükle-bırak dinlenemedi: Error: izin yok: event.listen"),
      ),
    );
  });

  it("şarkıları listeler, çift tıklayınca çalar, bitince sıradakine geçer", async () => {
    backend.desktop = true;
    backend.library = library;
    backend.tracks = [song(1, "Mayın Tarlası"), song(2, "Bir Kedi Gördüm"), song(3, "Hoşçakal")];
    render(<App />);
    const first = await screen.findByText("Mayın Tarlası");

    // BPM sütunu ve arka plan analizinin ilerlemesi.
    const rows = screen.getAllByRole("row").slice(1);
    expect(rows.map((row) => row.querySelector(".library__bpm")?.textContent)).toEqual([
      "128",
      "—",
      "",
    ]);
    expect(screen.getByText(/Şarkı haritası: 2 \/ 3 şarkı analiz edildi/)).toBeInTheDocument();

    // Çift tıklayınca çalar.
    backend.open.mockImplementation((path: string) => {
      const track = backend.tracks.find((t) => t.path === path)!;
      backend.status = {
        ...playing,
        positionSecs: 0,
        track: { ...playing.track!, path, title: track.title, durationSecs: 200 },
      };
    });
    await act(async () => fireEvent.doubleClick(first));
    await waitFor(() => expect(backend.open).toHaveBeenLastCalledWith(backend.tracks[0]!.path));

    // "Sonraki" düğmesi ikinci şarkıyı çalar.
    const nextButton = screen.getByRole("button", { name: "Sonraki" });
    await waitFor(() => expect(nextButton).toBeEnabled());
    await act(async () => fireEvent.click(nextButton));
    await waitFor(() => expect(backend.open).toHaveBeenLastCalledWith(backend.tracks[1]!.path));

    // Sıradaki şarkı çekirdeğe önceden bildirilir (boşluksuz geçiş için).
    await waitFor(() => expect(backend.setNext).toHaveBeenLastCalledWith(backend.tracks[2]!.path));

    // Boşluksuz geçiş olamazsa: şarkı bitince üçüncüye kendiliğinden geçer.
    backend.status = { ...backend.status!, state: "ended", positionSecs: 200 };
    await waitFor(() => expect(backend.open).toHaveBeenLastCalledWith(backend.tracks[2]!.path), {
      timeout: 2000,
    });
    // Son şarkıda sıradaki yok.
    await waitFor(() => expect(backend.setNext).toHaveBeenLastCalledWith(null));
  });

  it("kütüphaneden çalınca çekmece kapanır, sıradaki görünür; düğmeyle ve Esc ile açılıp kapanır", async () => {
    backend.desktop = true;
    backend.library = library;
    backend.tracks = [song(1, "Mayın Tarlası"), song(2, "Bir Kedi Gördüm"), song(3, "Hoşçakal")];
    render(<App />);
    const drawer = document.getElementById("drawer")!;
    // Açılışta henüz bir şey çalmıyor: kütüphane açık.
    expect(drawer).toHaveAttribute("aria-hidden", "false");
    backend.open.mockImplementation((path: string) => {
      const track = backend.tracks.find((t) => t.path === path)!;
      backend.status = {
        ...playing,
        state: "paused",
        track: { ...playing.track!, path, title: track.title, durationSecs: 200 },
      };
    });
    const first = await screen.findByText("Mayın Tarlası");
    await act(async () => fireEvent.doubleClick(first));

    // Çalmaya başlayınca sahne ortaya çıkar.
    expect(drawer).toHaveAttribute("aria-hidden", "true");
    expect(drawer).toHaveAttribute("inert");
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1, name: "Mayın Tarlası" })).toBeInTheDocument(),
    );
    // Albüm kütüphaneden; sıradaki yer ve sıradaki şarkı.
    expect(screen.getByText("Lyra — Kelimeler")).toBeInTheDocument();
    expect(screen.getByText("Duraklatıldı · 1 / 3")).toBeInTheDocument();
    expect(screen.getByText("Bir Kedi Gördüm · Şebnem Ferah")).toBeInTheDocument();

    const libraryButton = screen.getByRole("button", { name: "Kütüphane" });
    expect(libraryButton).toHaveAttribute("aria-expanded", "false");
    await act(async () => fireEvent.click(libraryButton));
    expect(drawer).toHaveAttribute("aria-hidden", "false");
    expect(libraryButton).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("tab", { name: "Kütüphane" })).toHaveAttribute("aria-selected", "true");
    await act(async () => fireEvent.keyDown(window, { key: "Escape" }));
    expect(drawer).toHaveAttribute("aria-hidden", "true");
    // Ekolayzer düğmesi çekmeceyi ekolayzerde açar; yeniden basınca kapanır.
    const eqButton = screen.getByRole("button", { name: "Ekolayzer" });
    await act(async () => fireEvent.click(eqButton));
    expect(screen.getByRole("tab", { name: /Ekolayzer/ })).toHaveAttribute("aria-selected", "true");
    await act(async () => fireEvent.click(eqButton));
    expect(drawer).toHaveAttribute("aria-hidden", "true");
  });

  it("çekirdek sıradakine kendisi geçince sıra ilerler, şarkı yeniden açılmaz", async () => {
    backend.desktop = true;
    backend.library = library;
    backend.tracks = [song(1, "Birinci Şarkı"), song(2, "İkinci Şarkı"), song(3, "Üçüncü Şarkı")];
    render(<App />);
    backend.open.mockImplementation((path: string) => {
      backend.status = {
        ...playing,
        positionSecs: 0,
        track: { ...playing.track!, path, durationSecs: 200 },
      };
    });
    const first = await screen.findByText("Birinci Şarkı");
    await act(async () => fireEvent.doubleClick(first));
    await waitFor(() => expect(backend.setNext).toHaveBeenLastCalledWith(backend.tracks[1]!.path));
    const opened = backend.open.mock.calls.length;

    // Çekirdek boşluksuz geçti: çalan şarkı artık ikincisi.
    backend.status = {
      ...backend.status!,
      positionSecs: 0.2,
      track: { ...backend.status!.track!, path: backend.tracks[1]!.path },
    };
    await waitFor(() => expect(backend.setNext).toHaveBeenLastCalledWith(backend.tracks[2]!.path), {
      timeout: 2000,
    });
    expect(backend.open).toHaveBeenCalledTimes(opened);
    expect(screen.getByRole("button", { name: "Sonraki" })).toBeEnabled();
  });

  it("art arda boşluksuz geçişlerde sıra kaybolmaz, son şarkıya kadar sürer", async () => {
    backend.desktop = true;
    backend.library = library;
    backend.tracks = [
      song(1, "Birinci Şarkı"),
      song(2, "İkinci Şarkı"),
      song(3, "Üçüncü Şarkı"),
      song(4, "Dördüncü Şarkı"),
    ];
    const paths = backend.tracks.map((t) => t.path);
    render(<App />);
    backend.open.mockImplementation((path: string) => {
      backend.status = {
        ...playing,
        positionSecs: 0,
        track: { ...playing.track!, path, durationSecs: 200 },
      };
    });
    const first = await screen.findByText("Birinci Şarkı");
    await act(async () => fireEvent.doubleClick(first));
    await waitFor(() => expect(backend.setNext).toHaveBeenLastCalledWith(paths[1]));
    const opened = backend.open.mock.calls.length;

    // Çekirdek iki kez kendisi geçer: 1 → 2 → 3. Sıra her geçişte ilerlemeli.
    for (const [now, upcoming] of [
      [paths[1], paths[2]],
      [paths[2], paths[3]],
    ] as const) {
      backend.status = {
        ...backend.status!,
        positionSecs: 0.2,
        track: { ...backend.status!.track!, path: now! },
      };
      await waitFor(() => expect(backend.setNext).toHaveBeenLastCalledWith(upcoming), {
        timeout: 2000,
      });
      expect(screen.getByRole("button", { name: "Sonraki" })).toBeEnabled();
    }
    expect(backend.open).toHaveBeenCalledTimes(opened);
  });

  it("arama kutusuna yazılanla arar ve klasör kaldırılabilir", async () => {
    backend.desktop = true;
    backend.library = library;
    render(<App />);
    const search = await screen.findByRole("searchbox", { name: "Kütüphanede ara" });
    await act(async () => fireEvent.change(search, { target: { value: "sebnem" } }));
    await waitFor(() => expect(backend.search).toHaveBeenLastCalledWith("sebnem"));
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: /Kütüphaneden çıkar/ })),
    );
    expect(backend.removeFolder).toHaveBeenCalledWith(7);
  });
});

describe("ekolayzer", () => {
  /** Üst çubuktaki ekolayzer düğmesinin ışığı: ses değiştiriliyorsa yanar. */
  const eqLight = () => screen.getByRole("button", { name: "Ekolayzer" }).querySelector(".led");
  const openEq = async () => {
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: /Ekolayzer/ })));
  };

  it("sekmeyle açılır; hazır ayar sürgüleri ve ekrandaki EQ ışığını değiştirir", async () => {
    await openEq();
    expect(screen.getByRole("tab", { name: /Ekolayzer/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("button", { name: "Düz" })).toHaveAttribute("aria-pressed", "true");

    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Bas" })));
    await waitFor(() =>
      expect(backend.setEq).toHaveBeenLastCalledWith({
        enabled: true,
        gainsDb: [0, 4, 3, 1, 0, 0, 0, 0, 0, 0],
        bassDb: 6,
        smallSpeaker: false,
        bassDepth: 0,
        bassPunch: 0.3,
      }),
    );
    expect(screen.getByRole("slider", { name: "62 Hz" })).toHaveAttribute("aria-valuenow", "4");
    // "Bas" hazır ayarı bas düğmesini de çevirir.
    expect(screen.getByRole("slider", { name: "Bas" })).toHaveValue("6");
    expect(screen.getByRole("button", { name: "Bas" })).toHaveAttribute("aria-pressed", "true");
    expect(eqLight()).toHaveClass("is-on");
    // Tarayıcı önizlemesinde şarkının boşluğu bilinmez: en kötü durum gösterilir
    // (bantların en büyüğü 4 + bas düğmesi 6 + vuruşun en yüksek anı %30 × 8 = 2,4).
    expect(await screen.findByText(/Bozulma koruması: −12,4 dB/)).toBeInTheDocument();
  });

  it("bas kulüp bölgesine çıkar; derinlik ayarlanır, küçük hoparlörde kapanır", async () => {
    await openEq();
    const bass = screen.getByRole("slider", { name: "Bas" });
    expect(bass).toHaveAttribute("max", "18");
    expect(screen.queryByText("Kulüp", { selector: ".bass__badge" })).not.toBeInTheDocument();
    await act(async () => fireEvent.change(bass, { target: { value: "15" } }));
    expect(screen.getByText("Kulüp", { selector: ".bass__badge" })).toBeInTheDocument();
    expect(screen.getByText(/Kulüp düzeyi: bas her şeyin önünde/)).toBeInTheDocument();

    const depth = screen.getByRole("slider", { name: "Derinlik" });
    await act(async () => fireEvent.change(depth, { target: { value: "0.6" } }));
    expect(depth).toHaveAttribute("aria-valuetext", "%60");
    await waitFor(() =>
      expect(backend.setEq).toHaveBeenLastCalledWith(
        expect.objectContaining({ bassDb: 15, bassDepth: 0.6 }),
      ),
    );

    // "Kulüp" hazır ayarı ikisini birlikte ayarlar.
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Kulüp" })));
    expect(bass).toHaveValue("14");
    expect(depth).toHaveValue("0.7");
    expect(screen.getByRole("button", { name: "Kulüp" })).toHaveAttribute("aria-pressed", "true");

    // Küçük hoparlörde alt oktav çalınamaz: derinlik kapanır.
    await act(async () =>
      fireEvent.click(screen.getByRole("switch", { name: "Küçük hoparlör bası" })),
    );
    expect(depth).toBeDisabled();
    expect(screen.getByText(/Derinlik küçük hoparlörde kapalı/)).toBeInTheDocument();
  });

  it("vuruş ayarlanır; kulüp hazır ayarında güçlü, küçük hoparlörde de açık", async () => {
    await openEq();
    const punch = screen.getByRole("slider", { name: "Vuruş" });
    expect(punch).toHaveValue("0");
    expect(screen.getByText(/vuruş göğse çarpar, sürekli bas şişmez/)).toBeInTheDocument();
    await act(async () => fireEvent.change(punch, { target: { value: "0.8" } }));
    expect(punch).toHaveAttribute("aria-valuetext", "%80");
    expect(eqLight()).toHaveClass("is-on");
    await waitFor(() =>
      expect(backend.setEq).toHaveBeenLastCalledWith(expect.objectContaining({ bassPunch: 0.8 })),
    );
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Kulüp" })));
    expect(punch).toHaveValue("0.7");
    // Vuruş küçük hoparlörde de çalışır (harmonikler de vuruşla güçlenir).
    await act(async () =>
      fireEvent.click(screen.getByRole("switch", { name: "Küçük hoparlör bası" })),
    );
    expect(punch).not.toBeDisabled();
  });

  it("bas düğmesi ve küçük hoparlör bası ayarlanır; hazır ayar 'özel'e döner", async () => {
    await openEq();
    const bass = screen.getByRole("slider", { name: "Bas" });
    expect(bass).toHaveValue("0");
    expect(eqLight()).not.toHaveClass("is-on");
    await act(async () => fireEvent.change(bass, { target: { value: "8.5" } }));
    expect(bass).toHaveValue("8.5");
    expect(bass).toHaveAttribute("aria-valuetext", "+8,5 dB");
    expect(screen.getByText("Özel ayar")).toBeInTheDocument();
    expect(eqLight()).toHaveClass("is-on");
    await waitFor(() =>
      expect(backend.setEq).toHaveBeenLastCalledWith(expect.objectContaining({ bassDb: 8.5 })),
    );

    const small = screen.getByRole("switch", { name: "Küçük hoparlör bası" });
    expect(small).toHaveAttribute("aria-checked", "false");
    await act(async () => fireEvent.click(small));
    expect(small).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText(/çalınamayan alt bas süzülür/)).toBeInTheDocument();
    await waitFor(() =>
      expect(backend.setEq).toHaveBeenLastCalledWith(
        expect.objectContaining({ bassDb: 8.5, smallSpeaker: true }),
      ),
    );

    // "Küçük hoparlör" hazır ayarı ikisini birlikte ayarlar.
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Küçük hoparlör" })));
    expect(bass).toHaveValue("8");
    expect(screen.getByRole("button", { name: "Küçük hoparlör" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("sürgü klavyeyle ayarlanır, ok tuşları şarkıyı sarmaz", async () => {
    backend.desktop = true;
    backend.status = playing;
    await openEq();
    const slider = screen.getByRole("slider", { name: "1kHz" });
    slider.focus();
    await act(async () => fireEvent.keyDown(slider, { key: "ArrowUp" }));
    await act(async () => fireEvent.keyDown(slider, { key: "PageUp" }));
    expect(slider).toHaveAttribute("aria-valuenow", "3.5");
    expect(screen.getByText("Özel ayar")).toBeInTheDocument();

    // ← / → komşu banda geçer; şarkı sarılmaz.
    await act(async () => fireEvent.keyDown(slider, { key: "ArrowRight" }));
    expect(screen.getByRole("slider", { name: "2kHz" })).toHaveFocus();
    expect(backend.seek).not.toHaveBeenCalled();

    // Çift tıklama sıfırlar; kapatınca ayar korunur ama ışık söner.
    await act(async () => fireEvent.doubleClick(slider));
    expect(slider).toHaveAttribute("aria-valuenow", "0");
    await act(async () => fireEvent.keyDown(slider, { key: "Home" }));
    expect(slider).toHaveAttribute("aria-valuenow", "12");
    await act(async () => fireEvent.click(screen.getByRole("switch", { name: "Açık" })));
    expect(screen.getByRole("switch", { name: "Kapalı" })).toHaveAttribute("aria-checked", "false");
    expect(slider).toHaveAttribute("aria-valuenow", "12");
    expect(eqLight()).not.toHaveClass("is-on");
    await waitFor(() =>
      expect(backend.setEq).toHaveBeenLastCalledWith(expect.objectContaining({ enabled: false })),
    );
  });

  it("işaretçinin konumunu kazanca çevirir", async () => {
    const { pointerToGain } = await import("./EqualizerPanel");
    expect(pointerToGain(100, 100, 200)).toBe(12);
    expect(pointerToGain(300, 100, 200)).toBe(-12);
    expect(pointerToGain(200, 100, 200)).toBe(0);
    expect(pointerToGain(0, 100, 200)).toBe(12);
    expect(pointerToGain(5, 0, 0)).toBe(0);
  });
});

describe("sahneler", () => {
  afterEach(() => window.localStorage.clear());

  it("açılışta gece göğü seçilidir; üstteki düğmeyle VU ibrelerine geçer ve seçimi hatırlar", async () => {
    render(<App />);
    expect(screen.getByRole("button", { name: "Gece göğü" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "VU ibreleri" })));
    expect(screen.getByRole("img", { name: "Sol kanal VU ölçer" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Sağ kanal VU ölçer" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "VU ibreleri" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "Gece göğü" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(window.localStorage.getItem("lyraska.scene")).toBe("vu");
  });

  it("1–4 tuşları sahne seçer; arama kutusuna yazarken seçmez", async () => {
    render(<App />);
    await act(async () => fireEvent.keyDown(window, { key: "3" }));
    expect(screen.getByRole("img", { name: "Sol kanal VU ölçer" })).toBeInTheDocument();
    await act(async () => fireEvent.keyDown(window, { key: "4" }));
    const spectrum = screen.getByRole("button", { name: "Spektrum" });
    expect(spectrum).toHaveAttribute("aria-pressed", "true");
    expect(window.localStorage.getItem("lyraska.scene")).toBe("spectrum");
    const search = screen.getByRole("searchbox", { name: "Kütüphanede ara" });
    await act(async () => fireEvent.keyDown(search, { key: "3" }));
    expect(spectrum).toHaveAttribute("aria-pressed", "true");
  });

  it("gece göğü: WebGL2 yoksa durgun gök gösterir ve nedeni yazar", async () => {
    // jsdom'da WebGL yok: tuval bağlamı alınamaz, sahne çökmeden durgun göğe düşer.
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockImplementation(() => null);
    window.localStorage.setItem("lyraska.scene", "vu");
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Gece göğü" })));
    const sky = screen.getByRole("img", { name: /Gece göğü/ });
    expect(sky).toHaveAttribute("data-webgl", "off");
    // Neden tuvale işlenir ve hata günlüğüne yazılır (kullanıcı bize iletebilir).
    expect(sky.dataset.webglError).toMatch(/WebGL2 açılamadı/);
    expect(getContext).toHaveBeenCalledWith("webgl2", expect.anything());
    expect(window.localStorage.getItem("lyraska.scene")).toBe("sky");
    getContext.mockRestore();
  });

  it("gece otoyolu: WebGL2 yoksa durgun görüntü kalır", async () => {
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockImplementation(() => null);
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Gece otoyolu" })));
    const road = screen.getByRole("img", { name: /Gece otoyolu/ });
    expect(road).toHaveAttribute("data-webgl", "off");
    expect(road.dataset.webglError).toMatch(/WebGL2 açılamadı/);
    expect(window.localStorage.getItem("lyraska.scene")).toBe("highway");
    getContext.mockRestore();
  });

  it("otoyol görsel veriden vuruşu ve Yönetmen notunu alır", async () => {
    const { highwayInput } = await import("./HighwayScene");
    const base = {
      positionSecs: 1,
      bands: Array(32).fill(0.8),
      rmsDb: [-20, -20] as [number, number],
      peakDb: [-6, -6] as [number, number],
      vuReferenceDb: null,
      energy: null,
      section: null,
    };
    expect(highwayInput({ ...base, beat: null, director: null })).toMatchObject({
      beat: null,
      director: null,
    });
    const input = highwayInput({
      ...base,
      beat: { bpm: 128, index: 3, phase: 0.25, barBeat: 4, meter: 4 },
      director: {
        atmosphere: { section: 1, theme: 2, mood: 0.7, warmth: 0.5 },
        rhythm: {
          pulse: 0,
          accent: 0,
          beatPhase: 0.25,
          barPhase: 0.81,
          anticipation: 0.4,
          release: 0,
        },
        texture: { detail: 0.5, motion: 0.5 },
      },
    });
    expect(input.beat).toEqual({ bpm: 128, phase: 0.25 });
    expect(input.director).toEqual({
      mood: 0.7,
      theme: 2,
      barPhase: 0.81,
      anticipation: 0.4,
      release: 0,
    });
    expect(input.energies[0]).toBeCloseTo(1, 5);
  });

  it("epilepsi güvenli modu ayarlarda açılır, kaydedilir ve ekranda belirtilir", async () => {
    backend.safe = false;
    backend.desktop = true;
    backend.status = playing;
    render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Ayarlar" })));
    const toggle = await screen.findByRole("button", { name: /Epilepsi güvenli modu: Kapalı/ });
    expect(screen.queryByText("Güvenli mod")).not.toBeInTheDocument();
    await act(async () => fireEvent.click(toggle));
    expect(backend.setSafe).toHaveBeenCalledWith(true);
    expect(toggle).toHaveAttribute("aria-pressed", "true");
    expect(toggle).toHaveTextContent("Açık");
    expect(screen.getByText("Güvenli mod")).toBeInTheDocument();
    backend.safe = false;
  });

  it("VU ölçerler sese göre ilerler, susunca dinlenmeye döner", async () => {
    const { stepMeters } = await import("./VuScene");
    const frame = {
      positionSecs: 1,
      bands: [],
      rmsDb: [-14, -60] as [number, number],
      peakDb: [-3, -60] as [number, number],
      vuReferenceDb: -14,
      beat: null,
      energy: null,
      section: null,
      director: null,
    };
    let meters: Parameters<typeof stepMeters>[0] = {
      needles: [
        { position: 0, velocity: 0 },
        { position: 0, velocity: 0 },
      ],
      lamps: [0, 0],
    };
    for (let i = 0; i < 90; i++) meters = stepMeters(meters, frame, 1 / 60);
    // Sol kanal şarkının referans seviyesinde: 0 VU (ölçeğin ~%71'i). Sağ kanal sessiz.
    expect(meters.needles[0].position).toBeCloseTo(0.708, 2);
    expect(meters.needles[1].position).toBe(0);
    for (let i = 0; i < 90; i++) meters = stepMeters(meters, null, 1 / 60);
    expect(meters.needles[0].position).toBeLessThan(0.01);
  });
});

describe("senkron", () => {
  it("sekmede ses gecikmesi ayarlanır ve vuruş göstergesi görünür", async () => {
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Senkron" })));
    const slider = screen.getByRole("slider", { name: "Ses gecikmesi" });
    expect(slider).toHaveValue("0");
    await act(async () => fireEvent.change(slider, { target: { value: "185" } }));
    expect(screen.getByText("185 ms")).toBeInTheDocument();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "10 ms artır" })));
    expect(screen.getByText("195 ms")).toBeInTheDocument();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Sıfırla" })));
    expect(screen.getByText("0 ms")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Vuruş göstergesi" })).toBeInTheDocument();
    // Tarayıcı önizlemesinde ses çalınamaz: ölçüm başlatılamaz.
    expect(screen.getByRole("button", { name: "Ölçümü başlat" })).toBeDisabled();
  });
});

describe("şarkı haritası ve bilgi kartları", () => {
  it("şeritte bölümleri ve drop'ları gösterir, drop yaklaşınca geri sayar", async () => {
    backend.desktop = true;
    backend.status = {
      ...playing,
      state: "paused",
      bpm: 128,
      output: {
        deviceName: "Hoparlörler",
        sampleRate: 48000,
        channels: 2,
        resampled: true,
        normalizationDb: null,
        bitPerfect: false,
        bitDepth: null,
        notice: null,
      },
    };
    backend.songMap = {
      meter: 4,
      downbeatPhase: 0,
      downbeats: [],
      sections: [
        { start: 0, end: 60, energy: 0.3, label: 0 },
        { start: 60, end: 150, energy: 0.9, label: 1 },
        { start: 150, end: 225, energy: 0.5, label: 0 },
      ],
      drops: [60, 100],
      energy: [],
    };
    const { container } = render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    const sections = () => [
      ...container.querySelectorAll(".songmap__layer--future .songmap__section"),
    ];
    await waitFor(() => expect(sections()).toHaveLength(3));
    // Benzer bölümler (aynı etiket) aynı renk temasını alır.
    const [first, second, third] = sections().map((e) => e.className);
    expect(first).toBe(third);
    expect(first).not.toBe(second);
    expect(container.querySelectorAll(".songmap__drop")).toHaveLength(2);
    // 83,4. saniyede sıradaki drop 100. saniyede: 17 saniye kala sayaç görünür.
    expect(screen.getByText("Drop yaklaşıyor")).toBeInTheDocument();
    expect(screen.getByText("17 saniye sonra")).toBeInTheDocument();
    expect(screen.getByText("128 BPM · 4/4")).toBeInTheDocument();
    expect(screen.getByText("FLAC 44,1 → 48 kHz")).toBeInTheDocument();
    // Arayüzün vurgu renkleri çalan bölümün temasında (83,4 sn: ikinci bölüm, etiket 1).
    expect(container.querySelector("main")).toHaveAttribute("data-theme", "1");
  });

  it("analiz bitmeden şerit düz çubuktur, sayaç görünmez", async () => {
    backend.desktop = true;
    backend.status = { ...playing, state: "paused" };
    const { container } = render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    await waitFor(() => expect(container.querySelector(".seekbar__line")).toBeInTheDocument());
    expect(container.querySelector(".songmap")).not.toBeInTheDocument();
    expect(screen.queryByText("Drop yaklaşıyor")).not.toBeInTheDocument();
  });

  it("çalma hatası başlığın altında gösterilir", async () => {
    backend.desktop = true;
    backend.status = { ...playing, state: "error", error: "Ses aygıtı bulunamadı." };
    render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Ses aygıtı bulunamadı.");
    expect(screen.getByText("Çalınamadı")).toBeInTheDocument();
  });
});

describe("sinema görünümü", () => {
  afterEach(() => vi.useRealTimers());

  it("çalarken fare kıpırdamazsa düğmeler çekilir, kıpırdayınca geri gelir", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    backend.desktop = true;
    backend.status = playing;
    const { container } = render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    const app = container.querySelector(".app")!;
    // Çekmece açıkken çekilmez.
    await act(async () => vi.advanceTimersByTime(5000));
    expect(app).not.toHaveClass("is-cinema");
    await act(async () => fireEvent.keyDown(window, { key: "Escape" }));
    await act(async () => vi.advanceTimersByTime(3000));
    expect(app).not.toHaveClass("is-cinema");
    await act(async () => vi.advanceTimersByTime(1100));
    expect(app).toHaveClass("is-cinema");
    await act(async () => fireEvent.pointerMove(window));
    expect(app).not.toHaveClass("is-cinema");
  });
});

describe("pointerRatio", () => {
  it("işaretçi konumunu 0..1 aralığına çevirir", () => {
    expect(pointerRatio(150, 100, 200)).toBe(0.25);
    expect(pointerRatio(50, 100, 200)).toBe(0);
    expect(pointerRatio(400, 100, 200)).toBe(1);
    expect(pointerRatio(10, 0, 0)).toBe(0);
  });
});

describe("stateLabel", () => {
  it("başlığın üstünde çalma durumunu yazar", () => {
    expect(stateLabel({ ...playing, track: null, state: "idle" })).toBe("Hoş geldiniz");
    expect(stateLabel(playing)).toBe("Şimdi çalıyor");
    expect(stateLabel({ ...playing, state: "paused" })).toBe("Duraklatıldı");
    // Durdur düğmesi başa sarıp duraklatır.
    expect(stateLabel({ ...playing, state: "paused", positionSecs: 0 })).toBe("Durduruldu");
    expect(stateLabel({ ...playing, state: "ended" })).toBe("Bitti");
    expect(stateLabel({ ...playing, state: "error" })).toBe("Çalınamadı");
  });
});

describe("marqueeWindow", () => {
  const source = textToColumns("AB");

  it("istenen genişlikte pencere döndürür", () => {
    expect(marqueeWindow(source, 0, 20)).toHaveLength(20);
  });

  it("başlangıçta ekran boştur, metin sağdan girer", () => {
    const view = marqueeWindow(source, 0, 20);
    expect(view.every((column) => column.every((dot) => !dot))).toBe(true);
  });

  it("ekran genişliği kadar kaydırınca metnin başı solda görünür", () => {
    expect(marqueeWindow(source, 20, 20)[0]).toEqual(source[0]);
  });

  it("döngüsel olarak tekrar eder", () => {
    const cycle = source.length + 20;
    expect(marqueeWindow(source, 5, 20)).toEqual(marqueeWindow(source, 5 + cycle, 20));
  });
});

describe("spectrumColumns", () => {
  it("seviyeyi alttan yukarı doğru doldurur ve tepe noktasını işaretler", () => {
    const columns = spectrumColumns([0.5], [1], 10);
    expect(columns).toHaveLength(2); // tek bant, iki sütun genişliğinde
    const column = columns[0]!;
    expect(column.filter(Boolean)).toHaveLength(6); // 5 seviye + 1 tepe
    expect(column[0]).toBe(true); // tepe en üstte
    expect(column[9]).toBe(true); // en alt yanık
    expect(column[4]).toBe(false);
  });

  it("aralık dışı değerleri sınırlar", () => {
    const [column] = spectrumColumns([2, -1], [0, 0], 4);
    expect(column!.every(Boolean)).toBe(true);
  });
});

describe("kulaklık düzeltmesi", () => {
  afterEach(() => {
    backend.headphone = null;
    backend.pickProfile.mockReset();
    backend.importProfile.mockReset();
  });

  it("AutoEq profili yüklenir, açılıp kapanır ve kaldırılır", async () => {
    backend.pickProfile.mockResolvedValue("C:\\İndirilenler\\Sennheiser HD 600 ParametricEQ.txt");
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: /Ekolayzer/ })));
    expect(screen.getByText(/autoeq\.app/)).toBeInTheDocument();

    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Profil yükle" })));
    expect(backend.importProfile).toHaveBeenCalledWith(
      "C:\\İndirilenler\\Sennheiser HD 600 ParametricEQ.txt",
    );
    expect(screen.getByText("Sennheiser HD 600")).toBeInTheDocument();
    expect(screen.getByText(/1 filtre · ön kazanç −6,4 dB/)).toBeInTheDocument();
    const toggle = screen.getByRole("switch", { name: "Kulaklık düzeltmesi" });
    expect(toggle).toHaveAttribute("aria-checked", "true");

    await act(async () => fireEvent.click(toggle));
    expect(screen.getByRole("switch", { name: "Kulaklık düzeltmesi" })).toHaveAttribute(
      "aria-checked",
      "false",
    );

    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Kaldır" })));
    expect(screen.queryByText("Sennheiser HD 600")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Profil yükle" })).toBeInTheDocument();
  });

  it("vazgeçilirse bir şey olmaz; bozuk dosyada anlaşılır hata gösterir", async () => {
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: /Ekolayzer/ })));
    backend.pickProfile.mockResolvedValue(null);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Profil yükle" })));
    expect(backend.importProfile).not.toHaveBeenCalled();

    backend.pickProfile.mockResolvedValue("C:\\bozuk.txt");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Profil yükle" })));
    expect(screen.getByText(/kulaklık düzeltmesi bulunamadı/)).toBeInTheDocument();
  });
});

describe("hata sınırları", () => {
  afterEach(() => {
    backend.brokenEqualizer = false;
    vi.restoreAllMocks();
  });

  it("bir panel çökerse yalnızca o panel 'açılamadı' der; sahne ve diğer paneller çalışır", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    backend.brokenEqualizer = true;
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: /Ekolayzer/ })));
    expect(await screen.findByText(/Ekolayzer açılamadı/)).toBeInTheDocument();
    expect(backend.logError).toHaveBeenCalledWith(expect.stringMatching(/^Ekolayzer çizilemedi/));
    // Sahne yerinde, diğer paneller açılıyor.
    expect(document.querySelector(".stage")).not.toBeNull();
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Ayarlar" })));
    expect(screen.getByRole("button", { name: "Göl" })).toBeInTheDocument();
  });
});

describe("gece göğünün manzarası", () => {
  it("ayarlardan seçilir ve hatırlanır", async () => {
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Ayarlar" })));
    const lake = screen.getByRole("button", { name: "Göl" });
    expect(lake).toHaveAttribute("aria-pressed", "true");
    expect(lake).toHaveAccessibleDescription(/durgun gölde yansımaları/);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Korona" })));
    expect(screen.getByRole("button", { name: "Korona" })).toHaveAttribute("aria-pressed", "true");
    expect(lake).toHaveAttribute("aria-pressed", "false");
    expect(window.localStorage.getItem("lyraska.skyLook")).toBe("corona");
  });
});

describe("hata günlüğü", () => {
  afterEach(() => {
    backend.desktop = false;
    backend.openLog.mockReset();
  });

  it("programda düğmeyle açılır; açılamazsa nedenini söyler", async () => {
    backend.desktop = true;
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Ayarlar" })));
    const button = await screen.findByRole("button", { name: "Hata günlüğü" });
    await act(async () => fireEvent.click(button));
    expect(backend.openLog).toHaveBeenCalledTimes(1);

    backend.openLog.mockRejectedValue("Hata günlüğü açılamadı: erişim yok");
    await act(async () => fireEvent.click(button));
    expect(screen.getByText("Hata günlüğü açılamadı: erişim yok")).toBeInTheDocument();
  });

  it("tarayıcı önizlemesinde gösterilmez", () => {
    render(<App />);
    expect(screen.queryByRole("button", { name: "Hata günlüğü" })).not.toBeInTheDocument();
  });
});

describe("işaretleme", () => {
  afterEach(() => {
    backend.desktop = false;
    backend.status = null;
    backend.annotation = null;
    backend.songMap = null;
    backend.saveMarks.mockReset();
    backend.toggle.mockReset();
  });

  it("Boşluk beat, D drop işaretler; Geri siler; kendiliğinden kaydeder; Esc bitirir", async () => {
    backend.desktop = true;
    backend.status = playing;
    render(<App />);
    // Oynatıcı çalan şarkıyı ilk komutla öğrenir (diğer testlerdeki gibi).
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    backend.toggle.mockReset();
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "İşaretle" })));
    const start = await screen.findByRole("button", { name: /İşaretlemeye başla/ });
    await waitFor(() => expect(start).toBeEnabled());
    await act(async () => fireEvent.click(start));

    await act(async () => fireEvent.keyDown(document.body, { code: "Space", key: " " }));
    await act(async () => fireEvent.keyDown(document.body, { code: "KeyD", key: "d" }));
    expect(backend.toggle).not.toHaveBeenCalled(); // Boşluk çal/duraklat yapmadı
    const counts = () => screen.getByText(/beat$/, { selector: ".marker__stat" }).textContent;
    expect(counts()).toMatch(/^1 beat/);
    expect(screen.getByText(/drop/, { selector: ".marker__stat" }).textContent).toMatch(/^1 drop/);

    await act(async () =>
      fireEvent.keyDown(document.body, { code: "Backspace", key: "Backspace" }),
    );
    expect(screen.getByText(/drop/, { selector: ".marker__stat" }).textContent).toMatch(/^0 drop/);

    await waitFor(() => expect(backend.saveMarks).toHaveBeenCalled(), { timeout: 2000 });
    const [path, beats, drops] = backend.saveMarks.mock.calls.at(-1)!;
    expect(path).toBe(playing.track!.path);
    expect(beats).toHaveLength(1);
    expect(drops).toEqual([]);
    await waitFor(() => expect(screen.getByText("Kaydedildi")).toBeInTheDocument());

    await act(async () => fireEvent.keyDown(document.body, { code: "Escape", key: "Escape" }));
    expect(screen.getByRole("button", { name: /İşaretlemeye başla/ })).toBeInTheDocument();
    await act(async () => fireEvent.keyDown(document.body, { code: "Space", key: " " }));
    expect(backend.toggle).toHaveBeenCalledTimes(1); // artık yine çal/duraklat
  });

  /** İşaretleme sekmesini açar ve işaretlemeyi başlatır. */
  async function startMarking() {
    backend.desktop = true;
    backend.status = playing;
    render(<App />);
    // Oynatıcı çalan şarkıyı ilk komutla öğrenir (diğer testlerdeki gibi).
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "İşaretle" })));
    const start = await screen.findByRole("button", { name: /İşaretlemeye başla/ });
    await waitFor(() => expect(start).toBeEnabled());
    await act(async () => fireEvent.click(start));
  }

  /** Bir vuruş aralığı arayla `count` kez Boşluk'a basar (sürekli işaretleme). */
  async function tapBeats(count: number, intervalMs: number) {
    for (let i = 0; i < count; i++) {
      // Sahte çekirdek de şarkının ilerlediğini bildirsin (yoksa vuruşlar üst üste düşer).
      const status = backend.status!;
      backend.status = { ...status, positionSecs: status.positionSecs + intervalMs / 1000 };
      await act(async () => fireEvent.keyDown(document.body, { code: "Space", key: " " }));
      await act(() => new Promise((resolve) => setTimeout(resolve, intervalMs)));
    }
  }

  it("şarkı boyunca durmadan işaretlerken de ara ara kaydeder", async () => {
    await startMarking();
    // 0,25 sn arayla 16 vuruş (4 sn): her vuruş kaydı ertelese hiç kaydedilmezdi.
    await tapBeats(16, 250);
    expect(backend.saveMarks).toHaveBeenCalled();
    const [, beats] = backend.saveMarks.mock.calls.at(-1)!;
    expect(beats.length).toBeGreaterThanOrEqual(8);
  }, 10_000);

  it("şarkı değişince kaydedilmemiş işaretler önceki şarkıya yazılır", async () => {
    await startMarking();
    await tapBeats(4, 150);
    expect(backend.saveMarks).not.toHaveBeenCalled(); // henüz kaydedilmedi
    // Sıradaki şarkı başladı (boşluksuz geçiş ya da "Sonraki").
    backend.status = {
      ...playing,
      positionSecs: 0.1,
      track: { ...playing.track!, path: "C:\\Müzik\\sonraki.flac" },
    };
    await waitFor(
      () =>
        expect(backend.saveMarks).toHaveBeenCalledWith(playing.track!.path, expect.any(Array), []),
      { timeout: 2000 },
    );
    const [, beats] = backend.saveMarks.mock.calls.find(([p]) => p === playing.track!.path)!;
    expect(beats).toHaveLength(4);
  });

  it("kayıtlı işaretleri yükler ve doğruluğu gösterir", async () => {
    backend.desktop = true;
    backend.status = playing;
    backend.annotation = {
      format: 1,
      appVersion: "0.0.25",
      track: {
        path: track.path,
        fileName: track.fileName,
        title: track.title,
        artist: track.artist,
        durationSecs: track.durationSecs,
        sizeBytes: null,
      },
      savedAt: "",
      beats: Array.from({ length: 12 }, (_, i) => 1 + i * 0.5),
      drops: [61],
      evaluation: null,
    };
    backend.songMap = {
      meter: 4,
      downbeatPhase: 0,
      downbeats: [],
      sections: [
        { start: 0, end: 60, energy: 0.3, label: 0 },
        { start: 60, end: 225, energy: 0.9, label: 1 },
      ],
      drops: [61.5],
      energy: [],
    };
    render(<App />);
    // Oynatıcı çalan şarkıyı ilk komutla öğrenir (diğer testlerdeki gibi).
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    backend.toggle.mockReset();
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "İşaretle" })));
    await waitFor(() =>
      expect(screen.getByText(/beat/, { selector: ".marker__stat" }).textContent).toMatch(
        /^12 beat · ~120 BPM/,
      ),
    );
    expect(screen.getByText(/01:01/)).toBeInTheDocument();
    // Diskteki, değişmemiş işaretler doğrudan ölçülebilir.
    const measure = screen.getByRole("button", { name: "Doğruluğu ölç" });
    await waitFor(() => expect(measure).toBeEnabled());
    await act(async () => fireEvent.click(measure));
    expect(screen.getByText("%87,3")).toBeInTheDocument();
    expect(screen.getByText("42 ms")).toBeInTheDocument();
    expect(screen.getByText("%95,1")).toBeInTheDocument();
    expect(screen.getByText("128 / 127,9 BPM")).toBeInTheDocument();
    expect(screen.getByText("1 / 1")).toBeInTheDocument(); // drop isabeti
    expect(
      screen.getByRole("img", {
        name: "Şarkı haritası: 2 bölüm, programın bulduğu 1 drop, sizin 1 drop işaretiniz",
      }),
    ).toBeInTheDocument();

    // Hepsi silinince kaydedilir; 8 beat'ten az olduğu için ölçülemez.
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Hepsini sil" })));
    expect(screen.queryByText("%87,3")).not.toBeInTheDocument();
    await waitFor(
      () => expect(backend.saveMarks).toHaveBeenLastCalledWith(expect.any(String), [], []),
      {
        timeout: 2000,
      },
    );
    expect(screen.getByRole("button", { name: "Doğruluğu ölç" })).toBeDisabled();
  });
});

describe("bit-perfect", () => {
  const bitPerfectOutput = {
    deviceName: "Kulaklık (USB DAC)",
    sampleRate: 44100,
    channels: 2,
    resampled: false,
    normalizationDb: null,
    bitPerfect: true,
    bitDepth: 24,
    notice: null,
  };

  it("ayarlardan açılır; uyarı ve açıklama görünür", async () => {
    render(<App />);
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Ayarlar" })));
    const toggle = await screen.findByRole("button", { name: "Bit-perfect (özel mod): Kapalı" });
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByText(/Windows ses düzeyi çoğu aygıtta etkisizdir/)).toBeInTheDocument();
    await act(async () => fireEvent.click(toggle));
    expect(screen.getByRole("button", { name: "Bit-perfect (özel mod): Açık" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    // Eşitleme ayarı yerinde kalır.
    expect(
      screen.getByRole("button", { name: "Ses yüksekliği eşitleme: Açık" }),
    ).toBeInTheDocument();
  });

  it("çalarken sesin yolunu söyler; ekolayzer devre dışı görünür", async () => {
    backend.desktop = true;
    backend.status = { ...playing, state: "paused", output: bitPerfectOutput };
    render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    await waitFor(() =>
      expect(screen.getByText("FLAC 44,1 kHz · bit-perfect")).toBeInTheDocument(),
    );
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Ayarlar" })));
    expect(
      screen.getByText("FLAC 44,1 kHz → bit-perfect (özel mod, 24 bit) → Kulaklık (USB DAC)"),
    ).toBeInTheDocument();
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Ekolayzer" })));
    expect(screen.getByText(/Bit-perfect açık: ses hiç işlenmiyor/)).toBeInTheDocument();
  });

  it("açılamadıysa nedenini söyler", async () => {
    backend.desktop = true;
    backend.status = {
      ...playing,
      state: "paused",
      output: {
        ...bitPerfectOutput,
        sampleRate: 48000,
        resampled: true,
        bitPerfect: false,
        bitDepth: null,
        notice: "aygıtı başka bir program özel modda kullanıyor",
      },
    };
    render(<App />);
    await act(async () => fireEvent.keyDown(window, { code: "Space", key: " " }));
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Ayarlar" })));
    await waitFor(() =>
      expect(
        screen.getByText(
          "Bit-perfect açılamadı: aygıtı başka bir program özel modda kullanıyor. Ses normal yoldan çalıyor.",
        ),
      ).toBeInTheDocument(),
    );
    await act(async () => fireEvent.click(screen.getByRole("tab", { name: "Ekolayzer" })));
    expect(screen.queryByText(/Bit-perfect açık/)).not.toBeInTheDocument();
  });
});
