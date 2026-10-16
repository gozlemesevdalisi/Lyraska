# Veritas ölçümleri: nasıl tekrar üretilir

Bu klasör [docs/VERITAS.md](../../VERITAS.md)'deki sayıları üreten araştırma betiklerini içerir. Programın
parçası değildir: derlenmez, paketlenmez, CI'da çalışmaz. Ses dosyası içermez; deney sesi çalışma anında üretilir
ve depo dışındaki bir çalışma klasörüne yazılır.

| Dosya                           | Ne yapar                                                                                        |
| ------------------------------- | ----------------------------------------------------------------------------------------------- |
| `sinyal.py`                     | Müziğe benzer sentetik stereo deney sesi (piyano benzeri, titreşimli yaylı, kick, hi-hat)       |
| `bit_hizi.py`                   | ffmpeg'in LAME'iyle 96–320 kbps: SNR, yan kanal hatası, üst kesim frekansı                      |
| `sartname_dogrulama.py`         | Şartnamenin hesapları: kutu tablosu, Seviye 0 kazancı, 2·MMSE, çoklu kopya, dither              |
| `lame-3.100-veritas-dump.patch` | LAME 3.100'e ölçüm yaması: nicemlemeden sonra her granülün θ, q ve yan bilgisini döker          |
| `kapsama.py`                    | F0 kapsama testi: θ çözücünün kurabildiği kutunun içinde mi (ISO, LAME ve bant kuralıyla)       |
| `hibrit.py`                     | MP3 hibrit süzgeç bankası: çözücünün sentezi S ve eşleniği Sᵀ (ffmpeg'e göre −125 dB)           |
| `beste.py`                      | R1 deney müziği: programla yazılmış altı MIDI parça, FluidSynth + FluidR3_GM (MIT) ile çalınır  |
| `deney.py`                      | R1 düzeneği: döküm → çözücü görünümü, kutular, yöntemler, SNR ve Zimtohrli MOS                  |
| `r1.py`                         | R1 ölçümleri: çözücü doğrulaması, yöntem karşılaştırması, kâhin tavanları                       |
| `r2_dogrusal.py`                | R2 yapılabilirlik: deliklerin komşu değerlerden doğrusal kestirimi (birini dışarıda bırak)      |
| `R2_GOREV.md`                   | R2 görev tanımı: devralacak model için kendi başına anlaşılır, kanıtlı olgular ve kapılarla     |
| `r2_beste.py`                   | R2 eğitim/sınama müziği: tohumdan rastgele ton, tempo, çalgı, ritim; FluidR3_GM ile çalınır     |
| `r2_veri.py`                    | R2 veri hazırlığı: dökümleri `deney.Track` ile çözücü görünümüne çevirip sıkışık .npz yazar     |
| `r2_model.py`                   | R2 model: delikler için kesik Gauss karışımı (bilgi ölçümü) ve akış eşleme örnekleyici; CPU     |
| `r2_degerlendir.py`             | R2 ölçüm: bit kazancı, enerji hatası, örnekleyiciler (SNR, MOS, kutu dışı), bilgi–algı eğrisi   |
| `r2_tonal.py`                   | R2 / S3: kâhin ayrıştırması, tonal tavan (gerçek sinyalin tepeleri) ve tonal POCS               |
| `r2_tablo.py`                   | R2 JSON sonuçlarını VERITAS.md için Markdown tablolarına çevirir                                |
| `r2_bagimsiz_kahin.py`          | R2'nin bağımsız denetimi: kâhin ayrıştırması (8.2) kendi kodumuzla + genlik/işaret ayrımı       |
| `r2_bagimsiz_komsu.py`          | Bağımsız denetim: gerçek komşu granüller deliklere doğrusal ve KSG ile ne kadar bilgi taşıyor   |
| `r2_bagimsiz_kosullu.py`        | Bağımsız denetim: çözücü bağlamına EK bilgi (KSG koşullu karşılıklı bilgi), seviye kestirimi    |
| `r2_bagimsiz_isaret.py`         | Bağımsız denetim: deliklerin işareti gerçek komşulardan kestirilebilir mi (kNN, olumlu denetim) |

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

## R2: öğrenmeli önsel (bölüm 8)

Ek gerekenler: `venv/bin/pip install torch` (işlemci sürümü yeter; GPU kullanılmaz). Eğitim müziği R1'in altı
parçasını içermez; onlar yalnızca sınamada kullanılır.

```sh
cd $BETIK
# 1. Eğitim (160 parça × 12 sn) ve sınama (16 parça) müziği, 64 ve 96 kbps dökümleri
$W/venv/bin/python r2_beste.py $W/egitim 0 160 12
$W/venv/bin/python r2_beste.py $W/sinama 500 16 12
for d in egitim sinama; do mkdir -p $W/dokum_$d; for f in $W/$d/*.wav; do p=$(basename $f .wav)
  for m in "64:-b 64 --cbr" "96:-b 96 --cbr"; do n=${m%%:*}; a=${m#*:}
    VERITAS_DUMP=$W/dokum_$d/${p}_$n.bin $W/lame-3.100/frontend/lame --quiet $a $f $W/dokum_$d/${p}_$n.mp3
  done; done; done
# 2. Sıkışık veri (R1 dökümleri de, sınama için)
export VERITAS_LAME=$W/lame-3.100
for d in dokum dokum_egitim dokum_sinama; do $W/venv/bin/python r2_veri.py $W/$d $W/veri_${d#dokum_} 3; done
# 3. Modeller (her biri 4 çekirdekte 15–30 dk)
$W/venv/bin/python r2_model.py egit $W/veri_egitim $W/m_yog_cozucu.pt --bas yogunluk --baglam cozucu --adim 4000
$W/venv/bin/python r2_model.py egit $W/veri_egitim $W/m_yog_yok.pt    --bas yogunluk --baglam yok    --adim 2500
$W/venv/bin/python r2_model.py egit $W/veri_egitim $W/m_yog_kahin.pt  --bas yogunluk --baglam kahin  --adim 4000
$W/venv/bin/python r2_model.py egit $W/veri_egitim $W/m_akis_cozucu.pt --bas akis    --baglam cozucu --adim 6000
# 4. Ölçüm (R1'in altı parçası, 64 ve 96 kbps) ve tablolar
$W/venv/bin/python r2_degerlendir.py $W/dokum $W/veri_dokum $W/r2_64.json pop_64 piyano_64 akustik_64 elektronik_64 orkestra_64 vurmali_64 \
  --model "bağlamsız=$W/m_yog_yok.pt" --model "koşullu=$W/m_yog_cozucu.pt" --model "kâhin bağlam=$W/m_yog_kahin.pt" --akis $W/m_akis_cozucu.pt --egri
$W/venv/bin/python r2_tonal.py $W/dokum pop_64 piyano_64 akustik_64 elektronik_64 orkestra_64 vurmali_64
$W/venv/bin/python r2_tablo.py $W/r2_64.json
```

Model ağırlıkları depoya konmaz (kapı geçilmedi; bkz. VERITAS.md bölüm 8).

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
