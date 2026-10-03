//! Şarkı haritası analizi.
//!
//! - [`spectrogram`]: şarkının her anı için frekans bantları (Faz 1).
//!
//! Faz 2'de burada şunlar olacak: beat ve ölçü takibi, bölüm sınırları,
//! drop tespiti ve enerji eğrisi. Analiz şarkı çalmadan önce yapılır ve
//! sonuç SQLite'ta saklanır; böylece görseller şarkıyı "önceden bilir".

pub mod spectrogram;

/// Analiz modülünün durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisStatus {
    /// Spektrogram var; şarkı haritası Faz 2'de.
    SpectrumOnly,
}

impl AnalysisStatus {
    /// Kullanıcıya gösterilen Türkçe durum metni.
    pub fn label(self) -> &'static str {
        match self {
            Self::SpectrumOnly => "spektrum (şarkı haritası Faz 2'de)",
        }
    }
}

/// Analiz modülünün şu anki durumu.
pub fn status() -> AnalysisStatus {
    AnalysisStatus::SpectrumOnly
}
