import { scrollToRow, visibleRange } from "./virtual";
import {
  EMPTY_QUEUE,
  currentPath,
  followQueue,
  nextInQueue,
  previousInQueue,
  queueFrom,
  upcomingPath,
} from "./queue";

describe("visibleRange", () => {
  it("görünen satırları ve kenar payını hesaplar", () => {
    // 34 px satır, 340 px pencere, 1000. pikselde: ilk görünen satır 29
    expect(visibleRange(1000, 340, 34, 10_000, 5)).toEqual({ start: 24, end: 45 });
  });

  it("listenin başını ve sonunu aşmaz", () => {
    expect(visibleRange(0, 340, 34, 3)).toEqual({ start: 0, end: 3 });
    expect(visibleRange(-50, 340, 34, 100).start).toBe(0);
    expect(visibleRange(100, 340, 34, 0)).toEqual({ start: 0, end: 0 });
  });
});

describe("scrollToRow", () => {
  it("satır görünmüyorsa kaydırır, görünüyorsa dokunmaz", () => {
    expect(scrollToRow(0, 100, 340, 34)).toBe(0); // yukarıda kalmış
    expect(scrollToRow(20, 0, 340, 34)).toBe(20 * 34 + 34 - 340); // aşağıda kalmış
    expect(scrollToRow(3, 0, 340, 34)).toBeNull();
  });
});

describe("çalma sırası", () => {
  const list = ["a.mp3", "b.mp3", "c.mp3"];

  it("seçilen şarkıdan başlar, sırayla ilerler ve sonda durur", () => {
    let queue = queueFrom(list, "b.mp3");
    expect(currentPath(queue)).toBe("b.mp3");
    queue = nextInQueue(queue)!;
    expect(currentPath(queue)).toBe("c.mp3");
    expect(nextInQueue(queue)).toBeNull();
  });

  it("geri gider, başta durur", () => {
    const queue = queueFrom(list, "b.mp3");
    expect(currentPath(previousInQueue(queue)!)).toBe("a.mp3");
    expect(previousInQueue(queueFrom(list, "a.mp3"))).toBeNull();
  });

  it("boşluksuz geçiş: sıradaki önceden bildirilir, çekirdek geçince sıra ilerler", () => {
    const queue = queueFrom(list, "a.mp3");
    expect(upcomingPath(queue, "a.mp3")).toBe("b.mp3");
    // Çalan şarkı sıradan değilse (ör. "Dosya aç") sıradaki bildirilmez.
    expect(upcomingPath(queue, "x.mp3")).toBeNull();
    expect(upcomingPath(queue, null)).toBeNull();
    expect(upcomingPath(queueFrom(list, "c.mp3"), "c.mp3")).toBeNull();
    // Çekirdek b'ye geçti: sıra b'de sayılır ve sıradaki c olur.
    const followed = followQueue(queue, "b.mp3");
    expect(currentPath(followed)).toBe("b.mp3");
    expect(upcomingPath(followed, "b.mp3")).toBe("c.mp3");
    // Değişiklik yoksa aynı sıra döner.
    expect(followQueue(queue, "a.mp3")).toBe(queue);
    expect(followQueue(queue, "x.mp3")).toBe(queue);
  });

  it("listede olmayan şarkı tek başına sıra olur; boş sıra güvenlidir", () => {
    expect(queueFrom(list, "x.mp3")).toEqual({ paths: ["x.mp3"], index: 0 });
    expect(currentPath(EMPTY_QUEUE)).toBeNull();
    expect(nextInQueue(EMPTY_QUEUE)).toBeNull();
  });
});
