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
