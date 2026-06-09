use alloc::sync::Arc;
use hashbrown::HashMap;
use spin::{rwlock::RwLock};

use super::*;
use crate::resource::ResourceSet;
use sync::*;


///type ResourceSet = ();

#[derive(Debug, Clone)]
pub struct ProcessData {
    // shared data
    pub(super) env: Arc<RwLock<HashMap<String, String, ahash::RandomState>>>,
    pub(super) resources: Arc<RwLock<ResourceSet>>,
    pub(super) semaphores: Arc<RwLock<SemaphoreSet>>,
}

impl Default for ProcessData {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessData {
    pub fn new() -> Self {
        Self {
            env: Arc::new(RwLock::new(HashMap::default())),
            resources: Arc::new(RwLock::new(ResourceSet::default())),
            semaphores: Arc::new(RwLock::new(SemaphoreSet::default())),
        }
    }

    pub fn env(&self, key: &str) -> Option<String> {
        self.env.read().get(key).cloned()
    }

    pub fn set_env(&mut self, key: &str, val: &str) {
        self.env.write().insert(key.into(), val.into());
    }

    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
    self.resources.read().read(fd, buf)
    }

    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        self.resources.read().write(fd, buf)
    }

    pub fn new_sem(&self, key:u32, value:usize) -> bool{
        self.semaphores.write().insert(key, value)
    }

    pub fn remove_sem(&self, key:u32) -> bool{
        self.semaphores.write().remove(key)
    }

    pub fn sem_wait(&self, key:u32, pid:ProcessId) -> SemaphoreResult{
        self.semaphores.write().wait(key, pid)
    }

    pub fn sem_signal(&self, key:u32) -> SemaphoreResult{
        self.semaphores.write().signal(key)
    }
}
