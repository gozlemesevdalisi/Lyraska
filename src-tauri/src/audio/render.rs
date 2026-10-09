//! Aygıt arabelleğini halka tampondan dolduran gerçek zamanlı kısım.
//!
//! Bu kod ses çıkış iş parçacığında çalışır. Kurallar: bellek ayırma yok,
//! kilit bekleme yok, dosya erişimi yok, panic yok.

use std::sync::Arc;

use rtrb::Consumer;

use super::eq::{EqControl, EqProcessor};
use super::limiter::Limiter;
use super::normalize::{Leveler, LoudnessControl, TrackLevels};
use super::peq::{PeqControl, PeqProcessor};
use super::Sample;

/// Ses işlemenin ayarları: oynatıcı boyunca aynı olanlar (ekolayzer, kulaklık düzeltmesi,
/// eşitleme ayarı) ve oturuma ait olanlar (şarkı seviyeleri, bit-perfect). Hepsi kilitsiz
/// okunur.
#[derive(Debug, Clone)]
pub struct RenderControls {
    pub eq: Arc<EqControl>,
    pub headphone: Arc<PeqControl>,
    pub loudness: Arc<LoudnessControl>,
    pub levels: Arc<TrackLevels>,
    /// Bit-perfect oturum: ses hiç işlenmez (yalnızca duraklatma geçişi ve taşma koruması,
    /// o da tam ölçeğe kadar olan sese dokunmaz).
    pub bit_perfect: bool,
}

impl Default for RenderControls {
    /// Sesi hiç değiştirmeyen ayarlar (ekolayzer düz, düzeltme ve eşitleme kapalı).
    fn default() -> Self {
        Self {
            eq: Arc::default(),
            headphone: Arc::default(),
            loudness: Arc::new(LoudnessControl::new(false)),
            levels: Arc::new(TrackLevels::new()),
            bit_perfect: false,
        }
    }
}

/// Duraklat/devam geçişinin süresi. Ani kesilme "tık" sesine yol açar.
pub const FADE_SECONDS: f64 = 0.010;
/// Eşitleme açıkken ilk ses, şarkının ses yüksekliği ölçümü gelene kadar en fazla bu kadar
/// bekletilir: seviye ilk örnekten doğru olsun. Önbellekteki şarkılarda ve sarmada ölçüm
/// zaten hazırdır, hiç beklenmez; yeni bir şarkıda hızlı ölçüm genellikle yetişir.
const LOUDNESS_WAIT_SECONDS: f64 = 0.6;

/// Bir doldurma turunun sonucu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderOutcome {
    /// Halka tampondan alınan kare sayısı (kare = her kanaldan bir örnek).
    pub frames_consumed: usize,
    /// Çalıyor olması gerekirken veri yetişmediği için sessizlikle doldurulan kare sayısı.
    pub frames_missing: usize,
}

/// Halka tampondaki 64-bit örnekleri ekolayzerden ve taşma korumasından geçirip aygıt
/// arabelleğine (−1..1, 64-bit) yazar; aygıtın biçimine çevirme çıkışta yapılır.
pub struct Renderer {
    channels: usize,
    /// Anlık kazanç (0 = sessiz, 1 = tam).
    gain: f64,
    /// Her karede kazancın değiştiği miktar.
    fade_step: f64,
    eq: EqProcessor,
    headphone: PeqProcessor,
    /// Ses yüksekliği eşitlemesi ve ekolayzerin taşma koruması.
    leveler: Leveler,
    levels: Arc<TrackLevels>,
    loudness: Arc<LoudnessControl>,
    sample_rate: u32,
    /// İlk sesten önce ölçüm için daha ne kadar beklenebilir (kare); 0: çalıyor.
    wait_frames: usize,
    /// Bit-perfect: kulaklık düzeltmesi, ekolayzer ve eşitleme atlanır.
    bit_perfect: bool,
    limiter: Limiter,
    /// Bir karelik çalışma alanı (bellek bir kez ayrılır).
    frame: Vec<Sample>,
}

impl Renderer {
    pub fn new(channels: usize, sample_rate: u32, controls: RenderControls) -> Self {
        let fade_frames = (FADE_SECONDS * f64::from(sample_rate)).max(1.0);
        let channels = channels.max(1);
        let leveler = Leveler::new(sample_rate, &controls.levels, controls.loudness.enabled());
        Self {
            channels,
            gain: 0.0,
            fade_step: 1.0 / fade_frames,
            eq: EqProcessor::new(controls.eq, channels, sample_rate),
            headphone: PeqProcessor::new(controls.headphone, channels, sample_rate),
            leveler,
            levels: controls.levels,
            loudness: controls.loudness,
            sample_rate,
            wait_frames: if controls.bit_perfect {
                0
            } else {
                (LOUDNESS_WAIT_SECONDS * f64::from(sample_rate)) as usize
            },
            bit_perfect: controls.bit_perfect,
            limiter: Limiter::new(channels, sample_rate),
            frame: vec![0.0; channels],
        }
    }

    /// Taşma korumasının gecikmesi (kare).
    pub fn latency_frames(&self) -> usize {
        self.limiter.latency_frames()
    }

    /// Anlık kazanç (testler ve tanılama için).
    pub fn gain(&self) -> f64 {
        self.gain
    }

    /// `out` arabelleğini doldurur. `out.len()` kanal sayısının katı olmalıdır.
    ///
    /// Duraklatılmışken kazanç yumuşakça sıfıra iner, sonra halka tampondan veri
    /// alınmaz (şarkı kaldığı yerde bekler).
    pub fn render(
        &mut self,
        source: &mut Consumer<Sample>,
        out: &mut [Sample],
        paused: bool,
    ) -> RenderOutcome {
        let target = if paused { 0.0 } else { 1.0 };
        let mut outcome = RenderOutcome::default();
        self.eq.begin_block();
        self.headphone.begin_block();
        let normalize = self.loudness.enabled();
        if self.wait_frames > 0 {
            if !normalize || self.levels.is_known(0) {
                // Ölçüm geldi: kazanç tahminden değil, ilk örnekten doğru başlar.
                self.wait_frames = 0;
                self.leveler = Leveler::new(self.sample_rate, &self.levels, normalize);
            } else if !paused {
                // Ölçüm bekleniyor: sessizlik yazılır, şarkı yerinde bekler.
                let frames = out.len() / self.channels;
                self.wait_frames = self.wait_frames.saturating_sub(frames);
                for frame in out.chunks_exact_mut(self.channels) {
                    self.frame.fill(0.0);
                    self.write_frame(frame);
                }
                return outcome;
            }
        }
        self.leveler
            .begin_block(&self.levels, normalize, self.eq.boost_db());

        for frame in out.chunks_exact_mut(self.channels) {
            // Sessiz karelerde de taşma korumasının gecikme hattı ilerler: duraklatma
            // anındaki son milisaniyeler de çalınır.
            if paused && self.gain <= 0.0 {
                self.frame.fill(0.0);
                self.write_frame(frame);
                continue;
            }
            // Yarım kare okumamak için bütün kanallar hazır olmalı.
            if source.slots() < self.channels {
                self.frame.fill(0.0);
                self.write_frame(frame);
                if paused {
                    // Veri yokken çıkış zaten sessiz: duraklatma geçişi bitmiş sayılır
                    // (devam edince ses yine yumuşakça açılır).
                    self.gain = 0.0;
                } else {
                    outcome.frames_missing += 1;
                }
                continue;
            }
            for value in self.frame.iter_mut() {
                *value = source.pop().unwrap_or(0.0);
            }
            if self.bit_perfect {
                // Ses işlenmez; çalarken kazanç tam 1 olduğu için örnekler aynen geçer.
                for value in self.frame.iter_mut() {
                    *value *= self.gain;
                }
            } else {
                // Sıra: kulaklık düzeltmesi → kullanıcının ekolayzeri → eşitleme ve
                // ekolayzerin taşma koruması → ses geçişi → taşma koruması (sınırlayıcı).
                self.headphone.process_frame(&mut self.frame);
                self.eq.advance_frame();
                let level = self
                    .leveler
                    .next_gain(&self.levels, normalize, self.eq.boost_db());
                let gain = level * self.gain;
                for (channel, value) in self.frame.iter_mut().enumerate() {
                    *value = self.eq.process(channel, *value) * gain;
                }
            }
            self.write_frame(frame);
            outcome.frames_consumed += 1;

            // Kayan nokta birikim hatası hedefi bir adım geciktirmesin diye
            // hedefe bir adımdan yakınsa doğrudan hedefe oturt.
            let delta = target - self.gain;
            if delta.abs() <= self.fade_step * (1.0 + 1e-9) {
                self.gain = target;
            } else {
                self.gain += self.fade_step.copysign(delta);
            }
        }
        outcome
    }
}

impl Renderer {
    /// Çalışma alanındaki kareyi taşma korumasından geçirip aygıt arabelleğine yazar.
    fn write_frame(&mut self, out: &mut [Sample]) {
        self.limiter.process_frame(&mut self.frame);
        for (slot, &value) in out.iter_mut().zip(&self.frame) {
            *slot = to_device(value);
        }
    }
}

/// 64-bit iç örneği aygıtın aralığına (−1..1) getirir. Taşma koruması tepeleri zaten
/// sınırın altına indirir; bu kırpma ve geçersiz sayının sessizliğe çevrilmesi yalnızca son
/// güvenlik önlemidir (aygıta asla NaN gitmez).
fn to_device(sample: Sample) -> Sample {
    if sample.is_nan() {
        0.0
    } else {
        sample.clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtrb::RingBuffer;

    /// 1000 Hz örnekleme: geçiş 10 kare sürer, hesaplar kolay olur.
    const RATE: u32 = 1000;

    fn filled(samples: &[Sample]) -> Consumer<Sample> {
        let (mut producer, consumer) = RingBuffer::new(samples.len().max(1));
        for &s in samples {
            producer.push(s).unwrap();
        }
        consumer
    }

    #[test]
    fn aygita_gecersiz_sayi_gitmez() {
        assert_eq!(to_device(Sample::NAN), 0.0);
        assert_eq!(to_device(Sample::INFINITY), 1.0);
        assert_eq!(to_device(Sample::NEG_INFINITY), -1.0);
        assert_eq!(to_device(0.5), 0.5);
    }

    #[test]
    fn baslangicta_sesi_yumusakca_acar() {
        let mut renderer = Renderer::new(1, RATE, RenderControls::default());
        let mut source = filled(&[0.8; 20]);
        let mut out = [0.0f64; 20];
        let outcome = renderer.render(&mut source, &mut out, false);

        let d = renderer.latency_frames(); // taşma korumasının gecikmesi
        assert_eq!(outcome.frames_consumed, 20);
        assert_eq!(out[0], 0.0); // ilk kare sessiz başlar
        assert!(out[5 + d] > 0.3 && out[5 + d] < 0.5);
        assert_eq!(out[19], 0.8); // geçiş bitti (sınırın altında: aynen)
        assert!(
            out.windows(2).all(|w| w[0] <= w[1]),
            "kazanç yalnızca artmalı"
        );
    }

    #[test]
    fn duraklatinca_yumusakca_susar_ve_veri_tuketmez() {
        let mut renderer = Renderer::new(2, RATE, RenderControls::default());
        let mut source = filled(&[0.5; 200]);
        let mut out = [0.0f64; 40];
        renderer.render(&mut source, &mut out, false); // tam sese ulaş
        assert_eq!(renderer.gain(), 1.0);

        let mut out = [0.0f64; 60]; // 30 kare
        let outcome = renderer.render(&mut source, &mut out, true);
        assert_eq!(
            outcome.frames_consumed, 10,
            "yalnızca geçiş süresince veri alınır"
        );
        assert_eq!(outcome.frames_missing, 0);
        assert!(out[0] > 0.45);
        assert!(out[40..].iter().all(|&s| s == 0.0));

        // Duraklatılmışken tampondaki veri yerinde kalır.
        let before = source.slots();
        renderer.render(&mut source, &mut out, true);
        assert_eq!(source.slots(), before);
    }

    #[test]
    fn devam_edince_kaldigi_yerden_surer() {
        let samples: Vec<Sample> = (0..100).map(|i| f64::from(i) / 100.0).collect();
        let mut renderer = Renderer::new(1, RATE, RenderControls::default());
        let mut source = filled(&samples);
        let mut out = [0.0f64; 30];
        renderer.render(&mut source, &mut out, false);
        renderer.render(&mut source, &mut out, true); // 10 karede susar
        renderer.render(&mut source, &mut out, true);
        assert_eq!(source.slots(), 100 - 30 - 10);

        let outcome = renderer.render(&mut source, &mut out, false);
        assert_eq!(outcome.frames_consumed, 30);
        // İlk kare, duraklatmadan önce kalınan örnektir (0,40), sessiz başlar.
        // Çıkış, taşma korumasının gecikmesi kadar geriden gelir.
        let d = renderer.latency_frames();
        assert_eq!(out[0], 0.0);
        assert!((out[29] - (0.69 - 0.01 * d as f64)).abs() < 1e-6);
    }

    #[test]
    fn duraklatilmisken_veri_yoksa_gecis_bitmis_sayilir() {
        let mut renderer = Renderer::new(1, RATE, RenderControls::default());
        let mut source = filled(&[0.5; 40]);
        let mut out = [0.0f64; 40];
        renderer.render(&mut source, &mut out, false); // tam ses, tampon boşaldı
        assert_eq!(renderer.gain(), 1.0);
        renderer.render(&mut source, &mut out, true);
        assert_eq!(renderer.gain(), 0.0);
    }

    #[test]
    fn veri_yetismezse_sessizlik_yazar_ve_sayar() {
        let mut renderer = Renderer::new(2, RATE, RenderControls::default());
        let mut source = filled(&[0.3; 5]); // 2,5 kare: son yarım kare okunmamalı
        let mut out = [9.0f64; 8];
        let outcome = renderer.render(&mut source, &mut out, false);
        assert_eq!(outcome.frames_consumed, 2);
        assert_eq!(outcome.frames_missing, 2);
        assert_eq!(source.slots(), 1, "yarım kare tamponda kalır");
        // Gecikme hattındaki son veri çıktıktan sonra yalnızca sessizlik gelir.
        let mut out = [9.0f64; 8];
        renderer.render(&mut source, &mut out, false);
        assert!(out.iter().all(|&s| s == 0.0), "{out:?}");
    }

    #[test]
    fn ekolayzer_ses_yolundadir() {
        use crate::audio::eq::{EqSettings, BANDS};
        // Bütün bantlar -12 dB: ses ~-12 dB'ye iner (ön kazanç 0, çünkü yükseltme yok;
        // tasarım sapması ±1 dB içinde).
        let eq = Arc::new(EqControl::new(EqSettings {
            enabled: true,
            gains_db: [-12.0; BANDS],
        }));
        let rate = 48_000;
        let controls = RenderControls {
            eq,
            ..RenderControls::default()
        };
        let mut renderer = Renderer::new(1, rate, controls);
        let w = 2.0 * std::f64::consts::PI * 1000.0 / f64::from(rate);
        let samples: Vec<Sample> = (0..rate).map(|i| (w * f64::from(i)).sin()).collect();
        let mut source = filled(&samples);
        let mut out = vec![0.0f64; rate as usize];
        renderer.render(&mut source, &mut out, false);
        let tail = &out[rate as usize / 2..];
        let rms = (tail.iter().map(|&s| s.powi(2)).sum::<f64>() / tail.len() as f64).sqrt();
        let db = 20.0 * (rms * 2f64.sqrt()).log10();
        assert!((db + 12.0).abs() < 1.0, "{db:.2} dB");
    }

    /// CLAUDE.md kuralı: ses çıkış geri çağrısında bellek ayırma yok. Ekolayzer ve kulaklık
    /// düzeltmesi açıkken, çalarken ayarlar değişirken (filtreler ses iş parçacığında yeniden
    /// tasarlanır), duraklatıp devam ederken ve veri yetişmezken tek bir ayırma bile olmamalı.
    #[test]
    fn ses_yolu_bellek_ayirmaz() {
        use crate::audio::eq::{EqSettings, BANDS};
        use crate::audio::peq::{FilterKind, HeadphoneProfile, PeqControl, PeqFilter};

        // Hazırlık (burada ayırma serbest).
        let rate = 48_000;
        let eq = Arc::new(EqControl::new(EqSettings {
            enabled: true,
            gains_db: [6.0, 5.0, 3.0, 1.0, 0.0, -1.0, 0.0, 2.0, 3.0, 4.0],
        }));
        let headphone = Arc::new(PeqControl::default());
        let profile = HeadphoneProfile {
            name: "Deneme".into(),
            preamp_db: -4.0,
            filters: vec![
                PeqFilter {
                    kind: FilterKind::LowShelf,
                    freq_hz: 105.0,
                    gain_db: 4.0,
                    q: 0.7,
                },
                PeqFilter {
                    kind: FilterKind::Peaking,
                    freq_hz: 3000.0,
                    gain_db: -3.0,
                    q: 2.0,
                },
                PeqFilter {
                    kind: FilterKind::HighShelf,
                    freq_hz: 10_000.0,
                    gain_db: 2.0,
                    q: 0.7,
                },
            ],
        };
        headphone.set(Some(&profile), true);
        let w = 2.0 * std::f64::consts::PI * 220.0 / f64::from(rate);
        let samples: Vec<Sample> = (0..rate * 2)
            .map(|i| 0.9 * (w * f64::from(i / 2)).sin())
            .collect();
        let mut source = filled(&samples);
        let loudness = Arc::new(LoudnessControl::new(true));
        let levels = Arc::new(TrackLevels::new());
        let controls = RenderControls {
            eq: eq.clone(),
            headphone: headphone.clone(),
            loudness: loudness.clone(),
            levels: levels.clone(),
            bit_perfect: false,
        };
        let mut renderer = Renderer::new(2, rate, controls.clone());
        // Bit-perfect oturumun yolu da aynı ölçümde.
        let mut bit_perfect = Renderer::new(
            2,
            rate,
            RenderControls {
                bit_perfect: true,
                ..controls
            },
        );
        let mut bit_perfect_source = filled(&samples[..rate as usize]);
        let mut out = vec![0.0f64; 2 * 480]; // 10 ms
        let flat = EqSettings {
            enabled: true,
            gains_db: [0.0; BANDS],
        };
        let boosted = EqSettings {
            enabled: true,
            gains_db: [12.0; BANDS],
        };

        let info = allocation_counter::measure(|| {
            for block in 0..250 {
                match block {
                    40 => {
                        eq.set(boosted);
                    }
                    60 => levels.set_loudness(
                        0,
                        Some(crate::audio::loudness::Loudness {
                            integrated_lufs: -8.0,
                            true_peak_dbtp: 0.0,
                        }),
                    ),
                    70 => levels.begin_track(70 * 480),
                    80 => headphone.set(Some(&profile), false),
                    90 => loudness.set(false),
                    100 => {
                        eq.set(flat);
                    }
                    _ => {}
                }
                let paused = (120..140).contains(&block);
                renderer.render(&mut source, &mut out, paused);
                bit_perfect.render(&mut bit_perfect_source, &mut out, paused);
            }
        });
        // Son bloklarda tampon boşaldı: veri yetişmeme yolu da çalıştı.
        assert_eq!(source.slots(), 0);
        assert_eq!(bit_perfect_source.slots(), 0);
        assert_eq!(
            info.count_total, 0,
            "ses yolu {} kez bellek ayırdı ({} bayt)",
            info.count_total, info.bytes_total
        );
    }

    fn normalizing(levels: &Arc<TrackLevels>) -> RenderControls {
        RenderControls {
            loudness: Arc::new(LoudnessControl::new(true)),
            levels: Arc::clone(levels),
            ..RenderControls::default()
        }
    }

    fn loud() -> crate::audio::loudness::Loudness {
        // −8 LUFS'lik şarkı: eşitleme −6 dB.
        crate::audio::loudness::Loudness {
            integrated_lufs: -8.0,
            true_peak_dbtp: 0.0,
        }
    }

    #[test]
    fn ilk_ses_olcumu_bekler_ve_dogru_seviyeden_baslar() {
        let levels = Arc::new(TrackLevels::new());
        let mut renderer = Renderer::new(1, RATE, normalizing(&levels));
        let mut source = filled(&[0.5; 400]);
        let mut out = [9.0f64; 100];
        let outcome = renderer.render(&mut source, &mut out, false);
        assert_eq!(
            outcome.frames_consumed, 0,
            "ölçüm yokken şarkı yerinde bekler"
        );
        assert_eq!(outcome.frames_missing, 0, "bekleme takılma sayılmaz");
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(source.slots(), 400);

        levels.set_loudness(0, Some(loud()));
        let mut out = [0.0f64; 200];
        let outcome = renderer.render(&mut source, &mut out, false);
        assert_eq!(outcome.frames_consumed, 200);
        // Açılış geçişinden sonra tam −6 dB'de: tahminden düzelme (yavaş kayma) yok.
        let expected = 0.5 * 10f64.powf(-6.0 / 20.0);
        assert!((out[199] - expected).abs() < 1e-6, "{}", out[199]);
        assert!((out[100] - expected).abs() < 1e-6, "{}", out[100]);
    }

    #[test]
    fn olcum_yetismezse_tahmini_seviyeyle_baslar() {
        let levels = Arc::new(TrackLevels::new());
        let mut renderer = Renderer::new(1, RATE, normalizing(&levels));
        let mut source = filled(&[0.5; 1000]);
        let mut out = [0.0f64; 100];
        for _ in 0..6 {
            // 0,6 sn (600 kare) beklenir.
            assert_eq!(
                renderer
                    .render(&mut source, &mut out, false)
                    .frames_consumed,
                0
            );
        }
        let mut out = [0.0f64; 200];
        assert_eq!(
            renderer
                .render(&mut source, &mut out, false)
                .frames_consumed,
            200
        );
        // Tipik kayıt varsayılır (−10 LUFS → −4 dB).
        let expected = 0.5 * 10f64.powf(-4.0 / 20.0);
        assert!((out[199] - expected).abs() < 1e-6, "{}", out[199]);
    }

    #[test]
    fn esitleme_kapaliyken_beklemez_ve_ses_aynen_gecer() {
        let levels = Arc::new(TrackLevels::new());
        let controls = RenderControls {
            levels: Arc::clone(&levels),
            ..RenderControls::default()
        };
        let mut renderer = Renderer::new(1, RATE, controls);
        let mut source = filled(&[0.5; 100]);
        let mut out = [0.0f64; 100];
        assert_eq!(
            renderer
                .render(&mut source, &mut out, false)
                .frames_consumed,
            100
        );
        assert_eq!(out[99], 0.5, "kazanç tam 1: bit bit aynı");
        assert!(!levels.is_known(0));
    }

    #[test]
    fn ses_yolu_bozulma_ve_gurultu_eklemez() {
        use crate::audio::eq::{EqSettings, BANDS};
        use crate::audio::peq::{FilterKind, HeadphoneProfile, PeqFilter};
        use crate::audio::test_util::{fit, sine};
        // Bütün işlemler açık: kulaklık düzeltmesi, "Bas" ayarı, eşitleme. Ses yolu doğrusal
        // olmalı: çıkışta tondan başka bir şey (bozulma, gürültü, cızırtı) olmamalı.
        let rate = 48_000u32;
        let mut gains = [0.0; BANDS];
        gains[..4].copy_from_slice(&[4.0, 7.0, 6.0, 2.5]);
        let headphone = Arc::new(PeqControl::default());
        headphone.set(
            Some(&HeadphoneProfile {
                name: "Deneme".into(),
                preamp_db: -4.0,
                filters: vec![
                    PeqFilter {
                        kind: FilterKind::LowShelf,
                        freq_hz: 105.0,
                        gain_db: 4.0,
                        q: 0.7,
                    },
                    PeqFilter {
                        kind: FilterKind::Peaking,
                        freq_hz: 3000.0,
                        gain_db: -3.0,
                        q: 2.0,
                    },
                ],
            }),
            true,
        );
        let levels = Arc::new(TrackLevels::new());
        levels.set_loudness(
            0,
            Some(crate::audio::loudness::Loudness {
                integrated_lufs: -9.0,
                true_peak_dbtp: -6.0,
            }),
        );
        let mut renderer = Renderer::new(
            2,
            rate,
            RenderControls {
                eq: Arc::new(EqControl::new(EqSettings {
                    enabled: true,
                    gains_db: gains,
                })),
                headphone,
                loudness: Arc::new(LoudnessControl::new(true)),
                levels,
                bit_perfect: false,
            },
        );
        for freq in [100.0, 1000.0, 10_000.0] {
            let frames = rate as usize * 2;
            let samples: Vec<Sample> = (0..frames)
                .flat_map(|f| [0.5 * sine(freq, rate, f); 2])
                .collect();
            let mut source = filled(&samples);
            let mut left = Vec::with_capacity(frames);
            let mut out = vec![0.0; 2 * 480];
            while left.len() < frames {
                renderer.render(&mut source, &mut out, false);
                // Paylaşımlı modda aygıta 32-bit kayan nokta gider: o dönüşüm de ölçüme girer.
                left.extend(out.chunks_exact(2).map(|f| f64::from(f[0] as f32)));
            }
            // Açılış ve ayarların yumuşak geçişi bittikten sonrası ölçülür.
            let (amplitude, noise_db) = fit(&left[rate as usize / 2..frames], freq, rate);
            assert!(amplitude > 0.05, "{freq} Hz: ses kayboldu ({amplitude})");
            assert!(
                noise_db < -MAX_PATH_NOISE_DB,
                "{freq} Hz: bozulma + gürültü {noise_db:.1} dB"
            );
        }
    }

    /// Ses yolunun eklediği bozulma ve gürültü en fazla bu kadar olabilir (sinüse göre, dB).
    /// Ölçülen ~−152 dB (32-bit kayan nokta çıkışın sınırı). Karşılaştırma: 24 bitlik
    /// kaydın kendi gürültüsü ~−144 dB, 16 bitliğinki (CD) ~−98 dB.
    const MAX_PATH_NOISE_DB: f64 = 140.0;

    #[test]
    fn bit_perfect_ornekleri_aynen_gecirir() {
        use crate::audio::eq::{EqSettings, BANDS};
        // Ekolayzer +12 dB, kulaklık düzeltmesi ve eşitleme açık: bit-perfect hepsini atlar.
        let eq = Arc::new(EqControl::new(EqSettings {
            enabled: true,
            gains_db: [12.0; BANDS],
        }));
        let levels = Arc::new(TrackLevels::new());
        let controls = RenderControls {
            eq,
            loudness: Arc::new(LoudnessControl::new(true)),
            levels: Arc::clone(&levels),
            bit_perfect: true,
            ..RenderControls::default()
        };
        let mut renderer = Renderer::new(2, RATE, controls);
        // 16 bitlik şarkının örnekleri (çözücünün verdiği biçimde), tam ölçek dahil.
        let samples: Vec<Sample> = (0..400)
            .map(|i| f64::from(((i * 7919) % 65_536 - 32_768) as i16) / 32_768.0)
            .chain([1.0 - 1.0 / 32_768.0, -1.0])
            .collect();
        let mut source = filled(&samples);
        let latency = renderer.latency_frames();
        let mut out = vec![0.0; samples.len() + 2 * latency + 40];
        let outcome = renderer.render(&mut source, &mut out, false);
        assert_eq!(
            outcome.frames_consumed,
            samples.len() / 2,
            "ölçüm beklenmez"
        );
        // Açılış geçişinden (10 kare) sonra örnekler taşma korumasının gecikmesi kadar
        // kayarak, bit bit aynı çıkar.
        let fade = 2 * 10;
        let start = 2 * latency;
        assert_eq!(&out[start + fade..start + samples.len()], &samples[fade..]);
        assert!(!levels.is_known(0));
    }

    #[test]
    fn tasmalari_kirpar() {
        assert_eq!(to_device(1.7), 1.0);
        assert_eq!(to_device(-3.0), -1.0);
        assert_eq!(to_device(0.25), 0.25);
    }
}
