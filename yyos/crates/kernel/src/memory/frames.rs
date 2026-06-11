use alloc::{boxed::Box, vec::Vec};
use boot::{MemoryDescriptor, MemoryType};
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB},
};

once_mutex!(pub FRAME_ALLOCATOR: BootInfoFrameAllocator);

guard_access_fn! {
    pub get_frame_alloc(FRAME_ALLOCATOR: BootInfoFrameAllocator)
}

type BootInfoFrameIter = Box<dyn Iterator<Item = PhysFrame> + Send>;

pub struct BootInfoFrameAllocator {
    size: usize,
    frames: BootInfoFrameIter,
    used: usize,
    recycled: Vec<PhysFrame>,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static [MemoryDescriptor], size: usize) -> Self {
        Self {
            size,
            frames: create_frame_iter(memory_map),
            used: 0,
            recycled: Vec::new(),
        }
    }

    pub fn frames_used(&self) -> usize {
        self.used
    }

    pub fn frames_total(&self) -> usize {
        self.size
    }

    pub fn frames_recycled(&self) -> usize {
        self.recycled.len()
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        if let Some(frame) = self.recycled.pop() {
            return Some(frame);
        }

        let frame = self.frames.next()?;
        self.used += 1;
        Some(frame)
    }
}

impl FrameDeallocator<Size4KiB> for BootInfoFrameAllocator {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame) {
        debug_assert!(
            !self.recycled.contains(&frame),
            "Physical frame double free: {:?}",
            frame
        );
        self.recycled.push(frame);
    }
}

fn create_frame_iter(memory_map: &'static [MemoryDescriptor]) -> BootInfoFrameIter {
    let iter = memory_map
        .iter()
        .filter(|r| r.ty == MemoryType::CONVENTIONAL)
        .flat_map(|r| (0..r.page_count).map(move |v| v * 4096 + r.phys_start))
        .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)));

    Box::new(iter)
}
