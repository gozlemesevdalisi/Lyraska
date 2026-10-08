//! Ses dosyası çözme (symphonia).
//!
//! Desteklenen biçimler: MP3, FLAC, WAV, AIFF, OGG Vorbis, AAC ve ALAC (M4A/MP4), CAF, MKV/WebM.

use std::fs::File;
use std::path::{Path, PathBuf};

use serde::Serialize;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTag};
use symphonia::core::units::{Time, TimeBase};

use super::gapless::{self, Trim};
use super::{AudioError, Sample};

/// Kodlayıcı dolgusu çözücü tarafından atılmayan, MP4 tabanlı uzantılar.
const MP4_EXTENSIONS: &[&str] = &["m4a", "m4b", "mp4"];

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
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
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
    time_base: Option<TimeBase>,
    info: TrackInfo,
    /// Son çözülen paketin örnekleri (kanallar iç içe: L, R, L, R...).
    buffer: Vec<Sample>,
    /// Sarmadan sonra bu saniyeden önceki örnekler atılır (örnek hassasiyetinde sarma).
    /// Çözülen akışın zamanıdır (kodlayıcı dolgusu dahil).
    skip_until: Option<f64>,
    /// Boşluksuz çalma: baştaki ve sondaki kodlayıcı dolgusu (MP4/AAC).
    trim: Option<Trim>,
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
        let time_base = track.time_base;
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

        // MP3 ve Vorbis'te dolguyu çözücü atar; MP4'te bilgiyi dosyadan biz okuruz.
        let is_mp4 = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| MP4_EXTENSIONS.iter().any(|m| e.eq_ignore_ascii_case(m)));
        let trim = if is_mp4 {
            gapless::mp4_trim(path, sample_rate)
        } else {
            None
        };
        let playable_frames = trim.and_then(|t| t.frames).or(num_frames);

        let tags = read_tags(format.as_mut());
        let file_name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let info = TrackInfo {
            path: path.to_path_buf(),
            file_name,
            title: tags.title,
            artist: tags.artist,
            album: tags.album,
            album_artist: tags.album_artist,
            track_number: tags.track_number,
            disc_number: tags.disc_number,
            codec: decoder.codec_info().short_name.to_owned(),
            sample_rate,
            channels,
            duration_secs: playable_frames.map(|n| n as f64 / f64::from(sample_rate)),
        };

        Ok(Self {
            format,
            decoder,
            track_id,
            time_base,
            info,
            buffer: Vec::new(),
            skip_until: None,
            trim,
        })
    }

    /// Şarkı bilgileri.
    pub fn info(&self) -> &TrackInfo {
        &self.info
    }

    /// Şarkıda verilen saniyeye atlar ve gerçekten atlanan kareyi döndürür.
    ///
    /// Biçimler genellikle hedefin biraz öncesine atlayabilir; aradaki fazla
    /// örnekler sonraki [`Decoder::next_chunk`] çağrılarında atılır.
    pub fn seek(&mut self, seconds: f64) -> Result<u64, AudioError> {
        let rate = f64::from(self.info.sample_rate);
        let last = self
            .info
            .duration_secs
            .map_or(f64::MAX, |d| (d - 1.0 / rate).max(0.0));
        let target = if seconds.is_finite() {
            seconds.clamp(0.0, last)
        } else {
            0.0
        };
        // Akış zamanı = şarkı zamanı + baştaki kodlayıcı dolgusu.
        let stream_target = target + self.trim.map_or(0.0, |t| t.delay as f64 / rate);
        let time = Time::try_from_secs_f64(stream_target)
            .ok_or_else(|| AudioError::Seek("geçersiz konum".to_owned()))?;
        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| AudioError::Seek(e.to_string()))?;
        self.decoder.reset();
        self.skip_until = self.time_base.map(|_| stream_target);
        Ok((target * rate).round() as u64)
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
            let packet_start = self
                .time_base
                .and_then(|tb| tb.calc_time(packet.pts))
                .map(|t| t.as_secs_f64());
            match self.decoder.decode(&packet) {
                Ok(audio) => {
                    let frames = audio.frames();
                    if frames == 0 {
                        continue;
                    }
                    self.buffer.resize(audio.samples_interleaved(), 0.0);
                    audio.copy_to_slice_interleaved(&mut self.buffer);
                    // Bozuk kayan noktalı kayıtlarda NaN ya da sonsuz değer olabilir:
                    // filtrelerin belleğine girerse şarkının geri kalanı bozulur. Sessizlik say.
                    for sample in &mut self.buffer {
                        if !sample.is_finite() {
                            *sample = 0.0;
                        }
                    }

                    // Paketin çalınacak kısmı [from, to): sarma hedefinden ve baştaki
                    // dolgudan önceki örnekler, sondaki dolgu da atılır.
                    let (mut from, mut to) = (0usize, frames);
                    if let Some(start) = packet_start {
                        let rate = f64::from(self.info.sample_rate);
                        let start = (start * rate).round() as i64;
                        let delay = self.trim.map_or(0, |t| t.delay as i64);
                        let skip_to = self
                            .skip_until
                            .map_or(0, |t| (t * rate).round() as i64)
                            .max(delay);
                        from = (skip_to - start).clamp(0, frames as i64) as usize;
                        if let Some(length) = self.trim.and_then(|t| t.frames) {
                            let end = delay + length as i64;
                            if start >= end {
                                return Ok(None);
                            }
                            to = (end - start).clamp(0, frames as i64) as usize;
                        }
                        if from >= to {
                            continue;
                        }
                        self.skip_until = None;
                    }
                    let channels = self.info.channels;
                    return Ok(Some(&self.buffer[from * channels..to * channels]));
                }
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// Kütüphane ve ekran için okunan etiketler.
#[derive(Default)]
struct Tags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    track_number: Option<u32>,
    disc_number: Option<u32>,
}

/// Etiketleri okur (ID3, Vorbis yorumları, MP4 vb.). Aynı etiket birden çok
/// kez varsa ilki alınır.
fn read_tags(format: &mut dyn FormatReader) -> Tags {
    let mut tags = Tags::default();
    let mut metadata = format.metadata();
    if let Some(revision) = metadata.skip_to_latest() {
        let track_tags = revision
            .per_track
            .iter()
            .flat_map(|t| t.metadata.tags.iter());
        for tag in revision.media.tags.iter().chain(track_tags) {
            match &tag.std {
                Some(StandardTag::TrackTitle(v)) if tags.title.is_none() => {
                    tags.title = non_empty(v);
                }
                Some(StandardTag::Artist(v)) if tags.artist.is_none() => {
                    tags.artist = non_empty(v);
                }
                Some(StandardTag::Album(v)) if tags.album.is_none() => {
                    tags.album = non_empty(v);
                }
                Some(StandardTag::AlbumArtist(v)) if tags.album_artist.is_none() => {
                    tags.album_artist = non_empty(v);
                }
                Some(StandardTag::TrackNumber(n)) if tags.track_number.is_none() => {
                    tags.track_number = u32::try_from(*n).ok().filter(|&n| n > 0);
                }
                Some(StandardTag::DiscNumber(n)) if tags.disc_number.is_none() => {
                    tags.disc_number = u32::try_from(*n).ok().filter(|&n| n > 0);
                }
                _ => {}
            }
        }
    }
    tags
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::{sine, temp_path, write_wav};

    /// 32 bit kayan noktalı mono WAV (biçim 3) yazar: bozuk dosyaları taklit etmek için.
    fn write_float_wav(path: &std::path::Path, samples: &[f32]) {
        let data_len = (samples.len() * 4) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8_000u32.to_le_bytes());
        bytes.extend_from_slice(&32_000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn bozuk_sayilar_sessizlige_cevrilir() {
        // Bozuk bir kayıt: araya NaN ve sonsuz değerler karışmış.
        let path = temp_path("bozuk-float.wav");
        let mut samples = vec![0.25f32; 4_000];
        samples[100] = f32::NAN;
        samples[200] = f32::INFINITY;
        samples[300] = f32::NEG_INFINITY;
        write_float_wav(&path, &samples);
        let mut decoder = Decoder::open(&path).unwrap();
        let mut decoded = Vec::new();
        while let Some(chunk) = decoder.next_chunk().unwrap() {
            decoded.extend_from_slice(chunk);
        }
        assert_eq!(decoded.len(), samples.len());
        assert!(
            decoded.iter().all(|s| s.is_finite()),
            "geçersiz sayı kalmamalı"
        );
        assert_eq!(decoded[100], 0.0);
        assert_eq!(decoded[200], 0.0);
        assert_eq!(decoded[300], 0.0);
        assert!((decoded[101] - 0.25).abs() < 1e-6);
        std::fs::remove_file(path).ok();
    }

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
    fn sarma_ornek_hassasiyetinde_calisir() {
        let path = temp_path("sarma.wav");
        let rate = 8_000;
        let frames = 24_000; // 3 saniye
                             // Her kare kendi sırasını taşır: kaç saniyeye atlandığı örnekten okunabilir.
        write_wav(&path, rate, 1, frames, |frame, _| {
            frame as f64 / frames as f64
        });

        let mut decoder = Decoder::open(&path).unwrap();
        let landed = decoder.seek(1.25).unwrap();
        assert_eq!(landed, 10_000);

        let mut samples = Vec::new();
        while let Some(chunk) = decoder.next_chunk().unwrap() {
            samples.extend_from_slice(chunk);
        }
        // Atlanan noktadan sona kadar tam olarak kalan örnekler gelir.
        assert_eq!(samples.len(), frames - 10_000);
        assert!(
            (samples[0] - 10_000.0 / frames as f64).abs() < 1e-4,
            "ilk örnek {}",
            samples[0]
        );

        // Sona taşan ve negatif konumlar sınırlanır.
        assert!(decoder.seek(99.0).unwrap() < frames as u64);
        assert_eq!(decoder.seek(-5.0).unwrap(), 0);
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
