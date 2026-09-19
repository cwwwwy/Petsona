use std::cell::RefCell;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

pub fn set(message: impl Into<String>) {
    LAST_ERROR.with(|slot| *slot.borrow_mut() = message.into());
}

pub fn clear() {
    set("");
}

pub fn get() -> String {
    LAST_ERROR.with(|slot| slot.borrow().clone())
}

pub fn copy(destination: *mut u8, capacity: usize) -> usize {
    let bytes = LAST_ERROR.with(|slot| slot.borrow().as_bytes().to_vec());
    copy_bytes(&bytes, destination, capacity)
}

pub fn copy_bytes(bytes: &[u8], destination: *mut u8, capacity: usize) -> usize {
    if !destination.is_null() && capacity != 0 {
        let amount = bytes.len().min(capacity);
        // The caller owns the destination buffer and promises it is writable.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), destination, amount) };
    }
    bytes.len()
}
