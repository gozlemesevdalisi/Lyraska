//! Ses motoru.
//!
//! Faz 1'de burada şunlar olacak:
//! - `decode`: symphonia ile dosya çözme
//! - `dsp`: 64-bit kayan noktalı iç işlem zinciri (EQ, ses seviyesi, loudness)
//! - `output`: wasapi ile Windows ses çıkışı (paylaşımlı ve bit-perfect özel mod)
//!
//! Bütün iç işlem [`Sample`] türüyle (64-bit) yapılır; yalnızca çıkışta aygıtın
//! biçimine dönüştürülür.

/// İç işlemde kullanılan örnek (sample) türü: 64-bit kayan nokta.
pub type Sample = f64;

/// Ses motorunun durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStatus {
    /// Henüz yazılmadı (Faz 0).
    NotImplemented,
}

impl EngineStatus {
    /// Kullanıcıya gösterilen Türkçe durum metni.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotImplemented => "Faz 1'de geliyor",
        }
    }
}

/// Ses motorunun şu anki durumu.
pub fn status() -> EngineStatus {
    EngineStatus::NotImplemented
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ic_islem_64_bit() {
        assert_eq!(std::mem::size_of::<Sample>(), 8);
    }
}
