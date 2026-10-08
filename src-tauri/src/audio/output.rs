//! Ses çıkışı.
//!
//! Windows'ta WASAPI paylaşımlı mod kullanılır. Diğer sistemlerde çıkış henüz
//! yoktur (macOS/Linux v1.0'dan sonra); motorun geri kalanı yine derlenir ve test edilir.
//!
//! Akış, aygıtın kendi örnekleme hızında açılır ([`device_info`]); şarkı o hıza
//! [`super::resample`] ile çevrilir. Böylece Windows ses motoru sese dokunmaz.

use std::sync::Arc;
use std::thread::JoinHandle;

use rtrb::Consumer;

use super::player::SharedState;
use super::{AudioError, Sample};

#[cfg(test)]
pub(crate) mod simulated;
#[cfg(all(windows, not(test)))]
mod wasapi;

/// Çıkış akışının biçimi: aygıtın örnekleme hızı ve akışın kanal sayısı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSpec {
    pub sample_rate: u32,
    pub channels: usize,
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
