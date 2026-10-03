//! Şarkı haritası analizi.
//!
//! - [`spectrogram`]: şarkının her anı için frekans bantları ve kanal seviyeleri (Faz 1).
//! - [`levels`]: VU ibreleri için sol/sağ seviyeler ve şarkıya göre 0 VU referansı.
//! - [`beats`]: vuruşlar ve tempo (BPM), spektrogramla aynı geçişte.
//!
//! Faz 2'de sırada: ölçü (ilk vuruş) takibi, bölüm sınırları, drop tespiti ve
//! enerji eğrisi. Analiz şarkı çalmadan önce yapılır ve
//! sonuç SQLite'ta saklanır; böylece görseller şarkıyı "önceden bilir".

pub mod beats;
pub mod levels;
pub mod spectrogram;

/// Analiz modülünün durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisStatus {
    /// Spektrogram ve beat takibi var; şarkı haritasının geri kalanı yolda.
    SpectrumAndBeats,
}

impl AnalysisStatus {
    /// Kullanıcıya gösterilen Türkçe durum metni.
    pub fn label(self) -> &'static str {
        match self {
            Self::SpectrumAndBeats => "spektrum ve beat takibi (şarkı haritası yolda)",
        }
    }
}

/// Analiz modülünün şu anki durumu.
pub fn status() -> AnalysisStatus {
    AnalysisStatus::SpectrumAndBeats
}
