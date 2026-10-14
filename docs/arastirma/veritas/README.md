# Veritas ölçümleri: nasıl tekrar üretilir

Bu klasör [docs/VERITAS.md](../../VERITAS.md)'deki sayıları üreten araştırma betiklerini içerir. Programın
parçası değildir: derlenmez, paketlenmez, CI'da çalışmaz. Ses dosyası içermez; deney sesi çalışma anında üretilir
ve depo dışındaki bir çalışma klasörüne yazılır.

| Dosya                           | Ne yapar                                                                                  |
| ------------------------------- | ----------------------------------------------------------------------------------------- |
| `sinyal.py`                     | Müziğe benzer sentetik stereo deney sesi (piyano benzeri, titreşimli yaylı, kick, hi-hat) |
| `bit_hizi.py`                   | ffmpeg'in LAME'iyle 96–320 kbps: SNR, yan kanal hatası, üst kesim frekansı                |
| `sartname_dogrulama.py`         | Şartnamenin hesapları: kutu tablosu, Seviye 0 kazancı, 2·MMSE, çoklu kopya, dither        |
| `lame-3.100-veritas-dump.patch` | LAME 3.100'e ölçüm yaması: nicemlemeden sonra her granülün θ, q ve yan bilgisini döker    |
| `kapsama.py`                    | F0 kapsama testi: θ çözücünün kurabildiği kutunun içinde mi (ISO, LAME ve bant kuralıyla) |

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
