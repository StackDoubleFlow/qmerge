use libc::{mprotect, sysconf, PROT_EXEC, PROT_READ, PROT_WRITE, _SC_PAGE_SIZE};
use std::mem::take;
use std::sync::{LazyLock, Mutex};

pub static HOOK_ALLOCATOR: LazyLock<Mutex<HookAllocator>> = LazyLock::new(Default::default);
/// The page size in instructions (bytes / 4)
static PAGE_SIZE: LazyLock<usize> =
    LazyLock::new(|| unsafe { sysconf(_SC_PAGE_SIZE) as usize / 4 });

struct Page {
    data: Box<[u32]>,
    used: usize,
}

impl Default for Page {
    fn default() -> Self {
        let data = vec![0; *PAGE_SIZE].into_boxed_slice();
        unsafe {
            mprotect(
                data.as_ptr() as _,
                *PAGE_SIZE,
                PROT_READ | PROT_WRITE | PROT_EXEC,
            );
        }

        Self { data, used: 0 }
    }
}

#[derive(Default)]
pub struct HookAllocator {
    old_pages: Vec<Page>,
    current_page: Page,
}

impl HookAllocator {
    pub fn alloc(&mut self, size: usize) -> *mut u32 {
        if self.current_page.used + size > *PAGE_SIZE {
            let old_page = take(&mut self.current_page);
            self.old_pages.push(old_page);
        }

        let page = &mut self.current_page;
        let ptr = &mut page.data[page.used] as *mut u32;
        page.used += size;
        ptr
    }
}
