#![no_std]

use core::{alloc::Layout};

use allocator::{AllocResult, BaseAllocator, ByteAllocator, PageAllocator, AllocError};

use core::ptr::NonNull;
/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
/// pub struct EarlyAllocator;
pub struct EarlyAllocator<const PAGE_SIZE: usize> {
    // 内存区域 [start, end)
    start: usize,
    end: usize,
    // 字节分配指针（向前增长）
    b_pos: usize,
    // 页分配指针（向后增长）
    p_pos: usize,
    // 字节分配计数
    count: usize,
    // 页分配计数器
    page_count: usize,
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    /// new!!!
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            count: 0,
            p_pos: 0,
            page_count: 0,
        }
    }
}

impl<const PAGE_SIZE: usize> EarlyAllocator<PAGE_SIZE> {
    /// 地址向上对齐
    fn align_up(addr: usize, align: usize) -> usize {
        (addr + align - 1) & !(align - 1)
    }

    /// 地址向下对齐
    fn align_down(addr: usize, align: usize) -> usize {
        addr & !(align - 1)
    }
}


impl<const PAGE_SIZE: usize> BaseAllocator for EarlyAllocator<PAGE_SIZE> {
    /// Initialize the allocator with a free memory region.
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = start + size;
        self.count = 0;
        self.page_count = 0;
    }

    /// Add a free memory region to the allocator.
    fn add_memory(&mut self, _start: usize, _size: usize) -> allocator::AllocResult {
        Err(AllocError::NoMemory) // 早期分配器不支持动态添加内存
    }
}

impl<const PAGE_SIZE: usize> ByteAllocator for EarlyAllocator<PAGE_SIZE> {
    /// Allocate memory with the given size (in bytes) and alignment.
    fn alloc (&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let size = layout.size();
        let align = layout.align();
        
        // 计算对齐后的起始地址
        let aligned_addr = Self::align_up(self.b_pos, align);
        
        // 计算新的指针位置（带溢出检查）
        let new_b_pos = aligned_addr.checked_add(size)
            .ok_or(AllocError::NoMemory)?;

        // 检查内存是否足够
        if new_b_pos > self.p_pos {
            return Err(AllocError::NoMemory);
        }

        // 更新状态
        self.b_pos = new_b_pos;
        self.count += 1;

        // 转换为 NonNull 指针
        NonNull::new(aligned_addr as *mut u8)
            .ok_or(AllocError::NoMemory)

    }

    /// Deallocate memory at the given position, size, and alignment.
    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout){
        if self.count > 0 {
            self.count -= 1;
            // 当所有分配都释放时重置
            if self.count == 0 {
                self.b_pos = self.start;
            }
        }
    }

    /// Returns total memory size in bytes.
    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    /// Returns allocated memory size in bytes.
    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }

    /// Return available memory size in bytes.
    fn available_bytes(&self) -> usize {
        self.p_pos - self.b_pos
    }
}

impl<const PAGE_SIZE: usize> PageAllocator for EarlyAllocator<PAGE_SIZE> {
    const PAGE_SIZE: usize = PAGE_SIZE;
    /// Allocates contiguous pages.
    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        // 计算总字节需求
        let total_bytes = num_pages.checked_mul(PAGE_SIZE)
            .ok_or(AllocError::InvalidParam)?;

        // 计算最大可能起始地址
        let max_start = self.p_pos.checked_sub(total_bytes)
            .ok_or(AllocError::NoMemory)?;

        // 向下对齐地址
        let aligned_start = Self::align_down(max_start, align_pow2);

        // 检查是否与字节分配区重叠
        if aligned_start < self.b_pos {
            return Err(AllocError::NoMemory);
        }

        // 更新页分配指针
        self.p_pos = aligned_start;
        self.page_count += 1;
        Ok(aligned_start)
      
    }

    /// Gives back the allocated pages starts from `pos` to the page allocator.
    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        if self.page_count > 0{
            self.page_count -= 1;
            if self.page_count == 0 {
                self.p_pos = self.end;
            }
        }
    }

    /// Returns the number of allocated bytes in the byte allocator.
    fn total_pages(&self) -> usize {
        (self.end - self.start) / PAGE_SIZE
    }

    /// Returns the number of allocated pages in the page allocator.
    fn used_pages(&self) -> usize {
        (self.end - self.p_pos) / PAGE_SIZE
    }

    /// Returns the number of available pages in the page allocator.
    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) / PAGE_SIZE
    }
}
