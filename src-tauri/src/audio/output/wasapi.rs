//! Windows WASAPI paylaşımlı mod çıkışı (olay güdümlü).
//!
//! Ses motoru her periyotta (genellikle 10 ms) bir olay gönderir; biz de
//! arabellekteki boş yeri halka tampondan doldururuz. Arabellek 100 ms'dir:
//! işletim sistemi iş parçacığımızı kısa süre geciktirse bile ses kesilmez.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::Consumer;
use wasapi::{
    initialize_mta, DeviceEnumerator, Direction, SampleType, StreamMode, WasapiError, WaveFormat,
};

use super::OutputSpec;
use crate::audio::player::SharedState;
use crate::audio::render::Renderer;
use crate::audio::{AudioError, Sample};

/// Aygıt arabelleğinin süresi (100 ns birimi): 100 ms.
const BUFFER_DURATION_HNS: i64 = 1_000_000;
/// Bu süre içinde olay gelmezse aygıt kopmuş sayılır.
const EVENT_TIMEOUT_MS: u32 = 2_000;
/// Şarkı sonunda aygıttaki son örneklerin çalınmasını beklerken yoklama aralığı.
const DRAIN_POLL: Duration = Duration::from_millis(5);

type Ready = SyncSender<Result<(), AudioError>>;

/// WASAPI hatasını kullanıcıya gösterilecek Türkçe bir mesajla sarar.
fn fail(context: &'static str) -> impl FnOnce(WasapiError) -> AudioError {
    move |error| AudioError::Output(format!("{context}: {error}"))
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

fn run(
    spec: OutputSpec,
    mut source: Consumer<Sample>,
    shared: &SharedState,
    ready: &mut Option<Ready>,
) -> Result<(), AudioError> {
    initialize_mta()
        .ok()
        .map_err(|e| AudioError::Output(format!("COM başlatılamadı: {e}")))?;

    let enumerator = DeviceEnumerator::new().map_err(fail("Ses aygıtları listelenemedi"))?;
    let device = enumerator
        .get_default_device(&Direction::Render)
        .map_err(|_| AudioError::NoOutputDevice)?;
    let mut client = device
        .get_iaudioclient()
        .map_err(fail("Ses aygıtı açılamadı"))?;

    // Şarkının biçiminde 32-bit kayan nokta akış; gerekirse Windows dönüştürür.
    let format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        spec.sample_rate as usize,
        spec.channels,
        None,
    );
    let block_align = format.get_blockalign() as usize;
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
    let render_client = client
        .get_audiorenderclient()
        .map_err(fail("Ses yazıcısı alınamadı"))?;
    let buffer_frames = client
        .get_buffer_size()
        .map_err(fail("Arabellek boyutu alınamadı"))? as usize;

    // Bütün bellek burada, bir kez ayrılır; döngüde ayırma yapılmaz.
    let mut floats = vec![0.0f32; buffer_frames * spec.channels];
    let mut bytes = vec![0u8; buffer_frames * block_align];
    let mut renderer = Renderer::new(spec.channels, spec.sample_rate, Arc::clone(&shared.eq));
    let mut consumed: u64 = 0;
    let mut started = false;

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
        let draining = shared.decode_done.load(Ordering::Acquire) && source.is_empty();

        if !draining {
            let space = client
                .get_available_space_in_frames()
                .map_err(fail("Ses aygıtı okunamadı"))? as usize;
            let frames = space.min(buffer_frames);
            if frames > 0 {
                let out = &mut floats[..frames * spec.channels];
                let paused = shared.paused.load(Ordering::Acquire);
                let outcome = renderer.render(&mut source, out, paused);
                for (dst, sample) in bytes.chunks_exact_mut(4).zip(out.iter()) {
                    dst.copy_from_slice(&sample.to_le_bytes());
                }
                render_client
                    .write_to_device(frames, &bytes[..frames * block_align], None)
                    .map_err(fail("Ses aygıtına yazılamadı"))?;
                consumed += outcome.frames_consumed as u64;
                if outcome.frames_missing > 0 && !shared.decode_done.load(Ordering::Acquire) {
                    shared.underruns.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        // Arabellek ilk kez dolduktan sonra başlat: çalma sessizlikle başlamasın.
        if !started {
            client
                .start_stream()
                .map_err(fail("Ses akışı başlatılamadı"))?;
            started = true;
        }

        // Duyulan konum ≈ oturumun başladığı kare + halka tampondan alınan -
        // henüz aygıtta bekleyen. Duraklatmada aygıta sessizlik yazıldığı için bu
        // tahmin bir an geriye kayabilir; oturum içinde geri gidiş olmadığından
        // (sarma yeni oturum açar) konum hiç azaltılmaz.
        let padding = client
            .get_current_padding()
            .map_err(fail("Ses aygıtı okunamadı"))? as u64;
        shared.frames_played.fetch_max(
            shared.start_frame + consumed.saturating_sub(padding),
            Ordering::AcqRel,
        );

        if draining {
            if padding == 0 {
                shared.ended.store(true, Ordering::Release);
                break Ok(());
            }
            std::thread::sleep(DRAIN_POLL);
            continue;
        }

        if event.wait_for_event(EVENT_TIMEOUT_MS).is_err() {
            break Err(AudioError::Output(
                "aygıt yanıt vermiyor; bağlantısı kesilmiş olabilir".to_owned(),
            ));
        }
    };

    if started {
        let _ = client.stop_stream();
    }
    result
}
