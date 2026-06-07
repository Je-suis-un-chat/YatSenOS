use alloc::{boxed::Box, vec::Vec};
// 1. 我们不再需要导入 arrayvec
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
    used: usize,
    frames: BootInfoFrameIter,
    recycled: Vec<PhysFrame>,
}

impl BootInfoFrameAllocator {
    // 2. 魔法在这里：将参数改为 &'static [MemoryDescriptor] (静态切片)
    // 当外面传入 &boot_info.memory_map (ArrayVec引用) 时，Rust 会自动把它转成切片！
    pub unsafe fn init(memory_map: &'static [MemoryDescriptor], size: usize) -> Self {
        Self { size, used: 0, frames: create_frame_iter(memory_map), recycled: Vec::new(), }
    }

    pub fn frames_used(&self) -> usize {
        self.used
    }

    pub fn frames_total(&self) -> usize {
        self.size
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.recycled.pop().or_else(|| self.frames.next())?;
        self.used += 1;
        Some(frame)
    }
}

impl FrameDeallocator<Size4KiB> for BootInfoFrameAllocator {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame) {
        // TODO: deallocate frame
        self.used = self.used.checked_sub(1).expect("Physical frame double free");

        self.recycled.push(frame);
    }
}

// 3. 迭代器生成函数同样接收切片
fn create_frame_iter(memory_map: &'static [MemoryDescriptor]) -> BootInfoFrameIter {
    let iter = memory_map
        .iter() // 切片自带 .iter() 方法
        .filter(|r| r.ty == MemoryType::CONVENTIONAL)
        // 提醒：如果后续 page_count 报错，说明官方字段名是 number_of_pages
        .flat_map(|r| (0..r.page_count).map(move |v| (v * 4096 + r.phys_start)))
        .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)));

    Box::new(iter)
}