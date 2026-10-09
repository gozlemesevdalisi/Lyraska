//! Şarkıyı ses aygıtının biçimine çevirme: örnekleme hızı ve kanal sayısı.
//!
//! Windows paylaşımlı modda ses motoru aygıtın kendi hızında (genellikle 48 kHz)
//! çalışır. Şarkı başka hızdaysa (ör. 44,1 kHz) dönüştürmeyi Windows'a bırakmak
//! yerine burada, yüksek kaliteli bir FFT yeniden örnekleyiciyle (`rubato`,
//! Blackman-Harris pencereli; geçiş bandı ~20 kHz'e kadar düz) yaparız. Böylece
//! Windows ses motoru sese hiç dokunmaz.
//!
//! Tek kanallı (mono) şarkılar iki kanala çoğaltılır: Windows'un mono akışı
//! hangi hoparlöre vereceği aygıta göre değişir.
//!
//! Çözücü iş parçacığında çalışır (gerçek zamanlı değil); bellek ayırabilir.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler, WindowFunction};

use super::{AudioError, Sample};

/// Yeniden örnekleyicinin işlediği blok (giriş karesi).
const CHUNK_FRAMES: usize = 2048;

/// Şarkı biçiminden aygıt biçimine dönüştürücü.
pub struct Converter {
    channels_in: usize,
    channels_out: usize,
    resampler: Option<Fft<Sample>>,
    /// Yeniden örnekleyiciye verilmeyi bekleyen (kanal çevrilmiş) giriş.
    input: Vec<Sample>,
    /// Bir bloğun çıktısı.
    block: Vec<Sample>,
    /// Çağırana verilen çıktı.
    output: Vec<Sample>,
    /// Başta atılacak gecikme karesi (yeniden örnekleyicinin gecikmesi).
    delay_left: usize,
    /// Şimdiye kadar alınan giriş ve verilen çıkış karesi (sondaki uzunluk için).
    frames_in: u64,
    frames_out: u64,
    rate_in: u32,
    rate_out: u32,
}

impl Converter {
    pub fn new(
        rate_in: u32,
        channels_in: usize,
        rate_out: u32,
        channels_out: usize,
    ) -> Result<Self, AudioError> {
        let resampler = if rate_in == rate_out {
            None
        } else {
            Some(
                Fft::<Sample>::new_custom(
                    rate_in as usize,
                    rate_out as usize,
                    CHUNK_FRAMES,
                    1,
                    channels_out,
                    WindowFunction::BlackmanHarris2,
                    FixedSync::Input,
                )
                .map_err(|e| AudioError::Output(format!("yeniden örnekleyici kurulamadı: {e}")))?,
            )
        };
        let delay_left = resampler.as_ref().map_or(0, |r| r.output_delay());
        Ok(Self {
            channels_in: channels_in.max(1),
            channels_out: channels_out.max(1),
            resampler,
            input: Vec::new(),
            block: Vec::new(),
            output: Vec::new(),
            delay_left,
            frames_in: 0,
            frames_out: 0,
            rate_in,
            rate_out,
        })
    }

    /// Aygıta gidecek kanal sayısı: mono iki kanala çoğaltılır, diğerleri aynen.
    pub fn output_channels(channels_in: usize) -> usize {
        if channels_in == 1 {
            2
        } else {
            channels_in
        }
    }

    /// Bir dilim şarkı örneğini dönüştürür; hazır çıktıyı döndürür (boş olabilir).
    pub fn process(&mut self, chunk: &[Sample]) -> Result<&[Sample], AudioError> {
        self.output.clear();
        let frames = chunk.len() / self.channels_in;
        self.frames_in += frames as u64;
        if self.resampler.is_none() {
            map_channels(chunk, self.channels_in, self.channels_out, &mut self.output);
            self.frames_out += frames as u64;
            return Ok(&self.output);
        }
        map_channels(chunk, self.channels_in, self.channels_out, &mut self.input);
        self.run_blocks(false)?;
        Ok(&self.output)
    }

    /// Şarkı bitince kalan örnekleri verir. Toplam uzunluk giriş süresine denk gelir.
    pub fn finish(&mut self) -> Result<&[Sample], AudioError> {
        self.output.clear();
        if self.resampler.is_none() {
            return Ok(&self.output);
        }
        self.run_blocks(true)?;
        Ok(&self.output)
    }

    fn run_blocks(&mut self, flush: bool) -> Result<(), AudioError> {
        let Some(resampler) = self.resampler.as_mut() else {
            return Ok(());
        };
        let channels = self.channels_out;
        let expected_total = (self.frames_in as f64 * f64::from(self.rate_out)
            / f64::from(self.rate_in))
        .round() as u64;
        loop {
            let need = resampler.input_frames_next();
            let available = self.input.len() / channels;
            let partial = if available >= need {
                None
            } else if flush && (available > 0 || self.frames_out < expected_total) {
                Some(available)
            } else {
                break;
            };

            // Kısmi blokta giriş tamponu yine tam blok boyunda olmalı.
            self.input
                .resize(self.input.len().max(need * channels), 0.0);
            self.block
                .resize(resampler.output_frames_max() * channels, 0.0);
            let input = InterleavedSlice::new(&self.input[..need * channels], channels, need)
                .map_err(|e| AudioError::Output(e.to_string()))?;
            let capacity = self.block.len() / channels;
            let mut output = InterleavedSlice::new_mut(&mut self.block, channels, capacity)
                .map_err(|e| AudioError::Output(e.to_string()))?;
            let indexing = partial.map(|n| Indexing::new().partial_len(n));
            let (_, written) = resampler
                .process_into_buffer(&input, &mut output, indexing.as_ref())
                .map_err(|e| AudioError::Output(e.to_string()))?;

            let consumed = partial.unwrap_or(need);
            self.input.drain(..consumed * channels);
            if partial.is_some() {
                self.input.clear();
            }

            let mut produced = &self.block[..written * channels];
            let skip = self.delay_left.min(written);
            self.delay_left -= skip;
            produced = &produced[skip * channels..];
            if flush {
                // Sonda giriş süresinden uzun çıktı verme (yeniden örnekleyicinin kuyruğu).
                let room = expected_total.saturating_sub(self.frames_out) as usize;
                produced = &produced[..produced.len().min(room * channels)];
            }
            self.frames_out += (produced.len() / channels) as u64;
            self.output.extend_from_slice(produced);
            if flush && self.frames_out >= expected_total {
                break;
            }
        }
        Ok(())
    }
}

/// Kanal çevirme: mono → stereo çoğaltma; kanal sayısı aynıysa kopyalama.
fn map_channels(input: &[Sample], from: usize, to: usize, out: &mut Vec<Sample>) {
    if from == to {
        out.extend_from_slice(input);
        return;
    }
    for frame in input.chunks_exact(from) {
        for channel in 0..to {
            out.push(frame[channel.min(from - 1)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::fit;
    use std::f64::consts::PI;

    fn sine(freq: f64, rate: u32, frames: usize) -> Vec<Sample> {
        (0..frames)
            .map(|i| (2.0 * PI * freq * i as f64 / f64::from(rate)).sin() * 0.5)
            .collect()
    }

    fn convert_all(converter: &mut Converter, input: &[Sample], chunk: usize) -> Vec<Sample> {
        let mut out = Vec::new();
        for piece in input.chunks(chunk) {
            out.extend_from_slice(converter.process(piece).unwrap());
        }
        out.extend_from_slice(converter.finish().unwrap());
        out
    }

    #[test]
    fn ayni_hizda_dokunmaz_mono_ikiye_cogalir() {
        let mut same = Converter::new(44_100, 2, 44_100, 2).unwrap();
        let input: Vec<Sample> = (0..100).map(|i| i as f64 / 100.0).collect();
        assert_eq!(convert_all(&mut same, &input, 33), input);

        let mut mono = Converter::new(48_000, 1, 48_000, 2).unwrap();
        let out = convert_all(&mut mono, &[0.1, 0.2, 0.3], 2);
        assert_eq!(out, vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3]);
        assert_eq!(Converter::output_channels(1), 2);
        assert_eq!(Converter::output_channels(6), 6);
    }

    #[test]
    fn yuksek_kaliteli_donusum_ton_ve_tizi_korur() {
        for (from, to) in [
            (44_100, 48_000),
            (48_000, 44_100),
            (96_000, 48_000),
            (22_050, 48_000),
        ] {
            for freq in [1000.0, 10_000.0, 19_000.0] {
                if freq > f64::from(from.min(to)) * 0.45 {
                    continue;
                }
                let input = sine(freq, from, from as usize * 2);
                let mut converter = Converter::new(from, 1, to, 1).unwrap();
                let out = convert_all(&mut converter, &input, 1000);
                // Süre korunur.
                let expected =
                    (input.len() as f64 * f64::from(to) / f64::from(from)).round() as usize;
                assert_eq!(out.len(), expected, "{from}→{to}");
                // Kenarları bırakıp ortayı ölç: frekans (ton) aynı, genlik aynı, bozulma yok.
                let middle = &out[to as usize / 4..out.len() - to as usize / 4];
                let (amplitude, noise_db) = fit(middle, freq, to);
                let loss_db = 20.0 * (amplitude / 0.5).log10();
                assert!(
                    loss_db.abs() < 0.1,
                    "{from}→{to} {freq} Hz: {loss_db:.3} dB"
                );
                assert!(
                    noise_db < -100.0,
                    "{from}→{to} {freq} Hz: bozulma {noise_db:.1} dB"
                );
            }
        }
    }

    #[test]
    fn yeni_hizin_siniri_ustundeki_tonlar_bastirilir() {
        // 48 kHz'teki 23 kHz ton, 44,1 kHz'e inerken (sınır 22,05 kHz) duyulabilir bir
        // yansımaya (21,1 kHz) dönüşmemeli; sınırın altındaki 20 kHz ise yerinde kalmalı.
        let (from, to) = (48_000u32, 44_100u32);
        let rms_db = |signal: &[Sample]| {
            let mean = signal.iter().map(|x| x * x).sum::<f64>() / signal.len() as f64;
            10.0 * mean.log10()
        };
        let reference = rms_db(&sine(1000.0, from, from as usize));
        for (freq, keep) in [(23_000.0, false), (20_000.0, true)] {
            let input = sine(freq, from, from as usize * 2);
            let mut converter = Converter::new(from, 1, to, 1).unwrap();
            let out = convert_all(&mut converter, &input, 1000);
            let middle = &out[to as usize / 4..out.len() - to as usize / 4];
            let level = rms_db(middle) - reference;
            if keep {
                assert!(level.abs() < 0.1, "{freq} Hz: {level:.2} dB");
            } else {
                assert!(level < -MIN_ALIAS_REJECTION_DB, "{freq} Hz: {level:.1} dB");
            }
        }
    }

    /// Sınırın üstündeki tonların en az bu kadar bastırılması beklenir (dB).
    const MIN_ALIAS_REJECTION_DB: f64 = 140.0;

    #[test]
    fn gecikme_telafi_edilir() {
        // 0,5. saniyedeki tek darbe çıkışta da 0,5. saniyede olmalı (konum ve senkron için).
        let (from, to) = (44_100u32, 48_000u32);
        let mut input = vec![0.0; from as usize];
        input[from as usize / 2] = 1.0;
        let mut converter = Converter::new(from, 1, to, 1).unwrap();
        let out = convert_all(&mut converter, &input, 777);
        let peak = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap()
            .0;
        assert!(
            (peak as i64 - to as i64 / 2).abs() <= 1,
            "darbe {peak}. örnekte"
        );
    }
}
