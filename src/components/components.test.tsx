import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { PlaybackStatus, TrackInfo } from "../lib/backend";
import { marqueeWindow } from "./Marquee";
import { spectrumColumns } from "./SpectrumDemo";
import { textToColumns } from "../lib/dotFont";

// Rust çekirdeğini taklit eden sahte arka uç.
const backend = vi.hoisted(() => ({
  desktop: false,
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
  drop: null as ((paths: string[]) => void) | null,
  setEq: vi.fn(),
  headphone: null as import("../lib/backend").HeadphoneState | null,
  pickProfile: vi.fn(),
  importProfile: vi.fn(),
  setNext: vi.fn(),
  openLog: vi.fn(),
  saveMarks: vi.fn(),
  annotation: null as import("../lib/backend").Annotation | null,
  songMap: null as import("../lib/backend").SongMap | null,
}));

vi.mock("../lib/backend", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/backend")>();
  const current = () => backend.status ?? actual.IDLE_STATUS;
  return {
    ...actual,
    isDesktop: () => backend.desktop,
    getAppInfo: async () => ({ ...actual.BROWSER_FALLBACK, supportedExtensions: ["mp3", "flac"] }),
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
    onFileDrop: async (handler: (paths: string[]) => void) => {
      backend.drop = handler;
      return () => {
        if (backend.drop === handler) backend.drop = null;
      };
    },
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
const { headline, marqueeText } = await import("./PlayerScreen");
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
  backend.drop = null;
  vi.clearAllMocks();
});

describe("oynatıcı ekranı", () => {
  it("tarayıcı önizlemesinde program adını gösterir, düğmeler kapalıdır", async () => {
    render(<App />);
    expect(screen.getByRole("img", { name: "Lyraska" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Dosya aç/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Çal/ })).toBeDisabled();
    await waitFor(() => expect(screen.getByText(/Faz 1 · Sürüm/)).toBeInTheDocument());
    expect(screen.getByText(/Windows'ta açın/)).toBeInTheDocument();
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
    await waitFor(() =>
      expect(screen.getByRole("img", { name: "Konum 01:23" })).toBeInTheDocument(),
    );
    expect(screen.getByText("01:23 / 03:45")).toBeInTheDocument();
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
    expect(screen.getByText("01:38 / 03:45")).toBeInTheDocument();
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
    expect(screen.getByText("02:30 / 03:45")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Konum 02:30" })).toBeInTheDocument();
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
  });
  const library = {
    folders: [{ id: 7, path: "C:\\Müzik" }],
    trackCount: 3,
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

  it("pencereye bırakılan klasörü kütüphaneye ekler, şarkıları sırayla çalar", async () => {
    backend.desktop = true;
    render(<App />);
    await screen.findByText("Müziğinizi ekleyin");
    // Desteklenen uzantılar yüklendikten sonra bırakılır.
    await waitFor(() => expect(backend.drop).not.toBeNull());
    await act(async () => {
      backend.drop!(["C:\\Müzik\\Rock", "C:\\İndirilenler\\a.mp3", "C:\\İndirilenler\\b.MP3"]);
      await Promise.resolve();
    });
    await waitFor(() => expect(backend.addFolder).toHaveBeenCalledWith("C:\\Müzik\\Rock"));
    await waitFor(() => expect(backend.open).toHaveBeenCalledWith("C:\\İndirilenler\\a.mp3"));
    expect(backend.open).toHaveBeenCalledTimes(1);
  });

  it("şarkıları listeler, çift tıklayınca çalar, bitince sıradakine geçer", async () => {
    backend.desktop = true;
    backend.library = library;
    backend.tracks = [song(1, "Mayın Tarlası"), song(2, "Bir Kedi Gördüm"), song(3, "Hoşçakal")];
    render(<App />);
    const first = await screen.findByText("Mayın Tarlası");

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
        gainsDb: [6, 5.5, 4.5, 2.5, 0.5, 0, 0, 0, 0, 0],
      }),
    );
    expect(screen.getByRole("slider", { name: "31 Hz" })).toHaveAttribute("aria-valuenow", "6");
    expect(screen.getByRole("button", { name: "Bas" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("EQ")).toHaveClass("is-on");
    expect(await screen.findByText(/Bozulma koruması: −6 dB/)).toBeInTheDocument();
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
    await act(async () => fireEvent.click(screen.getByRole("switch")));
    expect(screen.getByRole("switch")).toHaveAttribute("aria-checked", "false");
    expect(slider).toHaveAttribute("aria-valuenow", "12");
    expect(screen.getByText("EQ")).not.toHaveClass("is-on");
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

  it("sağdaki düğme VU ibrelerine geçer ve seçimi hatırlar", async () => {
    render(<App />);
    const knob = screen.getByRole("button", { name: /Sahne: Nokta matris spektrum/ });
    await act(async () => fireEvent.click(knob));
    expect(screen.getByRole("img", { name: "Sol kanal VU ölçer" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Sağ kanal VU ölçer" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Sahne: VU ibreleri/ })).toBeInTheDocument();
    expect(window.localStorage.getItem("lyraska.scene")).toBe("vu");
  });

  it("üçüncü basış gece göğüne geçer; WebGL2 yoksa durgun gök gösterir", async () => {
    // jsdom'da WebGL yok: tuval bağlamı alınamaz, sahne çökmeden durgun göğe düşer.
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockImplementation(() => null);
    render(<App />);
    const knob = screen.getByRole("button", { name: /Sahne:/ });
    await act(async () => fireEvent.click(knob));
    await act(async () => fireEvent.click(knob));
    const sky = screen.getByRole("img", { name: /Gece göğü/ });
    expect(sky).toHaveAttribute("data-webgl", "off");
    expect(getContext).toHaveBeenCalledWith("webgl2", expect.anything());
    expect(screen.getByRole("button", { name: /Sahne: Gece göğü/ })).toBeInTheDocument();
    expect(window.localStorage.getItem("lyraska.scene")).toBe("sky");
    await act(async () => fireEvent.click(knob));
    expect(
      screen.getByRole("button", { name: /Sahne: Nokta matris spektrum/ }),
    ).toBeInTheDocument();
    getContext.mockRestore();
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

describe("pointerRatio", () => {
  it("işaretçi konumunu 0..1 aralığına çevirir", () => {
    expect(pointerRatio(150, 100, 200)).toBe(0.25);
    expect(pointerRatio(50, 100, 200)).toBe(0);
    expect(pointerRatio(400, 100, 200)).toBe(1);
    expect(pointerRatio(10, 0, 0)).toBe(0);
  });
});

describe("headline", () => {
  it("boşta program adını, çalarken simge ve süreyi gösterir", () => {
    expect(headline({ ...playing, track: null, state: "idle" })).toBe("LYRASKA");
    expect(headline(playing)).toBe("▶ 01:23");
    expect(headline({ ...playing, state: "paused" })).toBe("‖ 01:23");
    expect(headline({ ...playing, state: "paused", positionSecs: 0 })).toBe("■ 00:00");
    expect(headline({ ...playing, state: "ended" })).toBe("■ 03:45");
  });
});

describe("marqueeText", () => {
  it("şarkı adını ve teknik bilgiyi, hata varsa hatayı gösterir", () => {
    expect(marqueeText(playing, null)).toBe("Lyra - Gece Otoyolu · FLAC · 44,1 kHz · Stereo ·");
    expect(marqueeText({ ...playing, state: "ended" }, null)).toContain("BİTTİ");
    expect(marqueeText(playing, "Dosya açılamadı")).toBe("HATA · Dosya açılamadı ·");
    // Tempo bulununca sona eklenir.
    expect(marqueeText({ ...playing, bpm: 128 }, null)).toBe(
      "Lyra - Gece Otoyolu · FLAC · 44,1 kHz · Stereo · 128 BPM ·",
    );
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

describe("hata günlüğü", () => {
  afterEach(() => {
    backend.desktop = false;
    backend.openLog.mockReset();
  });

  it("programda düğmeyle açılır; açılamazsa nedenini söyler", async () => {
    backend.desktop = true;
    render(<App />);
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

  it("kayıtlı işaretleri yükler ve doğruluğu gösterir", async () => {
    backend.desktop = true;
    backend.status = playing;
    backend.annotation = {
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
        { start: 0, end: 60, energy: 0.3 },
        { start: 60, end: 225, energy: 0.9 },
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
