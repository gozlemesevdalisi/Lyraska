# Veritas ölçümleri: nasıl tekrar üretilir

Bu klasör [docs/VERITAS.md](../../VERITAS.md)'deki sayıları üreten araştırma betiklerini içerir. Programın
parçası değildir: derlenmez, paketlenmez, CI'da çalışmaz. Ses dosyası içermez; deney sesi çalışma anında üretilir
ve depo dışındaki bir çalışma klasörüne yazılır.

| Dosya                           | Ne yapar                                                                                       |
| ------------------------------- | ---------------------------------------------------------------------------------------------- |
| `sinyal.py`                     | Müziğe benzer sentetik stereo deney sesi (piyano benzeri, titreşimli yaylı, kick, hi-hat)      |
| `bit_hizi.py`                   | ffmpeg'in LAME'iyle 96–320 kbps: SNR, yan kanal hatası, üst kesim frekansı                     |
| `sartname_dogrulama.py`         | Şartnamenin hesapları: kutu tablosu, Seviye 0 kazancı, 2·MMSE, çoklu kopya, dither             |
| `lame-3.100-veritas-dump.patch` | LAME 3.100'e ölçüm yaması: nicemlemeden sonra her granülün θ, q ve yan bilgisini döker         |
| `kapsama.py`                    | F0 kapsama testi: θ çözücünün kurabildiği kutunun içinde mi (ISO, LAME ve bant kuralıyla)      |
| `hibrit.py`                     | MP3 hibrit süzgeç bankası: çözücünün sentezi S ve eşleniği Sᵀ (ffmpeg'e göre −125 dB)          |
| `beste.py`                      | R1 deney müziği: programla yazılmış altı MIDI parça, FluidSynth + FluidR3_GM (MIT) ile çalınır |
| `deney.py`                      | R1 düzeneği: döküm → çözücü görünümü, kutular, yöntemler, SNR ve Zimtohrli MOS                 |
| `r1.py`                         | R1 ölçümleri: çözücü doğrulaması, yöntem karşılaştırması, kâhin tavanları                      |
| `r2_dogrusal.py`                | R2 yapılabilirlik: deliklerin komşu değerlerden doğrusal kestirimi (birini dışarıda bırak)     |
| `R2_GOREV.md`                   | R2 görev tanımı: devralacak model için kendi başına anlaşılır, kanıtlı olgular ve kapılarla    |

## Gerekenler

- Python 3.11+ ile `numpy` ve `scipy` (ayrı bir sanal ortamda: `python -m venv venv && venv/bin/pip install numpy scipy`)
- `ffmpeg` (libmp3lame ile; ölçümde ffmpeg 6.1.1, Python 3.13, numpy 2.5, scipy 1.18)
- LAME 3.100 kaynak kodu (`lame-3.100.tar.gz`, sha256 `ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e`),
  bir C derleyicisi ve `make`

## Adımlar

Çalışma klasörü depo dışında olmalıdır (ör. `/tmp/veritas`); ses dosyaları oraya yazılır.

```sh
W=/tmp/veritas && mkdir -p $W && cd $W
BETIK=<depo>/docs/arastirma/veritas

# 1. Yamalı LAME
tar xzf lame-3.100.tar.gz && cd lame-3.100
patch -p1 < $BETIK/lame-3.100-veritas-dump.patch
./configure --disable-shared --enable-static --disable-gtktest --disable-decoder CFLAGS="-O2" && make -j8
cd $W

# 2. Bit hızı ölçümü ve deney sesi
venv/bin/python $BETIK/bit_hizi.py $W
venv/bin/python -c "import sys; sys.path.insert(0, '$BETIK'); from sinyal import stereo, write_wav; write_wav('$W/kaynak16.wav', stereo(), bits=16)"

# 3. Döküm: VERITAS_DUMP ortam değişkeni verilince yamalı LAME dökümü o dosyaya yazar
for m in "96:-b 96 --cbr" "128:-b 128 --cbr" "320:-b 320 --cbr" "v2:-V 2"; do
  n=${m%%:*}; a=${m#*:}
  VERITAS_DUMP=$W/dokum_$n.bin lame-3.100/frontend/lame --quiet $a kaynak16.wav lame_dokum_$n.mp3
done

# 4. Kapsama testi ve şartname hesapları
venv/bin/python $BETIK/kapsama.py dokum_96.bin dokum_128.bin dokum_320.bin dokum_v2.bin
venv/bin/python $BETIK/sartname_dogrulama.py
```

## R1: gerçek dalga biçiminde ölçüm

Ek gerekenler: `fluidsynth` ve `fluid-soundfont-gm` (Ubuntu paketleri; ses bankası MIT), Python'da `mido` ve
`zimtohrli` (Apache-2.0). Hepsi yalnızca araştırma içindir, programa girmez.

```sh
venv/bin/pip install mido zimtohrli
venv/bin/python $BETIK/beste.py $W/muzik            # altı parça, 20 sn, 44,1 kHz, 16 bit
mkdir -p $W/dokum
for p in pop akustik piyano elektronik orkestra vurmali; do
  for m in "64:-b 64 --cbr" "128:-b 128 --cbr"; do
    n=${m%%:*}; a=${m#*:}
    VERITAS_DUMP=$W/dokum/${p}_$n.bin lame-3.100/frontend/lame --quiet $a $W/muzik/$p.wav $W/dokum/${p}_$n.mp3
  done
done
cd $BETIK && VERITAS_LAME=$W/lame-3.100 $W/venv/bin/python r1.py $W/dokum pop_64 piyano_64 pop_128
```

`hibrit.py`'nin polifaz penceresi LAME kaynağındaki `mpglib/tabinit.c`'den çalışma anında okunur (depoya kopyalanmaz).
LAME 64 kbps stereo'yu 24 kHz'e (MPEG-2), 96 kbps'i 32 kHz'e indirir; çözücümüz ikisini de doğru çözer.

## Döküm biçimi

Her granül ve kanal için sırayla (küçük endian, `int` = 4 bayt, `float` = 4 bayt):

- `int[12]`: granül, kanal, blok türü, karışık blok, global_gain, mode_ext, scalefac_scale, preflag, sfbmax,
  psymax, max_nonzero_coeff, 0
- `int[4]` subblock_gain, `int[39]` scalefac, `int[39]` width, `int[39]` window
- `float[576]` θ (`xr`: kodlayıcının nicemlediği MDCT katsayısı; M/S modunda M/S alanında)
- `int[576]` q (`l3_enc`: işaretli tam sayılar)

Ölçek çarpanları LAME'in iç gösterimindedir: ikinci granülde birinci granülünkini yeniden kullanan bantlar −1
yazılır (scfsi); `kapsama.py` çözücünün yaptığı gibi o bantlarda birinci granülün değerini kullanır.

## Sınırlar

- Ölçüm tek bir sentetik deney sesiyle yapıldı; gerçek şarkılarla (proje sahibinin kendi kayıpsız arşivi) tekrar
  edilmelidir. Telif nedeniyle şarkılar depoya girmez.
- Yalnızca LAME 3.100 ölçüldü. FhG, iTunes, Xing ve eski LAME sürümlerinin yuvarlama kuralları farklı olabilir.
- Karışık bloklar (mixed block) ölçülmedi; deney sesinde hiç çıkmadı.
