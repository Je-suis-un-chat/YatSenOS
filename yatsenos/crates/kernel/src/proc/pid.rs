use core::sync::atomic::{AtomicU16, Ordering};


#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub u16);

pub  static NEXT_PID:AtomicU16 = AtomicU16::new(1);

impl ProcessId {
    pub fn new() -> Self {
        Self(NEXT_PID.fetch_add(1,Ordering::SeqCst))
    }
}

impl Default for ProcessId {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Debug for ProcessId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ProcessId> for u16 {
    fn from(pid: ProcessId) -> Self {
        pid.0
    }
}
