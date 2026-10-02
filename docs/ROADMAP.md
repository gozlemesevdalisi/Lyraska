# Lyraska — Yol haritası

Görselleri şarkıyı önceden bilen, tamamen çevrimdışı çalışan Windows müzik çalar.
Lansman hedefi: **Mart 2027 başı (v1.0)**.

## Kalite hedefleri

| Hedef                  | Ölçüt                            |
| ---------------------- | -------------------------------- |
| Beat tespiti doğruluğu | F-ölçüsü en az **0,80**          |
| Ses–görüntü senkronu   | **±20 ms**                       |
| Görsel akıcılık        | 1080p, dahili GPU'da **60 fps**  |
| Kararlılık             | 1 saat çalmada **sıfır takılma** |

## Faz 0 — Temel (5–11 Ekim 2026)

- [x] Tauri 2 + React + TypeScript iskeleti; Rust tarafı ses motoru, analiz ve görsel köprüsü modüllerine ayrılmış
- [x] Karşılama ekranı (eski araba teybi ekranı havasında, özgün tasarım)
- [x] CI: her push ve PR'da Rust + TypeScript testleri, lint, lisans denetimi
- [x] Windows kurulum dosyası ve sabit "test-surumu" indirme linki
- [x] CLAUDE.md, README, Türkçe hata kaydı formu

## Faz 1 — Çalan program, v0.1 (12 Ekim – 8 Kasım 2026)

- [ ] Ses motoru: symphonia ile çözme, 64-bit iç işlem, WASAPI çıkışı
- [ ] Müzik kütüphanesi (SQLite): klasör tarama, etiketler, arama
- [ ] 10 bantlı ekolayzer
- [ ] 3 sahne: nokta matris spektrum, VU ibreleri, bir shader sahnesi

## Faz 2 — Görsel Yönetmen, v0.2 (9 Kasım – 20 Aralık 2026)

- [ ] Şarkı haritası analizi: beat, ölçü, bölümler, drop, enerji
- [ ] Gecikme telafisi ve kalibrasyon
- [ ] Koreografi: atmosfer, ritim ve doku katmanları
- [ ] "Gece otoyolu" sahnesi
- [ ] Epilepsi güvenli modu (saniyede en fazla 3 parlama)

## Faz 3 — Profesyonel ses, v0.3 (21 Aralık 2026 – 17 Ocak 2027)

- [ ] Parametrik EQ
- [ ] AutoEq kulaklık profilleri
- [ ] EBU R128 loudness
- [ ] Crossfeed
- [ ] Retriever: sıkıştırılmış (MP3 vb.) sesi iyileştirme — çalışma adı; ürün içi adı özgün olacak
- [ ] Bit-perfect mod

## Faz 4 — Görsel kütüphane ve v1.0 (18 Ocak – 28 Şubat 2027; lansman Mart 2027 başı)

- [ ] 15+ özgün sahne
- [ ] Butterchurn / MilkDrop desteği
- [ ] Kinetik şarkı sözleri
- [ ] Preset sistemi
- [ ] Kod imzalama (Windows "bilinmeyen yayımcı" uyarısının kalkması)

## Faz 5 — Büyüme (v1.0 sonrası)

- [ ] Sistem sesi modu (bilgisayarda çalan her sesi görselleştirme)
- [ ] Video dışa aktarma
- [ ] macOS ve Linux sürümleri
- [ ] Oda ışıkları entegrasyonu
- [ ] Yapay zekâ destekli analiz
