//! Taşma koruması: ileriye bakan tepe sınırlayıcı (lookahead limiter).
//!
//! Bugünün yüksek seviyede basılmış kayıtlarında (özellikle MP3'te) örnekler tam
//! ölçeği (±1,0) aşabilir. Bunları sert kırpmak yüksek yerlerde cızırtı yapar.
//! Sınırlayıcı yalnızca aşan tepelerde devreye girer: tepe gelmeden önce kazancı
//! yumuşakça düşürür, sonra yavaşça geri bırakır. Tepeler sınırın altındayken ses
//! bit bit aynen geçer (yalnızca ~1,5 ms gecikir).
//!
//! Yöntem: istenen kazançların kayan penceredeki en küçüğü, hızlı iniş / yavaş
//! dönüş, ardından gecikme uzunluğunda kayan ortalama. Bu üçlü, tepe çıkışa
//! ulaştığında kazancın yeterince düşmüş olmasını garanti eder.
//!
//! Gerçek zamanlı ses iş parçacığında çalışır: bütün bellek `new` içinde ayrılır.

use super::Sample;

/// Sınır: 0 dBFS (tam ölçek). Yalnızca gerçekten taşan tepeler sınırlanır; tam
/// ölçeğe kadar olan ses bit bit aynen geçer. (Eskiden −0,3 dBFS idi: bugünün yüksek
/// basılmış kayıtlarında sınırlayıcı neredeyse sürekli çalışıp sesi hafifçe değiştiriyordu.)
pub const CEILING: Sample = 1.0;
/// İleriye bakma süresi.
const LOOKAHEAD_SECONDS: f64 = 0.0015;
/// Kazancın geri dönüş süresi (zaman sabiti).
const RELEASE_SECONDS: f64 = 0.08;

pub struct Limiter {
    channels: usize,
    /// Gecikme uzunluğu (kare).
    delay: usize,
    /// Gecikme hattı (kare × kanal), dairesel.
    line: Vec<Sample>,
    line_pos: usize,
    /// Kayan en küçük için son `delay + 1` istenen kazanç, dairesel.
    wanted: Vec<Sample>,
    wanted_pos: usize,
    /// Kayan ortalama için son `delay` yumuşatılmış kazanç ve toplamları.
    smoothed: Vec<Sample>,
    smoothed_pos: usize,
    smoothed_sum: Sample,
    /// Geri dönüş katsayısı ve son yumuşatılmış kazanç.
    release: Sample,
    held: Sample,
    /// En son uygulanan kazanç (tanılama ve testler için).
    gain: Sample,
}

impl Limiter {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        let channels = channels.max(1);
        let delay = ((LOOKAHEAD_SECONDS * f64::from(sample_rate)).round() as usize).max(1);
        let release = 1.0 - (-1.0 / (RELEASE_SECONDS * f64::from(sample_rate))).exp();
        Self {
            channels,
            delay,
            line: vec![0.0; delay * channels],
            line_pos: 0,
            wanted: vec![1.0; delay + 1],
            wanted_pos: 0,
            smoothed: vec![1.0; delay],
            smoothed_pos: 0,
            smoothed_sum: delay as Sample,
            release,
            held: 1.0,
            gain: 1.0,
        }
    }

    /// Gecikme (kare).
    pub fn latency_frames(&self) -> usize {
        self.delay
    }

    /// Son uygulanan kazanç (1 = dokunulmadı).
    pub fn gain(&self) -> Sample {
        self.gain
    }

    /// Bir kareyi yerinde işler: `frame` girişi alır, gecikmiş ve sınırlanmış çıkışı verir.
    pub fn process_frame(&mut self, frame: &mut [Sample]) {
        let peak = frame.iter().fold(0.0, |m: Sample, &x| m.max(x.abs()));
        let wanted = if peak > CEILING { CEILING / peak } else { 1.0 };

        // Kayan en küçük (pencere küçük: ~70 değer; düz tarama yeterince hızlı).
        self.wanted[self.wanted_pos] = wanted;
        self.wanted_pos = (self.wanted_pos + 1) % self.wanted.len();
        let minimum = self.wanted.iter().fold(1.0, |m: Sample, &g| m.min(g));

        // Hızlı iniş, yavaş dönüş.
        self.held = if minimum < self.held {
            minimum
        } else {
            self.held + (minimum - self.held) * self.release
        };

        // Kayan ortalama: kazanç eğrisini yumuşatır (ani kazanç değişimi bozulma yapar).
        self.smoothed_sum += self.held - self.smoothed[self.smoothed_pos];
        self.smoothed[self.smoothed_pos] = self.held;
        self.smoothed_pos = (self.smoothed_pos + 1) % self.smoothed.len();
        let mut gain = self.smoothed_sum / self.delay as Sample;
        // Kayan nokta birikimi: tam 1'e çok yakınsa 1 say (sese hiç dokunulmasın).
        if gain > 1.0 - 1e-12 {
            gain = 1.0;
        }
        self.gain = gain;

        // Gecikme hattı: çıkışa `delay` kare önceki örnek gider.
        let base = self.line_pos * self.channels;
        for (channel, sample) in frame.iter_mut().enumerate().take(self.channels) {
            let delayed = self.line[base + channel];
            self.line[base + channel] = *sample;
            *sample = if gain == 1.0 { delayed } else { delayed * gain };
        }
        self.line_pos = (self.line_pos + 1) % self.delay;
    }

    /// Belleği sıfırlar (sarma ya da yeni şarkı).
    pub fn reset(&mut self) {
        self.line.fill(0.0);
        self.wanted.fill(1.0);
        self.smoothed.fill(1.0);
        self.smoothed_sum = self.delay as Sample;
        self.held = 1.0;
        self.gain = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn run(limiter: &mut Limiter, input: &[Sample], channels: usize) -> Vec<Sample> {
        let mut out = input.to_vec();
        for frame in out.chunks_exact_mut(channels) {
            limiter.process_frame(frame);
        }
        out
    }

    #[test]
    fn sinirin_altinda_ses_aynen_gecer() {
        let rate = 48_000;
        let mut limiter = Limiter::new(2, rate);
        let d = limiter.latency_frames();
        let input: Vec<Sample> = (0..20_000)
            .map(|i| 0.95 * (2.0 * PI * 440.0 * (i / 2) as f64 / f64::from(rate)).sin())
            .collect();
        let out = run(&mut limiter, &input, 2);
        assert_eq!(
            &out[d * 2..],
            &input[..input.len() - d * 2],
            "bit bit aynı, yalnızca gecikmeli"
        );
        assert_eq!(limiter.gain(), 1.0);
    }

    #[test]
    fn tam_olcege_kadar_ses_aynen_gecer() {
        // Bugünün kayıtlarının tepeleri genellikle 0 ile −0,1 dBFS arasında: taşma
        // yoksa ses değişmemeli (ekolayzer kapalıyken ses bit bit aynı geçsin).
        let rate = 44_100;
        let mut limiter = Limiter::new(1, rate);
        let d = limiter.latency_frames();
        let mut input: Vec<Sample> = (0..20_000)
            .map(|i| 0.99 * (2.0 * PI * 1000.0 * i as f64 / f64::from(rate)).sin())
            .collect();
        input[5_000] = 1.0;
        input[9_000] = -1.0;
        let out = run(&mut limiter, &input, 1);
        assert_eq!(&out[d..], &input[..input.len() - d]);
        assert_eq!(limiter.gain(), 1.0);
    }

    #[test]
    fn tasan_tepeler_siniri_hic_asmaz() {
        let rate = 44_100;
        let mut limiter = Limiter::new(2, rate);
        // Yüksek basılmış kayıt: +3 dB'ye kadar taşan sinüs, ani darbeler ve kare dalga.
        let mut input = Vec::new();
        for i in 0..rate as usize {
            let t = i as f64 / f64::from(rate);
            let mut x = 1.4 * (2.0 * PI * 100.0 * t).sin() * (2.0 * PI * 0.7 * t).sin().abs();
            if i % 5000 == 0 {
                x = 1.9;
            }
            if (20_000..21_000).contains(&i) {
                x = if (i / 50) % 2 == 0 { 1.3 } else { -1.3 };
            }
            input.push(x);
            input.push(-x * 0.8);
        }
        let out = run(&mut limiter, &input, 2);
        let worst = out.iter().fold(0.0f64, |m, &x| m.max(x.abs()));
        assert!(worst <= CEILING + 1e-9, "en yüksek {worst}");
    }

    #[test]
    fn kazanc_yavasca_geri_doner_ve_ani_degismez() {
        let rate = 48_000;
        let mut limiter = Limiter::new(1, rate);
        // Kısa bir taşma, sonra sessizliğe yakın sabit ses.
        let mut input = vec![0.5; rate as usize];
        input[1000] = 2.0;
        let mut gains = Vec::new();
        for x in input.iter_mut() {
            let mut frame = [*x];
            limiter.process_frame(&mut frame);
            gains.push(limiter.gain());
        }
        let lowest = gains.iter().cloned().fold(1.0, f64::min);
        assert!(lowest < CEILING / 2.0 + 1e-6, "{lowest}");
        // Kazanç adımları küçük (tıkırtı yok).
        let biggest_step = gains
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max);
        assert!(biggest_step < 0.02, "{biggest_step}");
        // 0,5 saniye sonra neredeyse tam kazanç.
        assert!(gains[rate as usize / 2 + 1000] > 0.99);
        limiter.reset();
        assert_eq!(limiter.gain(), 1.0);
    }
}
