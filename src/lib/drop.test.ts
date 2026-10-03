import { describe, expect, it } from "vitest";
import { splitDropped } from "./drop";

describe("splitDropped", () => {
  const extensions = ["mp3", "flac"];

  it("ses dosyalarını ve klasörleri ayırır", () => {
    const result = splitDropped(
      ["C:\\Müzik\\a.MP3", "C:\\Müzik\\Rock", "D:/b.flac", "C:\\Müzik 2.0"],
      extensions,
    );
    expect(result.tracks).toEqual(["C:\\Müzik\\a.MP3", "D:/b.flac"]);
    expect(result.folders).toEqual(["C:\\Müzik\\Rock", "C:\\Müzik 2.0"]);
  });

  it("desteklenmeyen dosyalar klasör yoluna düşer (çekirdek anlaşılır hata verir)", () => {
    expect(splitDropped(["C:\\notlar.txt"], extensions).folders).toEqual(["C:\\notlar.txt"]);
  });

  it("uzantı listesi henüz yoksa hepsini ses dosyası sayar", () => {
    expect(splitDropped(["C:\\a.mp3", "C:\\Rock"], [])).toEqual({
      tracks: ["C:\\a.mp3", "C:\\Rock"],
      folders: [],
    });
  });

  it("noktayla başlayan adı uzantı saymaz", () => {
    expect(splitDropped(["/home/.mp3"], extensions).folders).toEqual(["/home/.mp3"]);
  });
});
