//! Windows WASAPI çıkışı.
//!
//! - **Paylaşımlı mod** (olay güdümlü): ses motoru her periyotta (genellikle 10 ms) bir
//!   olay gönderir; arabellekteki boş yer halka tampondan doldurulur. Arabellek 100 ms'dir:
//!   işletim sistemi iş parçacığımızı kısa süre geciktirse bile ses kesilmez.
//! - **Özel mod** (bit-perfect, yoklamalı): aygıt yalnızca Lyraska'ya ayrılır; şarkının
//!   hızında ve aygıtın kabul ettiği tamsayı biçiminde açılır. Olay güdümlü özel mod bazı
//!   USB aygıtlarda takılmaya yol açtığı için yoklama kullanılır: arabellek ~100 ms, aygıtın
//!   periyodunun yarısında bir doldurulur.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::Consumer;
use wasapi::{
    initialize_mta, AudioClient, Device, DeviceEnumerator, Direction, Handle, SampleType,
    StreamMode, WasapiError, WaveFormat,
};

use super::{DeviceInfo, FadeWatch, IntFormat, OutputMode, OutputSpec};
use crate::audio::player::SharedState;
use crate::audio::render::Renderer;
use crate::audio::{AudioError, Sample};

/// Aygıt arabelleğinin süresi (100 ns birimi): 100 ms.
const BUFFER_DURATION_HNS: i64 = 1_000_000;
/// Bu süre içinde olay gelmezse aygıt kopmuş sayılır.
const EVENT_TIMEOUT_MS: u32 = 2_000;
/// Şarkı sonunda aygıttaki son örneklerin çalınmasını beklerken yoklama aralığı.
const DRAIN_POLL: Duration = Duration::from_millis(5);
/// Intel HDA uyumlu aygıtlar özel modda arabelleğin 128 baytın katı olmasını ister.
const EXCLUSIVE_ALIGN_BYTES: u32 = 128;

/// WASAPI hata kodları (`AUDCLNT_E_…`, audioclient.h).
const AUDCLNT_E_UNSUPPORTED_FORMAT: u32 = 0x8889_0008;
const AUDCLNT_E_DEVICE_IN_USE: u32 = 0x8889_000A;
const AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED: u32 = 0x8889_000E;

type Ready = SyncSender<Result<(), AudioError>>;

/// WASAPI hatasını kullanıcıya gösterilecek Türkçe bir mesajla sarar.
fn fail(context: &'static str) -> impl FnOnce(WasapiError) -> AudioError {
    move |error| AudioError::Output(format!("{context}: {error}"))
}

/// Windows hata kodu (HRESULT), varsa.
fn hresult(error: &WasapiError) -> Option<u32> {
    match error {
        WasapiError::Windows(e) => Some(e.code().0 as u32),
        _ => None,
    }
}

/// Özel modun açılamama nedeni, kullanıcının anlayacağı biçimde.
fn exclusive_reason(error: &WasapiError) -> String {
    match hresult(error) {
        Some(AUDCLNT_E_DEVICE_IN_USE) => {
            "aygıtı başka bir program özel modda kullanıyor".to_owned()
        }
        Some(AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED) => "Windows bu aygıtta özel moda izin vermiyor \
             (Ses ayarları > aygıt > Gelişmiş: \"Uygulamaların bu cihazın özel denetimini \
             almasına izin ver\")"
            .to_owned(),
        Some(AUDCLNT_E_UNSUPPORTED_FORMAT) => "aygıt bu biçimi özel modda desteklemiyor".to_owned(),
        _ => format!("özel mod açılamadı ({error})"),
    }
}

/// Örnekleme hızı, Türkçe yazımla ("44,1 kHz").
fn khz(sample_rate: u32) -> String {
    format!("{} kHz", f64::from(sample_rate) / 1000.0).replace('.', ",")
}

fn int_wave_format(format: IntFormat, sample_rate: u32, channels: usize) -> WaveFormat {
    WaveFormat::new(
        usize::from(format.container_bits),
        usize::from(format.valid_bits),
        &SampleType::Int,
        sample_rate as usize,
        channels,
        None,
    )
}

fn default_device() -> Result<Device, AudioError> {
    let enumerator = DeviceEnumerator::new().map_err(fail("Ses aygıtları listelenemedi"))?;
    enumerator
        .get_default_device(&Direction::Render)
        .map_err(|_| AudioError::NoOutputDevice)
}

/// Varsayılan çıkış aygıtının adı ve paylaşımlı mod biçimi (mix format).
pub fn device_info() -> Option<DeviceInfo> {
    // COM bu iş parçacığında başka kipte başlatılmış olabilir; aygıt sorgusu yine çalışır.
    let _ = initialize_mta();
    let device = default_device().ok()?;
    let name = device.get_friendlyname().unwrap_or_default();
    let format = device.get_iaudioclient().ok()?.get_mixformat().ok()?;
    Some(DeviceInfo {
        name,
        sample_rate: format.get_samplespersec(),
        channels: usize::from(format.get_nchannels()),
    })
}

/// Varsayılan aygıtın bu hızda ve kanal sayısında özel modda kabul ettiği en iyi biçim.
pub fn exclusive_format(sample_rate: u32, channels: usize) -> Result<IntFormat, String> {
    let _ = initialize_mta();
    let device = default_device().map_err(|e| e.to_string())?;
    let client = device
        .get_iaudioclient()
        .map_err(|e| exclusive_reason(&e))?;
    IntFormat::CANDIDATES
        .into_iter()
        .find(|&format| {
            client
                .is_supported_exclusive_with_quirks(&int_wave_format(format, sample_rate, channels))
                .is_ok()
        })
        .ok_or_else(|| {
            format!(
                "aygıt {} ve {channels} kanalı özel modda desteklemiyor",
                khz(sample_rate)
            )
        })
}

pub fn spawn(
    spec: OutputSpec,
    source: Consumer<Sample>,
    shared: Arc<SharedState>,
) -> Result<JoinHandle<()>, AudioError> {
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let handle = std::thread::Builder::new()
        .name("lyraska-ses-cikisi".to_owned())
        .spawn(move || {
            let mut ready = Some(ready_tx);
            if let Err(error) = run(spec, source, &shared, &mut ready) {
                match ready.take() {
                    // Açılışta hata: oynatıcıya hemen bildir.
                    Some(tx) => {
                        let _ = tx.send(Err(error));
                    }
                    // Çalarken hata: arayüz durum sorgusunda görür.
                    None => shared.fail(error.to_string()),
                }
            }
        })
        .map_err(|e| AudioError::Output(e.to_string()))?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(error)) => {
            let _ = handle.join();
            Err(error)
        }
        Err(_) => {
            let _ = handle.join();
            Err(AudioError::Output(
                "ses iş parçacığı beklenmedik şekilde kapandı".to_owned(),
            ))
        }
    }
}

/// Açılmış akış: istemci, biçim ve nasıl beklendiği.
struct Stream {
    client: AudioClient,
    /// Paylaşımlı modda ses motorunun olayı; özel modda yoklanır (`None`).
    event: Option<Handle>,
    /// Özel modda yoklama aralığı.
    poll: Duration,
    /// Aygıta yazılan örnek: `None` 32-bit kayan nokta, `Some` tamsayı.
    int: Option<IntFormat>,
    block_align: usize,
}

/// Paylaşımlı mod: şarkının (aygıta çevrilmiş) biçiminde 32-bit kayan nokta akış.
fn open_shared(device: &Device, spec: OutputSpec) -> Result<Stream, AudioError> {
    let mut client = device
        .get_iaudioclient()
        .map_err(fail("Ses aygıtı açılamadı"))?;
    // Akış zaten aygıtın hızında; değilse (aygıt bilgisi alınamadıysa) Windows dönüştürür.
    let format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        spec.sample_rate as usize,
        spec.channels,
        None,
    );
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: BUFFER_DURATION_HNS,
    };
    client
        .initialize_client(&format, &Direction::Render, &mode)
        .map_err(fail("Ses akışı başlatılamadı"))?;
    let event = client
        .set_get_eventhandle()
        .map_err(fail("Ses olayı oluşturulamadı"))?;
    Ok(Stream {
        block_align: format.get_blockalign() as usize,
        client,
        event: Some(event),
        poll: DRAIN_POLL,
        int: None,
    })
}

/// Özel mod (bit-perfect): şarkının hızında, aygıtın tamsayı biçiminde. Açılamazsa
/// [`AudioError::Exclusive`] döner; oynatıcı paylaşımlı moda geçer.
fn open_exclusive(device: &Device, spec: OutputSpec, int: IntFormat) -> Result<Stream, AudioError> {
    let exclusive = |e: WasapiError| AudioError::Exclusive(exclusive_reason(&e));
    let mut client = device.get_iaudioclient().map_err(exclusive)?;
    // Aygıt biçimi kanal maskesi ya da yapı türüyle kabul etmiş olabilir: kabul edileni kullan.
    let format = client
        .is_supported_exclusive_with_quirks(&int_wave_format(int, spec.sample_rate, spec.channels))
        .map_err(exclusive)?;
    let (default_period, _) = client.get_device_period().map_err(exclusive)?;
    let period = client
        .calculate_aligned_period_near(default_period, Some(EXCLUSIVE_ALIGN_BYTES), &format)
        .map_err(exclusive)?
        .max(1);
    // Arabellek periyodun tam katı ve en az ~100 ms (hizası da periyodunki gibi kalır).
    let periods = (BUFFER_DURATION_HNS + period - 1) / period;
    let mode = StreamMode::PollingExclusive {
        period_hns: period,
        buffer_duration_hns: period * periods.max(2),
    };
    client
        .initialize_client(&format, &Direction::Render, &mode)
        .map_err(exclusive)?;
    Ok(Stream {
        block_align: format.get_blockalign() as usize,
        client,
        event: None,
        // 100 ns birimindeki periyodun yarısı.
        poll: Duration::from_nanos((period as u64 * 100 / 2).max(1_000_000)),
        int: Some(int),
    })
}

fn run(
    spec: OutputSpec,
    mut source: Consumer<Sample>,
    shared: &SharedState,
    ready: &mut Option<Ready>,
) -> Result<(), AudioError> {
    initialize_mta()
        .ok()
        .map_err(|e| AudioError::Output(format!("COM başlatılamadı: {e}")))?;

    let device = default_device()?;
    let stream = match spec.mode {
        OutputMode::Shared => open_shared(&device, spec)?,
        OutputMode::Exclusive(int) => open_exclusive(&device, spec, int)?,
    };
    let client = &stream.client;
    let render_client = client
        .get_audiorenderclient()
        .map_err(fail("Ses yazıcısı alınamadı"))?;
    let buffer_frames = client
        .get_buffer_size()
        .map_err(fail("Arabellek boyutu alınamadı"))? as usize;

    // Bütün bellek burada, bir kez ayrılır; döngüde ayırma yapılmaz.
    let mut samples: Vec<Sample> = vec![0.0; buffer_frames * spec.channels];
    let mut bytes = vec![0u8; buffer_frames * stream.block_align];
    let mut renderer = Renderer::new(spec.channels, spec.sample_rate, shared.controls.clone());
    let mut consumed: u64 = 0;
    let mut started = false;
    // Şarkı bitince taşma korumasının gecikme hattında kalan son kareler de yazılır.
    let mut tail_left = renderer.latency_frames();
    // Duraklatma geçişi hoparlöre ulaştı mı (oturum ancak o zaman kapatılır)?
    let mut fade = FadeWatch::default();

    if let Some(tx) = ready.take() {
        let _ = tx.send(Ok(()));
    }

    let result = loop {
        if shared.stop.load(Ordering::Acquire) {
            break Ok(());
        }

        // Şarkı tamamen çözüldü ve tampon boşaldıysa yeni veri yazma; aygıtta
        // kalan son örneklerin çalınmasını bekle. (Sessizlik yazmaya devam etseydik
        // arabellek hiç boşalmaz, "bitti" durumuna geçilemezdi.)
        let source_done = shared.decode_done.load(Ordering::Acquire) && source.is_empty();
        let draining = source_done && tail_left == 0;

        if !draining {
            let space = client
                .get_available_space_in_frames()
                .map_err(fail("Ses aygıtı okunamadı"))? as usize;
            let mut frames = space.min(buffer_frames);
            if source_done {
                // Sona sessizlik ekleme: yalnızca gecikme hattındaki son kareler.
                frames = frames.min(tail_left);
                tail_left -= frames;
            }
            if frames > 0 {
                let out = &mut samples[..frames * spec.channels];
                let paused = shared.paused.load(Ordering::Acquire);
                let outcome = renderer.render(&mut source, out, paused);
                // Aygıtın biçimine çevirme yalnızca burada yapılır.
                match stream.int {
                    None => {
                        for (dst, &sample) in bytes.chunks_exact_mut(4).zip(out.iter()) {
                            dst.copy_from_slice(&(sample as f32).to_le_bytes());
                        }
                    }
                    Some(int) => {
                        for (dst, &sample) in bytes.chunks_exact_mut(int.bytes()).zip(out.iter()) {
                            int.encode(sample, dst);
                        }
                    }
                }
                render_client
                    .write_to_device(frames, &bytes[..frames * stream.block_align], None)
                    .map_err(fail("Ses aygıtına yazılamadı"))?;
                consumed += outcome.frames_consumed as u64;
                if outcome.frames_missing > 0 && !shared.decode_done.load(Ordering::Acquire) {
                    shared.underruns.fetch_add(1, Ordering::Relaxed);
                }
                let silent = paused && renderer.gain() == 0.0;
                fade.wrote(frames, silent, renderer.latency_frames());
            }
        }

        // Arabellek ilk kez dolduktan sonra başlat: çalma sessizlikle başlamasın.
        if !started {
            client
                .start_stream()
                .map_err(fail("Ses akışı başlatılamadı"))?;
            started = true;
        }

        // Duyulan çıkış karesi ≈ halka tampondan alınan - henüz aygıtta bekleyen -
        // taşma korumasının gecikmesi. (Oynatıcı bunu çalan şarkıya ve şarkıdaki
        // konuma çevirir; boşluksuz geçişte şarkı sınırını da bilir.) Duraklatmada aygıta sessizlik yazıldığı için bu
        // tahmin bir an geriye kayabilir; oturum içinde geri gidiş olmadığından
        // (sarma yeni oturum açar) konum hiç azaltılmaz.
        let padding = client
            .get_current_padding()
            .map_err(fail("Ses aygıtı okunamadı"))? as u64;
        shared
            .faded_out
            .store(fade.faded_out(padding), Ordering::Release);
        let latency = padding + renderer.latency_frames() as u64;
        shared
            .output_heard
            .fetch_max(consumed.saturating_sub(latency), Ordering::AcqRel);

        if draining {
            if padding == 0 {
                // Her şey çalındı: konum tam şarkı sonu.
                shared.output_heard.fetch_max(consumed, Ordering::AcqRel);
                shared.ended.store(true, Ordering::Release);
                break Ok(());
            }
            std::thread::sleep(DRAIN_POLL);
            continue;
        }

        match &stream.event {
            Some(event) => {
                if event.wait_for_event(EVENT_TIMEOUT_MS).is_err() {
                    break Err(AudioError::Output(
                        "aygıt yanıt vermiyor; bağlantısı kesilmiş olabilir".to_owned(),
                    ));
                }
            }
            // Özel mod (yoklama): arabellek ~100 ms; yarım periyotta bir doldurulur.
            None => std::thread::sleep(stream.poll),
        }
    };

    if started {
        let _ = client.stop_stream();
    }
    result
}
