//! Ses çıkışı.
//!
//! Windows'ta WASAPI paylaşımlı mod kullanılır. Diğer sistemlerde çıkış henüz
//! yoktur (macOS/Linux v1.0'dan sonra); motorun geri kalanı yine derlenir ve test edilir.

use std::sync::Arc;
use std::thread::JoinHandle;

use rtrb::Consumer;

use super::player::SharedState;
use super::{AudioError, Sample};

#[cfg(test)]
mod simulated;
#[cfg(all(windows, not(test)))]
mod wasapi;

/// Çıkış akışının biçimi: şarkının kendi örnekleme hızı ve kanal sayısı.
///
/// Aygıtın biçimi farklıysa (ör. 48 kHz) Windows ses motoru dönüştürür.
/// Kendi yüksek kaliteli yeniden örnekleyicimiz ve bit-perfect mod Faz 3'te.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSpec {
    pub sample_rate: u32,
    pub channels: usize,
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
