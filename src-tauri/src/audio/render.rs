//! Aygıt arabelleğini halka tampondan dolduran gerçek zamanlı kısım.
//!
//! Bu kod ses çıkış iş parçacığında çalışır. Kurallar: bellek ayırma yok,
//! kilit bekleme yok, dosya erişimi yok, panic yok.

use std::sync::Arc;

use rtrb::Consumer;

use super::eq::{EqControl, EqProcessor};
use super::limiter::Limiter;
use super::peq::{PeqControl, PeqProcessor};
use super::Sample;

/// Duraklat/devam geçişinin süresi. Ani kesilme "tık" sesine yol açar.
pub const FADE_SECONDS: f64 = 0.010;

/// Bir doldurma turunun sonucu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderOutcome {
    /// Halka tampondan alınan kare sayısı (kare = her kanaldan bir örnek).
    pub frames_consumed: usize,
    /// Çalıyor olması gerekirken veri yetişmediği için sessizlikle doldurulan kare sayısı.
    pub frames_missing: usize,
}

/// Halka tampondaki 64-bit örnekleri ekolayzerden ve taşma korumasından geçirip
/// aygıtın 32-bit kayan nokta biçimine yazar.
pub struct Renderer {
    channels: usize,
    /// Anlık kazanç (0 = sessiz, 1 = tam).
    gain: f64,
    /// Her karede kazancın değiştiği miktar.
    fade_step: f64,
    eq: EqProcessor,
    headphone: PeqProcessor,
    limiter: Limiter,
    /// Bir karelik çalışma alanı (bellek bir kez ayrılır).
    frame: Vec<Sample>,
}

impl Renderer {
    pub fn new(
        channels: usize,
        sample_rate: u32,
        eq: Arc<EqControl>,
        headphone: Arc<PeqControl>,
    ) -> Self {
        let fade_frames = (FADE_SECONDS * f64::from(sample_rate)).max(1.0);
        let channels = channels.max(1);
        Self {
            channels,
            gain: 0.0,
            fade_step: 1.0 / fade_frames,
            eq: EqProcessor::new(eq, channels, sample_rate),
            headphone: PeqProcessor::new(headphone, channels, sample_rate),
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
        out: &mut [f32],
        paused: bool,
    ) -> RenderOutcome {
        let target = if paused { 0.0 } else { 1.0 };
        let mut outcome = RenderOutcome::default();
        self.eq.begin_block();
        self.headphone.begin_block();

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
            // Sıra: kulaklık düzeltmesi → kullanıcının ekolayzeri → ses geçişi → taşma koruması.
            self.headphone.process_frame(&mut self.frame);
            self.eq.advance_frame();
            for (channel, value) in self.frame.iter_mut().enumerate() {
                *value = self.eq.process(channel, *value) * self.gain;
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
    fn write_frame(&mut self, out: &mut [f32]) {
        self.limiter.process_frame(&mut self.frame);
        for (slot, &value) in out.iter_mut().zip(&self.frame) {
            *slot = to_device(value);
        }
    }
}

/// 64-bit iç örneği aygıt biçimine çevirir. Taşma koruması tepeleri zaten sınırın
/// altına indirir; bu kırpma ve geçersiz sayının sessizliğe çevrilmesi yalnızca son
/// güvenlik önlemidir (aygıta asla NaN gitmez).
fn to_device(sample: Sample) -> f32 {
    if sample.is_nan() {
        0.0
    } else {
        sample.clamp(-1.0, 1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtrb::RingBuffer;

    /// 1000 Hz örnekleme: geçiş 10 kare sürer, hesaplar kolay olur.
    const RATE: u32 = 1000;

    fn flat() -> Arc<EqControl> {
        Arc::new(EqControl::default())
    }

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
        let mut renderer = Renderer::new(1, RATE, flat(), Arc::default());
        let mut source = filled(&[0.8; 20]);
        let mut out = [0.0f32; 20];
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
        let mut renderer = Renderer::new(2, RATE, flat(), Arc::default());
        let mut source = filled(&[0.5; 200]);
        let mut out = [0.0f32; 40];
        renderer.render(&mut source, &mut out, false); // tam sese ulaş
        assert_eq!(renderer.gain(), 1.0);

        let mut out = [0.0f32; 60]; // 30 kare
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
        let mut renderer = Renderer::new(1, RATE, flat(), Arc::default());
        let mut source = filled(&samples);
        let mut out = [0.0f32; 30];
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
        assert!((out[29] - (0.69 - 0.01 * d as f32)).abs() < 1e-6);
    }

    #[test]
    fn duraklatilmisken_veri_yoksa_gecis_bitmis_sayilir() {
        let mut renderer = Renderer::new(1, RATE, flat(), Arc::default());
        let mut source = filled(&[0.5; 40]);
        let mut out = [0.0f32; 40];
        renderer.render(&mut source, &mut out, false); // tam ses, tampon boşaldı
        assert_eq!(renderer.gain(), 1.0);
        renderer.render(&mut source, &mut out, true);
        assert_eq!(renderer.gain(), 0.0);
    }

    #[test]
    fn veri_yetismezse_sessizlik_yazar_ve_sayar() {
        let mut renderer = Renderer::new(2, RATE, flat(), Arc::default());
        let mut source = filled(&[0.3; 5]); // 2,5 kare: son yarım kare okunmamalı
        let mut out = [9.0f32; 8];
        let outcome = renderer.render(&mut source, &mut out, false);
        assert_eq!(outcome.frames_consumed, 2);
        assert_eq!(outcome.frames_missing, 2);
        assert_eq!(source.slots(), 1, "yarım kare tamponda kalır");
        // Gecikme hattındaki son veri çıktıktan sonra yalnızca sessizlik gelir.
        let mut out = [9.0f32; 8];
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
        let mut renderer = Renderer::new(1, rate, eq, Arc::default());
        let w = 2.0 * std::f64::consts::PI * 1000.0 / f64::from(rate);
        let samples: Vec<Sample> = (0..rate).map(|i| (w * f64::from(i)).sin()).collect();
        let mut source = filled(&samples);
        let mut out = vec![0.0f32; rate as usize];
        renderer.render(&mut source, &mut out, false);
        let tail = &out[rate as usize / 2..];
        let rms =
            (tail.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>() / tail.len() as f64).sqrt();
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
        let mut renderer = Renderer::new(2, rate, eq.clone(), headphone.clone());
        let mut out = vec![0.0f32; 2 * 480]; // 10 ms
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
                    80 => headphone.set(Some(&profile), false),
                    100 => {
                        eq.set(flat);
                    }
                    _ => {}
                }
                let paused = (120..140).contains(&block);
                renderer.render(&mut source, &mut out, paused);
            }
        });
        // Son bloklarda tampon boşaldı: veri yetişmeme yolu da çalıştı.
        assert_eq!(source.slots(), 0);
        assert_eq!(
            info.count_total, 0,
            "ses yolu {} kez bellek ayırdı ({} bayt)",
            info.count_total, info.bytes_total
        );
    }

    #[test]
    fn tasmalari_kirpar() {
        assert_eq!(to_device(1.7), 1.0);
        assert_eq!(to_device(-3.0), -1.0);
        assert_eq!(to_device(0.25), 0.25);
    }
}
