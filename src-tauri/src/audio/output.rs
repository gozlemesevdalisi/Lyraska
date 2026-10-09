//! Ses çıkışı.
//!
//! Windows'ta WASAPI kullanılır. Diğer sistemlerde çıkış henüz yoktur (macOS/Linux
//! v1.0'dan sonra); motorun geri kalanı yine derlenir ve test edilir.
//!
//! İki kip vardır ([`OutputMode`]):
//! - **Paylaşımlı** (varsayılan): akış aygıtın kendi örnekleme hızında açılır
//!   ([`device_info`]); şarkı o hıza [`super::resample`] ile çevrilir. Böylece Windows
//!   ses motoru sese dokunmaz; diğer programların sesi de duyulur.
//! - **Özel (bit-perfect)**: aygıt yalnızca Lyraska'ya ayrılır, şarkının kendi hızında ve
//!   tamsayı biçiminde açılır ([`exclusive_format`]). Şarkının örnekleri aygıta değişmeden
//!   gider; Windows ses düzeyi ve diğer programların sesi devre dışı kalır.

use std::sync::Arc;
use std::thread::JoinHandle;

use rtrb::Consumer;

use super::player::SharedState;
use super::{AudioError, Sample};

#[cfg(test)]
pub(crate) mod simulated;
#[cfg(all(windows, not(test)))]
mod wasapi;

/// Çıkış akışının biçimi: aygıtın örnekleme hızı, akışın kanal sayısı ve kipi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSpec {
    pub sample_rate: u32,
    pub channels: usize,
    pub mode: OutputMode,
}

/// Aygıtın nasıl açılacağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// Paylaşımlı mod, 32-bit kayan nokta.
    #[default]
    Shared,
    /// Özel mod (bit-perfect), aygıtın kabul ettiği tamsayı biçiminde.
    Exclusive(IntFormat),
}

/// Özel modda aygıta yazılan tamsayı örnek biçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntFormat {
    /// Bir örneğin kapladığı bit (16, 24 ya da 32).
    pub container_bits: u16,
    /// Anlamlı bit sayısı (kapsayıcıdan büyük olamaz); kapsayıcının üst bitlerinde durur.
    pub valid_bits: u16,
}

impl IntFormat {
    /// Özel modda denenen biçimler, en yüksek çözünürlükten başlayarak. 16 ve 24 bitlik
    /// şarkılar daha geniş biçimde de değişmeden (alt bitleri sıfır) çalınır.
    pub const CANDIDATES: [IntFormat; 4] = [
        IntFormat::new(32, 32),
        IntFormat::new(32, 24),
        IntFormat::new(24, 24),
        IntFormat::new(16, 16),
    ];

    pub const fn new(container_bits: u16, valid_bits: u16) -> Self {
        Self {
            container_bits,
            valid_bits,
        }
    }

    /// Bir örneğin bayt sayısı.
    pub fn bytes(self) -> usize {
        usize::from(self.container_bits / 8)
    }

    /// Aygıta hazırlanmış örneği (−1..1) tamsayıya çevirip `out`'a küçük sonlu yazar.
    /// Şarkının kendi tamsayıları (ör. 16 ya da 24 bit) bit bit aynen çıkar: çözücü
    /// onları 2'nin kuvvetine bölerek kayan noktaya çevirir, burada aynı kuvvetle çarpılır.
    #[inline]
    pub fn encode(self, sample: Sample, out: &mut [u8]) {
        let scale = (1i64 << (self.valid_bits - 1)) as f64;
        let value = (sample * scale).round().clamp(-scale, scale - 1.0) as i64;
        let shifted = value << (self.container_bits - self.valid_bits);
        let bytes = shifted.to_le_bytes();
        let n = self.bytes();
        out[..n].copy_from_slice(&bytes[..n]);
    }
}

/// Varsayılan ses aygıtı: adı ve paylaşımlı modda çalıştığı biçim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub name: String,
    pub sample_rate: u32,
    pub channels: usize,
}

/// Varsayılan ses aygıtının bilgisi. Öğrenilemezse `None` (akış şarkının kendi
/// hızında açılır, gerekirse Windows dönüştürür).
pub fn device_info() -> Option<DeviceInfo> {
    #[cfg(test)]
    {
        simulated::device_info()
    }
    #[cfg(all(windows, not(test)))]
    {
        wasapi::device_info()
    }
    #[cfg(all(not(windows), not(test)))]
    {
        None
    }
}

/// Aygıtın bu hızda ve kanal sayısında özel modda kabul ettiği en iyi tamsayı biçimi.
/// Olmazsa kullanıcıya gösterilecek nedeni döndürür.
pub fn exclusive_format(sample_rate: u32, channels: usize) -> Result<IntFormat, String> {
    #[cfg(test)]
    {
        simulated::exclusive_format(sample_rate, channels)
    }
    #[cfg(all(windows, not(test)))]
    {
        wasapi::exclusive_format(sample_rate, channels)
    }
    #[cfg(all(not(windows), not(test)))]
    {
        let _ = (sample_rate, channels);
        Err("özel mod yalnızca Windows'ta var".to_owned())
    }
}

/// Duraklatma geçişinin hoparlöre ulaşıp ulaşmadığını izler.
///
/// Aygıt arabelleği sesi ~100 ms önden tutar: geçiş yazıldığı anda değil, ancak
/// arabellekte önündeki ses çalınınca duyulur. Ses akışı bundan önce kapatılırsa
/// (sarma, şarkı değiştirme, durdurma) ses tam seviyede kesilir ve "tık" duyulur.
#[cfg(any(windows, test))]
#[derive(Debug)]
pub struct FadeWatch {
    /// Aygıta yazılan bütün kareler (sessizlik dahil).
    written: u64,
    /// Bu kareden itibaren çıkış tamamen sessiz (geçiş ve taşma korumasının gecikmesi bitti).
    silent_from: Option<u64>,
}

#[cfg(any(windows, test))]
impl Default for FadeWatch {
    /// Akış sessiz başlar (render kazancı 0, taşma korumasının hattı boş).
    fn default() -> Self {
        Self {
            written: 0,
            silent_from: Some(0),
        }
    }
}

#[cfg(any(windows, test))]
impl FadeWatch {
    /// Aygıta `frames` kare yazıldıktan sonra çağrılır. `silent`: bu turun sonunda
    /// ses tamamen kısılmış (duraklatıldı, geçiş bitti); `latency`: taşma korumasının
    /// gecikmesi (kısılan ses bu kadar kare sonra çıkar).
    pub fn wrote(&mut self, frames: usize, silent: bool, latency: usize) {
        self.written += frames as u64;
        if !silent {
            self.silent_from = None;
        } else if self.silent_from.is_none() {
            // Geçişin turun neresinde bittiği bilinmez: turun sonu sayılır (güvenli yan).
            self.silent_from = Some(self.written + latency as u64);
        }
    }

    /// Aygıtta henüz çalınmamış `padding` kare varken: sessizlik hoparlöre ulaştı mı?
    pub fn faded_out(&self, padding: u64) -> bool {
        self.silent_from
            .is_some_and(|from| self.written.saturating_sub(padding) >= from)
    }
}

/// Bu platformda ses çıkışı var mı?
pub fn is_available() -> bool {
    cfg!(windows)
}

/// Çıkış iş parçacığını başlatır. Aygıt açılamazsa hatayı hemen döndürür.
///
/// İş parçacığı `shared.stop` işaretlenene ya da şarkı bitene kadar çalışır.
pub fn spawn(
    spec: OutputSpec,
    source: Consumer<Sample>,
    shared: Arc<SharedState>,
) -> Result<JoinHandle<()>, AudioError> {
    // Birim testleri ses aygıtı gerektirmeyen sanal çıkışı kullanır; gerçek
    // aygıt `examples/ses_denemesi.rs` ile denenir.
    #[cfg(test)]
    {
        simulated::spawn(spec, source, shared)
    }
    #[cfg(all(windows, not(test)))]
    {
        wasapi::spawn(spec, source, shared)
    }
    #[cfg(all(not(windows), not(test)))]
    {
        let _ = (spec, source, shared);
        Err(AudioError::OutputUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(format: IntFormat, sample: Sample) -> Vec<u8> {
        let mut out = vec![0u8; format.bytes()];
        format.encode(sample, &mut out);
        out
    }

    #[test]
    fn tamsayiya_cevirme_sarkinin_orneklerini_aynen_verir() {
        // Çözücü 16 bitlik örneği 32768'e, 24 bitliği 8388608'e böler.
        for value in [i16::MIN, -12_345, -1, 0, 1, 12_345, i16::MAX] {
            let sample = f64::from(value) / 32_768.0;
            assert_eq!(encoded(IntFormat::new(16, 16), sample), value.to_le_bytes());
            // Daha geniş biçimde değer aynı, alt bitler sıfır.
            let wide = i32::from(value) << 16;
            assert_eq!(encoded(IntFormat::new(32, 32), sample), wide.to_le_bytes());
            assert_eq!(encoded(IntFormat::new(32, 24), sample), wide.to_le_bytes());
            assert_eq!(
                encoded(IntFormat::new(24, 24), sample),
                (i32::from(value) << 8).to_le_bytes()[..3]
            );
        }
        for value in [-8_388_608i32, -4_000_001, -1, 1, 4_000_001, 8_388_607] {
            let sample = f64::from(value) / 8_388_608.0;
            assert_eq!(
                encoded(IntFormat::new(24, 24), sample),
                value.to_le_bytes()[..3]
            );
            assert_eq!(
                encoded(IntFormat::new(32, 24), sample),
                (value << 8).to_le_bytes()
            );
        }
        let value = -1_234_567_891i32;
        assert_eq!(
            encoded(IntFormat::new(32, 32), f64::from(value) / 2_147_483_648.0),
            value.to_le_bytes()
        );
    }

    #[test]
    fn tamsayiya_cevirme_sinirda_tasmaz() {
        let f = IntFormat::new(16, 16);
        assert_eq!(encoded(f, 1.0), i16::MAX.to_le_bytes());
        assert_eq!(encoded(f, -1.0), i16::MIN.to_le_bytes());
        assert_eq!(encoded(f, 5.0), i16::MAX.to_le_bytes());
        let f = IntFormat::new(32, 32);
        assert_eq!(encoded(f, 1.0), i32::MAX.to_le_bytes());
        assert_eq!(encoded(f, -1.0), i32::MIN.to_le_bytes());
    }

    /// 48 kHz'te 100 ms'lik aygıt arabelleği, 10 ms'lik periyot ve taşma korumasının gecikmesi.
    const BUFFER: u64 = 4_800;
    const PERIOD: usize = 480;
    const LATENCY: usize = 72;

    #[test]
    fn gecis_arabellekteki_ses_calininca_duyulmus_sayilir() {
        let mut watch = FadeWatch::default();
        watch.wrote(BUFFER as usize, false, LATENCY); // çalarken arabellek dolu
                                                      // Duraklatıldı: geçiş bir sonraki turda yazılır, ama önünde 100 ms ses var.
        watch.wrote(PERIOD, true, LATENCY);
        assert!(!watch.faded_out(BUFFER));
        // Her periyotta arabellekten 10 ms çalınır, yerine sessizlik yazılır.
        let mut periods = 0;
        while !watch.faded_out(BUFFER) {
            watch.wrote(PERIOD, true, LATENCY);
            periods += 1;
            assert!(periods < 100, "sessizlik hiç duyulmadı");
        }
        // Eskiden 25 ms beklenip akış kesiliyordu: geçiş henüz duyulmamıştı.
        assert!(periods * 10 >= 100, "{periods} periyot");
        assert!(periods * 10 <= 130, "{periods} periyot");
    }

    #[test]
    fn duraklatilmis_baslayan_akis_hemen_sessiz_sayilir() {
        let mut watch = FadeWatch::default();
        watch.wrote(BUFFER as usize, true, LATENCY);
        assert!(watch.faded_out(BUFFER));
    }

    #[test]
    fn yeniden_calinca_sessizlik_bilgisi_silinir() {
        let mut watch = FadeWatch::default();
        watch.wrote(BUFFER as usize, true, LATENCY);
        watch.wrote(PERIOD, false, LATENCY);
        assert!(!watch.faded_out(0));
    }
}
