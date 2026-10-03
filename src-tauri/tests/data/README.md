# Test verisi

Bu klasördeki ses dosyaları proje için **ffmpeg ile üretilmiş sentetik sinyallerdir**; hiçbir
kayıttan alınmamıştır ve projenin lisansı (GPL-3.0) altındadır.

`uc-uc-ton.*`: 22 050 Hz mono, arka arkaya üç ton (440 Hz, 880 Hz, 1760 Hz; her biri 1 sn).
Etiketler: başlık "Üç Ton", sanatçı "Lyraska Test", albüm "Deneme Albümü", albüm sanatçısı "Lyraska",
parça 3/9, disk 1/2.

Yeniden üretmek için:

```sh
SRC="sine=frequency=440:duration=1:sample_rate=22050[a];sine=frequency=880:duration=1:sample_rate=22050[b];sine=frequency=1760:duration=1:sample_rate=22050[c];[a][b][c]concat=n=3:v=0:a=1,volume=0.5"
ffmpeg -filter_complex "$SRC" -ac 1 -c:a libmp3lame -b:a 64k -metadata title="Üç Ton" -metadata artist="Lyraska Test" -metadata album="Deneme Albümü" -metadata album_artist="Lyraska" -metadata track="3/9" -metadata disc="1/2" uc-uc-ton.mp3
ffmpeg -filter_complex "$SRC" -ac 1 -c:a aac -b:a 64k -metadata title="Üç Ton" -metadata artist="Lyraska Test" -metadata album="Deneme Albümü" -metadata album_artist="Lyraska" -metadata track="3/9" -metadata disc="1/2" uc-uc-ton.m4a
ffmpeg -filter_complex "$SRC" -ac 1 -c:a libvorbis -q:a 2 -metadata title="Üç Ton" -metadata artist="Lyraska Test" -metadata album="Deneme Albümü" -metadata album_artist="Lyraska" -metadata track="3/9" -metadata disc="1/2" uc-uc-ton.ogg
ffmpeg -filter_complex "$SRC" -ac 1 -c:a flac -metadata title="Üç Ton" -metadata artist="Lyraska Test" -metadata album="Deneme Albümü" -metadata album_artist="Lyraska" -metadata track="3/9" -metadata disc="1/2" uc-uc-ton.flac
```
