//! Windows WASAPI paylaşımlı mod çıkışı (olay güdümlü).
//!
//! Ses motoru her periyotta (genellikle 10 ms) bir olay gönderir; biz de
//! arabellekteki boş yeri halka tampondan doldururuz. Arabellek 100 ms'dir:
//! işletim sistemi iş parçacığımızı kısa süre geciktirse bile ses kesilmez.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;

use rtrb::Consumer;
use wasapi::{initialize_mta, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

use super::OutputSpec;
use crate::audio::player::SharedState;
use crate::audio::render::Renderer;
use crate::audio::{AudioError, Sample};

/// Aygıt arabelleğinin süresi (100 ns birimi): 100 ms.
const BUFFER_DURATION_HNS: i64 = 1_000_000;
/// Bu süre içinde olay gelmezse aygıt kopmuş sayılır.
const EVENT_TIMEOUT_MS: u32 = 2_000;
/// Şarkı sonunda aygıttaki son örneklerin çalınmasını beklerken yoklama aralığı.
const DRAIN_POLL: std::time::Duration = std::time::Duration::from_millis(5);

type Ready = SyncSender<Result<(), String>>;

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
            if let Err(message) = run(spec, source, &shared, &mut ready) {
                match ready.take() {
                    // Açılışta hata: oynatıcıya hemen bildir.
                    Some(tx) => {
                        let _ = tx.send(Err(message));
                    }
                    // Çalarken hata: arayüz durum sorgusunda görür.
                    None => shared.fail(message),
                }
            }
        })
        .map_err(|e| AudioError::Output(e.to_string()))?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(message)) => {
            let _ = handle.join();
            Err(AudioError::Output(message))
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
) -> Result<(), String> {
    initialize_mta()
        .ok()
        .map_err(|e| format!("COM başlatılamadı: {e}"))?;

    let enumerator =
        DeviceEnumerator::new().map_err(|e| format!("Ses aygıtları listelenemedi: {e}"))?;
    let device = enumerator
        .get_default_device(&Direction::Render)
        .map_err(|e| format!("Varsayılan ses çıkış aygıtı bulunamadı: {e}"))?;
    let mut client = device
        .get_iaudioclient()
        .map_err(|e| format!("Ses aygıtı açılamadı: {e}"))?;

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
        .map_err(|e| format!("Ses akışı başlatılamadı: {e}"))?;
    let event = client
        .set_get_eventhandle()
        .map_err(|e| format!("Ses olayı oluşturulamadı: {e}"))?;
    let render_client = client
        .get_audiorenderclient()
        .map_err(|e| format!("Ses yazıcısı alınamadı: {e}"))?;
    let buffer_frames = client
        .get_buffer_size()
        .map_err(|e| format!("Arabellek boyutu alınamadı: {e}"))? as usize;

    // Bütün bellek burada, bir kez ayrılır; döngüde ayırma yapılmaz.
    let mut floats = vec![0.0f32; buffer_frames * spec.channels];
    let mut bytes = vec![0u8; buffer_frames * block_align];
    let mut renderer = Renderer::new(spec.channels, spec.sample_rate);
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
                .map_err(|e| format!("Ses aygıtı okunamadı: {e}"))?
                as usize;
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
                    .map_err(|e| format!("Ses aygıtına yazılamadı: {e}"))?;
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
                .map_err(|e| format!("Ses akışı başlatılamadı: {e}"))?;
            started = true;
        }

        // Duyulan konum = halka tampondan alınan - henüz aygıtta bekleyen.
        let padding = client
            .get_current_padding()
            .map_err(|e| format!("Ses aygıtı okunamadı: {e}"))? as u64;
        shared
            .frames_played
            .store(consumed.saturating_sub(padding), Ordering::Release);

        if draining {
            if padding == 0 {
                shared.ended.store(true, Ordering::Release);
                break Ok(());
            }
            std::thread::sleep(DRAIN_POLL);
            continue;
        }

        if event.wait_for_event(EVENT_TIMEOUT_MS).is_err() {
            break Err("Ses aygıtı yanıt vermiyor; bağlantısı kesilmiş olabilir.".to_owned());
        }
    };

    if started {
        let _ = client.stop_stream();
    }
    result
}
