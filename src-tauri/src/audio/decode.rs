//! Ses dosyası çözme (symphonia).
//!
//! Desteklenen biçimler: MP3, FLAC, WAV, AIFF, OGG Vorbis, AAC ve ALAC (M4A/MP4), CAF, MKV/WebM.

use std::fs::File;
use std::path::{Path, PathBuf};

use serde::Serialize;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTag};

use super::{AudioError, Sample};

/// "Dosya aç" penceresinde gösterilen uzantılar.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "wave", "aif", "aiff", "aifc", "ogg", "oga", "m4a", "m4b", "mp4", "aac",
    "caf", "mka", "webm",
];

/// Açılan şarkının bilgileri.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub path: PathBuf,
    /// Dosya adı (uzantısız); etiket yoksa başlık olarak kullanılır.
    pub file_name: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// Kodek adı (ör. "flac", "mp3").
    pub codec: String,
    pub sample_rate: u32,
    pub channels: usize,
    /// Süre (saniye); dosya bildirmiyorsa `None`.
    pub duration_secs: Option<f64>,
}

/// Bir ses dosyasını parça parça 64-bit örneklere çözer.
pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    info: TrackInfo,
    /// Son çözülen paketin örnekleri (kanallar iç içe: L, R, L, R...).
    buffer: Vec<Sample>,
}

impl Decoder {
    /// Dosyayı açar, biçimini tanır ve çözücüyü hazırlar.
    pub fn open(path: &Path) -> Result<Self, AudioError> {
        let file = File::open(path)?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let mut format = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| match e {
                SymphoniaError::IoError(io) => AudioError::Unsupported(io.to_string()),
                other => AudioError::from(other),
            })?;

        let track = format
            .default_track(TrackType::Audio)
            .ok_or(AudioError::NoAudioTrack)?;
        let track_id = track.id;
        let num_frames = track.num_frames;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or(AudioError::NoAudioTrack)?
            .clone();

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())?;

        let sample_rate = params
            .sample_rate
            .ok_or_else(|| AudioError::Unsupported("örnekleme hızı bilinmiyor".to_owned()))?;
        let channels = params
            .channels
            .as_ref()
            .map(|c| c.count())
            .filter(|&c| c > 0)
            .ok_or_else(|| AudioError::Unsupported("kanal sayısı bilinmiyor".to_owned()))?;

        let (title, artist) = read_tags(format.as_mut());
        let file_name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let info = TrackInfo {
            path: path.to_path_buf(),
            file_name,
            title,
            artist,
            codec: decoder.codec_info().short_name.to_owned(),
            sample_rate,
            channels,
            duration_secs: num_frames.map(|n| n as f64 / f64::from(sample_rate)),
        };

        Ok(Self {
            format,
            decoder,
            track_id,
            info,
            buffer: Vec::new(),
        })
    }

    /// Şarkı bilgileri.
    pub fn info(&self) -> &TrackInfo {
        &self.info
    }

    /// Sonraki örnek dilimini döndürür; şarkı bittiyse `None`.
    ///
    /// Bozuk tek bir paket şarkıyı durdurmaz: atlanır ve sonrakine geçilir.
    pub fn next_chunk(&mut self) -> Result<Option<&[Sample]>, AudioError> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(None),
                // Bazı biçimlerde dosya sonu G/Ç hatası olarak bildirilir.
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None)
                }
                Err(e) => return Err(e.into()),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(audio) => {
                    if audio.frames() == 0 {
                        continue;
                    }
                    self.buffer.resize(audio.samples_interleaved(), 0.0);
                    audio.copy_to_slice_interleaved(&mut self.buffer);
                    return Ok(Some(&self.buffer));
                }
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// Başlık ve sanatçı etiketlerini okur (ID3, Vorbis yorumları, MP4 vb.).
fn read_tags(format: &mut dyn FormatReader) -> (Option<String>, Option<String>) {
    let mut title = None;
    let mut artist = None;
    let mut metadata = format.metadata();
    if let Some(revision) = metadata.skip_to_latest() {
        let track_tags = revision
            .per_track
            .iter()
            .flat_map(|t| t.metadata.tags.iter());
        for tag in revision.media.tags.iter().chain(track_tags) {
            match &tag.std {
                Some(StandardTag::TrackTitle(value)) if title.is_none() => {
                    title = non_empty(value);
                }
                Some(StandardTag::Artist(value)) if artist.is_none() => {
                    artist = non_empty(value);
                }
                _ => {}
            }
        }
    }
    (title, artist)
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::{sine, temp_path, write_wav};

    #[test]
    fn wav_dosyasini_dogru_cozer() {
        let path = temp_path("cozme.wav");
        let frames = 22_050; // 0,5 saniye
        write_wav(&path, 44_100, 2, frames, |frame, channel| {
            sine(440.0, 44_100, frame) * if channel == 0 { 0.5 } else { -0.5 }
        });

        let mut decoder = Decoder::open(&path).unwrap();
        let info = decoder.info().clone();
        assert_eq!(info.sample_rate, 44_100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.file_name, "cozme");
        assert!((info.duration_secs.unwrap() - 0.5).abs() < 1e-6);

        let mut samples = Vec::new();
        while let Some(chunk) = decoder.next_chunk().unwrap() {
            samples.extend_from_slice(chunk);
        }
        assert_eq!(samples.len(), frames * 2);

        // 16 bit'lik nicemleme hatası payıyla karşılaştır.
        for frame in [0, 1, 100, 12_345, frames - 1] {
            let expected = sine(440.0, 44_100, frame) * 0.5;
            assert!((samples[frame * 2] - expected).abs() < 1e-4, "kare {frame}");
            assert!(
                (samples[frame * 2 + 1] + expected).abs() < 1e-4,
                "kare {frame}"
            );
        }
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn olmayan_dosya_acma_hatasi_verir() {
        let error = Decoder::open(Path::new("/olmayan/klasor/sarki.mp3"))
            .err()
            .unwrap();
        assert!(matches!(error, AudioError::Open(_)));
    }

    #[test]
    fn ses_olmayan_dosya_desteklenmiyor_hatasi_verir() {
        let path = temp_path("metin.mp3");
        std::fs::write(&path, b"bu bir ses dosyasi degil".repeat(100)).unwrap();
        let error = Decoder::open(&path).err().unwrap();
        assert!(matches!(error, AudioError::Unsupported(_)), "{error:?}");
        std::fs::remove_file(path).ok();
    }
}
