//! Host test adapter for the same kernel wait-state contract used on OHOS.
use std::io;
pub(crate) struct WorkerState(libc::mach_port_t);
impl WorkerState {
    pub(crate) fn current() -> io::Result<Self> {
        Ok(Self(unsafe {
            libc::pthread_mach_thread_np(libc::pthread_self())
        }))
    }
    pub(crate) fn is_blocked(&self) -> io::Result<bool> {
        let mut info = std::mem::MaybeUninit::<libc::thread_basic_info>::uninit();
        let mut count = libc::THREAD_BASIC_INFO_COUNT;
        let result = unsafe {
            libc::thread_info(
                self.0,
                libc::THREAD_BASIC_INFO as u32,
                info.as_mut_ptr().cast(),
                &mut count,
            )
        };
        if result != 0 {
            return Err(io::Error::other(format!("thread_info: {result}")));
        }
        let info = unsafe { info.assume_init() };
        Ok(matches!(
            info.run_state,
            libc::TH_STATE_WAITING | libc::TH_STATE_UNINTERRUPTIBLE
        ))
    }
}
