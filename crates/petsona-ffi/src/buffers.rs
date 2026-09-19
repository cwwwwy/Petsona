use std::str;

use crate::types::PetsonaStringView;

pub fn view_string(view: PetsonaStringView) -> Result<String, &'static str> {
    if view.len == 0 {
        return Ok(String::new());
    }
    if view.ptr.is_null() {
        return Err("string pointer is null");
    }
    // The view is borrowed only for this synchronous ABI call. Convert it to
    // owned UTF-8 before sending a command to the runtime worker.
    let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| "string is not valid UTF-8")
}
