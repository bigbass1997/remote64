
use std::cell::UnsafeCell;

/// Infinite access unsafe cell. Multiple mutable references of this data can exist
/// across threads. No locking or any kind of safety checks are performed.
/// 
/// Extreme care must be taken when using this across threads. However, undefined
/// behavor may be desirable to mimic similar behavior on hardware.
#[derive(Debug)]
pub struct InfCell<T> {
    uc: UnsafeCell<T>,
}
impl<T> InfCell<T> {
    pub fn new(val: T) -> Self {
        Self { uc: UnsafeCell::new(val) }
    }
    pub fn with_uc(cell: UnsafeCell<T>) -> Self {
        Self { uc: cell }
    }
    
    /// Get mutable pointer to wrapped data.
    pub fn get_raw(&self) -> *mut T {
        self.uc.get()
    }
    
    /// Get immutable reference to wrapped data.
    pub fn get(&self) -> &T {
        unsafe { &*self.get_raw() }
    }
    
    /// Get mutable reference to wrapped data. No safety checks, no locking.
    /// 
    /// This can be used multiple times! Undefined behavior when multiple threads
    /// write to the same data type/struct contained in the wrapped data.
    pub fn get_mut(&self) -> &'static mut T {
        unsafe { &mut *self.get_raw() }
    }
}

unsafe impl<T> Send for InfCell<T> {}
unsafe impl<T> Sync for InfCell<T> {}