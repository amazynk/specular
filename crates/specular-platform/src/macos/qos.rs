//! Thread QoS classes. On Apple Silicon the QoS class is what decides between performance and
//! efficiency cores (there is no affinity API): `USER_INTERACTIVE` / `USER_INITIATED` prefer
//! P-cores, `UTILITY` may use either, `BACKGROUND` stays on E-cores and has its disk I/O
//! throttled. On Intel Macs the class still sets priority and timer coalescing.

use std::cell::Cell;

use crate::ThreadClass;

thread_local! {
    /// The class this thread was last set to, so per-job calls cost nothing when unchanged.
    static CURRENT: Cell<Option<ThreadClass>> = const { Cell::new(None) };
}

pub(crate) fn set_current(class: ThreadClass) {
    if CURRENT.get() == Some(class) {
        return;
    }
    let qos = match class {
        ThreadClass::Interactive => libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE,
        ThreadClass::UserInitiated => libc::qos_class_t::QOS_CLASS_USER_INITIATED,
        ThreadClass::Utility => libc::qos_class_t::QOS_CLASS_UTILITY,
        ThreadClass::Background => libc::qos_class_t::QOS_CLASS_BACKGROUND,
    };
    if set_self(qos) == 0 {
        CURRENT.set(Some(class));
    } else {
        specular_log::debug!(target: specular_log::JOBS, "pthread_set_qos_class_self_np({class:?}) failed");
    }
}

/// `pthread_set_qos_class_self_np`; returns its error code (0 on success).
#[allow(unsafe_code)]
fn set_self(qos: libc::qos_class_t) -> libc::c_int {
    // SAFETY: plain FFI call with two by-value integers; it only changes the calling thread's
    // scheduling attributes and reports invalid arguments as an error code. It fails (EPERM)
    // only for a thread in a workqueue with an explicit QoS override, which ours are not.
    unsafe { libc::pthread_set_qos_class_self_np(qos, 0) }
}
