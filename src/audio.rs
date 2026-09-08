//! Uses Omarchy's existing PulseAudio-compatible service (normally PipeWire).
//! No audio daemon, shell command, or device driver is shipped or installed.
use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};

#[repr(C)]
struct SampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}
#[repr(C)]
struct BufferAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}

#[link(name = "pulse-simple")]
unsafe extern "C" {
    fn pa_simple_new(
        server: *const c_char,
        name: *const c_char,
        dir: c_int,
        dev: *const c_char,
        stream: *const c_char,
        spec: *const SampleSpec,
        map: *const c_void,
        attr: *const BufferAttr,
        error: *mut c_int,
    ) -> *mut c_void;
    fn pa_simple_write(
        s: *mut c_void,
        data: *const c_void,
        bytes: usize,
        error: *mut c_int,
    ) -> c_int;
    fn pa_simple_free(s: *mut c_void);
}
#[link(name = "pulse")]
unsafe extern "C" {
    fn pa_strerror(error: c_int) -> *const c_char;
}

struct Stream(*mut c_void);
impl Drop for Stream {
    fn drop(&mut self) {
        unsafe { pa_simple_free(self.0) }
    }
}
fn error(code: c_int) -> String {
    // PulseAudio owns a static NUL-terminated string for every error code.
    unsafe {
        CStr::from_ptr(pa_strerror(code))
            .to_string_lossy()
            .into_owned()
    }
}

pub struct Audio {
    sender: SyncSender<Vec<f32>>,
    pub failed: Arc<AtomicBool>,
}
impl Audio {
    pub fn new() -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel::<Vec<f32>>(8);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let failed = Arc::new(AtomicBool::new(false));
        let failure = failed.clone();
        std::thread::spawn(move || {
            let spec = SampleSpec {
                format: 5,
                rate: 48_000,
                channels: 2,
            }; // FLOAT32LE
            // Byte counts, so stereo needs twice as many for the same latency.
            let attr = BufferAttr {
                maxlength: 96_000,
                tlength: 38_400,
                prebuf: u32::MAX,
                minreq: u32::MAX,
                fragsize: u32::MAX,
            };
            let mut code = 0;
            // All pointers stay valid for the duration of the call; ownership
            // of the returned connection remains entirely on this thread.
            let ptr = unsafe {
                pa_simple_new(
                    std::ptr::null(),
                    c"OmaFM".as_ptr(),
                    1,
                    std::ptr::null(),
                    c"FM radio".as_ptr(),
                    &spec,
                    std::ptr::null(),
                    &attr,
                    &mut code,
                )
            };
            if ptr.is_null() {
                let _ = ready_tx.send(Err(error(code)));
                return;
            }
            let stream = Stream(ptr);
            let _ = ready_tx.send(Ok(()));
            while let Ok(samples) = receiver.recv() {
                let result = unsafe {
                    pa_simple_write(
                        stream.0,
                        samples.as_ptr().cast(),
                        samples.len() * 4,
                        &mut code,
                    )
                };
                if result < 0 {
                    failure.store(true, Ordering::Relaxed);
                    crate::event("error", &format!("Audio output failed: {}", error(code)));
                    break;
                }
            }
        });
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| "Audio service did not respond".to_string())??;
        Ok(Self { sender, failed })
    }
    pub fn push(&self, samples: &[f32]) -> bool {
        // Bound latency and memory even if the output device stalls.
        self.sender.try_send(samples.to_vec()).is_ok()
    }
}
