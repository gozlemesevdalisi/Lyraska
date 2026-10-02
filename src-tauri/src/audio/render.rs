//! Aygıt arabelleğini halka tampondan dolduran gerçek zamanlı kısım.
//!
//! Bu kod ses çıkış iş parçacığında çalışır. Kurallar: bellek ayırma yok,
//! kilit bekleme yok, dosya erişimi yok, panic yok.

use rtrb::Consumer;

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

/// Halka tampondaki 64-bit örnekleri aygıtın 32-bit kayan nokta biçimine yazar.
pub struct Renderer {
    channels: usize,
    /// Anlık kazanç (0 = sessiz, 1 = tam).
    gain: f64,
    /// Her karede kazancın değiştiği miktar.
    fade_step: f64,
}

impl Renderer {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        let fade_frames = (FADE_SECONDS * f64::from(sample_rate)).max(1.0);
        Self {
            channels: channels.max(1),
            gain: 0.0,
            fade_step: 1.0 / fade_frames,
        }
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

        for frame in out.chunks_exact_mut(self.channels) {
            if paused && self.gain <= 0.0 {
                frame.fill(0.0);
                continue;
            }
            // Yarım kare okumamak için bütün kanallar hazır olmalı.
            if source.slots() < self.channels {
                frame.fill(0.0);
                if !paused {
                    outcome.frames_missing += 1;
                }
                continue;
            }
            for slot in frame.iter_mut() {
                let sample = source.pop().unwrap_or(0.0);
                *slot = to_device(sample * self.gain);
            }
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

/// 64-bit iç örneği aygıt biçimine çevirir; taşmaları kırpar.
fn to_device(sample: Sample) -> f32 {
    sample.clamp(-1.0, 1.0) as f32
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
    fn baslangicta_sesi_yumusakca_acar() {
        let mut renderer = Renderer::new(1, RATE);
        let mut source = filled(&[1.0; 20]);
        let mut out = [0.0f32; 20];
        let outcome = renderer.render(&mut source, &mut out, false);

        assert_eq!(outcome.frames_consumed, 20);
        assert_eq!(out[0], 0.0); // ilk kare sessiz başlar
        assert!(out[5] > 0.4 && out[5] < 0.6);
        assert_eq!(out[19], 1.0); // geçiş bitti
        assert!(
            out.windows(2).all(|w| w[0] <= w[1]),
            "kazanç yalnızca artmalı"
        );
    }

    #[test]
    fn duraklatinca_yumusakca_susar_ve_veri_tuketmez() {
        let mut renderer = Renderer::new(2, RATE);
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
        let mut renderer = Renderer::new(1, RATE);
        let mut source = filled(&samples);
        let mut out = [0.0f32; 30];
        renderer.render(&mut source, &mut out, false);
        renderer.render(&mut source, &mut out, true); // 10 karede susar
        renderer.render(&mut source, &mut out, true);
        assert_eq!(source.slots(), 100 - 30 - 10);

        let outcome = renderer.render(&mut source, &mut out, false);
        assert_eq!(outcome.frames_consumed, 30);
        // İlk kare, duraklatmadan önce kalınan örnektir (0,40), sessiz başlar.
        assert_eq!(out[0], 0.0);
        assert!((out[29] - 0.69).abs() < 1e-6);
    }

    #[test]
    fn veri_yetismezse_sessizlik_yazar_ve_sayar() {
        let mut renderer = Renderer::new(2, RATE);
        let mut source = filled(&[0.3; 5]); // 2,5 kare: son yarım kare okunmamalı
        let mut out = [9.0f32; 8];
        let outcome = renderer.render(&mut source, &mut out, false);
        assert_eq!(outcome.frames_consumed, 2);
        assert_eq!(outcome.frames_missing, 2);
        assert_eq!(source.slots(), 1, "yarım kare tamponda kalır");
        assert!(out[4..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn tasmalari_kirpar() {
        assert_eq!(to_device(1.7), 1.0);
        assert_eq!(to_device(-3.0), -1.0);
        assert_eq!(to_device(0.25), 0.25);
    }
}
