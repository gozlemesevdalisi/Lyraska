//! Ses motoru.
//!
//! Veri akışı:
//!
//! ```text
//! dosya ──► decode (symphonia, ayrı iş parçacığı) ──► halka tampon (f64) ──► render (+ eq) ──► output (WASAPI)
//! ```
//!
//! - [`decode`]: symphonia ile dosyayı açar ve 64-bit örneklere çözer.
//! - [`eq`]: 10 bantlı grafik ekolayzer (bantlar arası taşmayı düzelten tasarım, kilitsiz ayar).
//! - [`loudness`]: ses yüksekliği (EBU R128) ve gerçek tepe ölçümü (analizde).
//! - [`normalize`]: çalarken ses yüksekliği eşitlemesi ve ekolayzer için boşluk yönetimi.
//! - [`render`]: halka tampondan aygıt arabelleğini doldurur; ekolayzeri uygular,
//!   duraklatmada yumuşak geçiş yapar.
//!   Gerçek zamanlı iş parçacığında çalışır: bellek ayırmaz, kilit beklemez.
//! - [`output`]: Windows WASAPI çıkışı (paylaşımlı mod; ayarla özel mod, bit-perfect).
//! - [`player`]: yukarıdakileri bir araya getiren, arayüzün kullandığı oynatıcı.
//!
//! Bütün iç işlem [`Sample`] türüyle (64-bit) yapılır; yalnızca çıkışta aygıtın
//! biçimine (paylaşımlı modda 32-bit kayan nokta, özel modda aygıtın tamsayısı)
//! dönüştürülür.

pub mod decode;
pub mod eq;
pub mod gapless;
pub mod limiter;
pub mod loudness;
pub mod normalize;
pub mod output;
pub mod peq;
pub mod player;
pub mod render;
pub mod resample;

use thiserror::Error;

/// İç işlemde kullanılan örnek (sample) türü: 64-bit kayan nokta.
pub type Sample = f64;

/// Ses motoru hataları. Mesajlar doğrudan kullanıcıya gösterilir.
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Dosya açılamadı: {0}")]
    Open(#[from] std::io::Error),
    #[error("Bu dosya biçimi desteklenmiyor ya da dosya bozuk: {0}")]
    Unsupported(String),
    #[error("Dosyada çalınabilir bir ses kanalı bulunamadı.")]
    NoAudioTrack,
    #[error("Şarkı çözülürken hata oluştu: {0}")]
    Decode(String),
    #[error("Bu dosyada istenen yere atlanamadı: {0}")]
    Seek(String),
    #[error("Ses çıkışı bu işletim sisteminde henüz desteklenmiyor (şimdilik yalnızca Windows).")]
    OutputUnavailable,
    #[error("Ses çıkış aygıtı bulunamadı. Hoparlör ya da kulaklık bağlı ve açık mı?")]
    NoOutputDevice,
    #[error("Ses aygıtı hatası: {0}")]
    Output(String),
    /// Özel mod (bit-perfect) açılamadı; oynatıcı paylaşımlı moda geçer.
    #[error("Bit-perfect açılamadı: {0}")]
    Exclusive(String),
}

impl From<symphonia::core::errors::Error> for AudioError {
    fn from(error: symphonia::core::errors::Error) -> Self {
        use symphonia::core::errors::Error as E;
        match error {
            E::IoError(e) => AudioError::Decode(e.to_string()),
            E::Unsupported(what) => AudioError::Unsupported(what.to_owned()),
            other => AudioError::Decode(other.to_string()),
        }
    }
}

/// Ses motorunun durumu (karşılama ekranındaki durum satırı için).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStatus {
    /// Çalmaya hazır (Windows, WASAPI).
    Ready,
    /// Bu platformda ses çıkışı henüz yok.
    OutputUnavailable,
}

impl EngineStatus {
    /// Kullanıcıya gösterilen Türkçe durum metni.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "hazır (WASAPI)",
            Self::OutputUnavailable => "bu sistemde çıkış yok",
        }
    }
}

/// Ses motorunun şu anki durumu.
pub fn status() -> EngineStatus {
    if output::is_available() {
        EngineStatus::Ready
    } else {
        EngineStatus::OutputUnavailable
    }
}

#[cfg(test)]
pub(crate) mod test_util;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ic_islem_64_bit() {
        assert_eq!(std::mem::size_of::<Sample>(), 8);
    }
}

/// Çalma seçenekleri (kalıcı ayar). Yeni alanlar eklendiğinde eski ayar dosyaları
/// varsayılanla tamamlanır.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct PlaybackOptions {
    /// Ses yüksekliği eşitlemesi (EBU R128): bütün şarkılar aynı yükseklikte çalar,
    /// ekolayzere yer açılır. Proje sahibinin kararıyla varsayılan açık.
    pub normalize: bool,
    /// Bit-perfect: aygıt özel modda, şarkının kendi hızında açılır; ses hiç işlenmez
    /// (ekolayzer, kulaklık düzeltmesi, eşitleme devre dışı). Proje sahibinin kararıyla
    /// varsayılan kapalı.
    pub bit_perfect: bool,
}

impl Default for PlaybackOptions {
    fn default() -> Self {
        Self {
            normalize: true,
            bit_perfect: false,
        }
    }
}
