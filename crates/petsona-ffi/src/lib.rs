//! Thin C ABI for native Petsona frontends.
//!
//! The ABI owns no domain logic. It validates/copies caller-owned values,
//! forwards commands to the serialized runtime worker and projects immutable
//! snapshots/text buffers back to the caller. Handles are thread-confined to
//! the creating UI thread; the runtime worker owns all disk/protocol state.

mod buffers;
mod commands;
mod error;
mod handles;
mod render;
mod types;

pub use types::{
    PetsonaCommand, PetsonaCommandKind, PetsonaEngineOptions, PetsonaSnapshot, PetsonaStatus,
    PetsonaStringView, PetsonaTextField, ABI_VERSION,
};

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::ptr;

pub use handles::Engine;
use petsona_runtime::engine::RuntimeEngine;
use petsona_runtime::snapshot::RuntimeTextField;

fn engine_mut<'a>(handle: *mut Engine) -> Result<&'a mut Engine, PetsonaStatus> {
    if handle.is_null() {
        error::set("engine handle is null");
        return Err(PetsonaStatus::InvalidHandle);
    }
    // The native host must serialize calls on the engine's creating thread.
    Ok(unsafe { &mut *handle })
}

fn status_with_panic<F>(handle: *mut Engine, f: F) -> PetsonaStatus
where
    F: FnOnce() -> PetsonaStatus,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => {
            error::set("Rust panic crossed the FFI boundary; engine is unusable");
            if let Ok(engine) = engine_mut(handle) {
                engine.mark_faulted(error::get());
            }
            PetsonaStatus::Panic
        }
    }
}

fn runtime_status(engine: &Engine) -> PetsonaStatus {
    if engine.terminal_error.is_some() {
        PetsonaStatus::Panic
    } else if engine.runtime.snapshot().faulted {
        PetsonaStatus::RuntimeFailed
    } else {
        PetsonaStatus::Ok
    }
}

/// Create an engine. The worker starts loading asynchronously; this call does
/// not read the data directory or bind the state server on the caller thread.
/// `options` may be null to use the normal Petsona data directory.
///
/// # Safety
///
/// `out_engine` must be a valid writable pointer. If `options` is non-null it
/// must point to a valid options value and its borrowed home bytes must remain
/// alive for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn petsona_engine_create(
    options: *const PetsonaEngineOptions,
    out_engine: *mut *mut Engine,
) -> PetsonaStatus {
    let result = catch_unwind(AssertUnwindSafe(|| {
        error::clear();
        if out_engine.is_null() {
            error::set("out_engine is null");
            return PetsonaStatus::InvalidArgument;
        }
        unsafe { *out_engine = ptr::null_mut() };

        let options = if options.is_null() {
            PetsonaEngineOptions::default()
        } else {
            unsafe { *options }
        };
        if options.abi_version != 0 && options.abi_version != ABI_VERSION {
            error::set(format!(
                "unsupported ABI version {}; expected {}",
                options.abi_version, ABI_VERSION
            ));
            return PetsonaStatus::InvalidArgument;
        }
        let home = match buffers::view_string(options.home) {
            Ok(value) if value.trim().is_empty() => None,
            Ok(value) => Some(PathBuf::from(value)),
            Err(message) => {
                error::set(message);
                return PetsonaStatus::InvalidArgument;
            }
        };
        let runtime = match RuntimeEngine::spawn(home, || {}) {
            Ok(runtime) => runtime,
            Err(error) => {
                error::set(format!("cannot spawn runtime engine: {error:#}"));
                return PetsonaStatus::InitializationFailed;
            }
        };
        let engine = Box::new(Engine {
            runtime,
            terminal_error: None,
        });
        unsafe { *out_engine = Box::into_raw(engine) };
        PetsonaStatus::Ok
    }));
    match result {
        Ok(status) => status,
        Err(_) => {
            error::set("Rust panic while creating the engine");
            PetsonaStatus::Panic
        }
    }
}

/// Stop and destroy an engine. The handle must not be used afterwards.
///
/// # Safety
///
/// `handle` must be null or a live handle returned by
/// [`petsona_engine_create`], and no other operation may be using it.
#[no_mangle]
pub unsafe extern "C" fn petsona_engine_destroy(handle: *mut Engine) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !handle.is_null() {
            unsafe { drop(Box::from_raw(handle)) };
        }
    }));
}

/// Wake the runtime worker and advance protocol/TTL/animation state as soon
/// as possible. The worker also wakes at its own next-frame deadline.
#[no_mangle]
pub extern "C" fn petsona_engine_tick(handle: *mut Engine) -> PetsonaStatus {
    status_with_panic(handle, || {
        let engine = match engine_mut(handle) {
            Ok(engine) => engine,
            Err(status) => return status,
        };
        let status = runtime_status(engine);
        if status != PetsonaStatus::Ok {
            return status;
        }
        match engine
            .runtime
            .send(petsona_runtime::commands::RuntimeCommand::Tick)
        {
            Ok(()) => PetsonaStatus::Ok,
            Err(error) => {
                error::set(error.to_string());
                PetsonaStatus::RuntimeFailed
            }
        }
    })
}

/// Copy the current immutable runtime projection into `destination`.
///
/// # Safety
///
/// `handle` must be a live engine handle owned by the calling thread and
/// `destination` must point to writable storage for one snapshot.
#[no_mangle]
pub unsafe extern "C" fn petsona_engine_snapshot(
    handle: *mut Engine,
    destination: *mut PetsonaSnapshot,
) -> PetsonaStatus {
    status_with_panic(handle, || {
        if destination.is_null() {
            error::set("snapshot destination is null");
            return PetsonaStatus::InvalidArgument;
        }
        let engine = match engine_mut(handle) {
            Ok(engine) => engine,
            Err(status) => return status,
        };
        unsafe { *destination = render::snapshot_of(engine) };
        runtime_status(engine)
    })
}

/// Return the required UTF-8 byte count and optionally copy one text field.
#[no_mangle]
pub extern "C" fn petsona_engine_copy_text(
    handle: *mut Engine,
    field: u32,
    destination: *mut u8,
    capacity: usize,
) -> usize {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let Ok(engine) = engine_mut(handle) else {
            return 0;
        };
        let Some((_, runtime_field)) = render::text_field(field) else {
            error::set(format!("unknown text field: {field}"));
            return 0;
        };
        let text = if runtime_field == RuntimeTextField::Error {
            engine
                .terminal_error
                .clone()
                .unwrap_or_else(|| engine.runtime.text(RuntimeTextField::Error))
        } else {
            engine.runtime.text(runtime_field)
        };
        error::copy_bytes(text.as_bytes(), destination, capacity)
    }));
    match result {
        Ok(value) => value,
        Err(_) => {
            error::set("Rust panic while copying an FFI text field");
            0
        }
    }
}

/// Enqueue one validated command. Text is copied before this function
/// returns; the runtime worker never retains a foreign pointer.
///
/// # Safety
///
/// `handle` must be a live engine handle owned by the calling thread and
/// `command` must point to a valid command whose borrowed bytes remain alive
/// for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn petsona_engine_command(
    handle: *mut Engine,
    command: *const PetsonaCommand,
) -> PetsonaStatus {
    status_with_panic(handle, || {
        if command.is_null() {
            error::set("command is null");
            return PetsonaStatus::InvalidArgument;
        }
        let engine = match engine_mut(handle) {
            Ok(engine) => engine,
            Err(status) => return status,
        };
        let status = runtime_status(engine);
        if status != PetsonaStatus::Ok {
            return status;
        }
        let command = unsafe { &*command };
        let command = match commands::convert(command) {
            Ok(command) => command,
            Err((status, message)) => {
                error::set(message);
                return status;
            }
        };
        match engine.runtime.send(command) {
            Ok(()) => PetsonaStatus::Ok,
            Err(error) => {
                error::set(error.to_string());
                PetsonaStatus::RuntimeFailed
            }
        }
    })
}

/// Return the required byte count and optionally copy the calling thread's
/// last ABI error.
#[no_mangle]
pub extern "C" fn petsona_last_error_copy(destination: *mut u8, capacity: usize) -> usize {
    error::copy(destination, capacity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::mem::{align_of, size_of};
    use std::time::Duration;

    fn temp_home() -> tempfile::TempDir {
        let home = tempfile::tempdir().expect("temporary home");
        let paths = petsona_core::config::AppPaths::resolve(home.path().to_path_buf());
        paths.ensure().expect("home directories");
        let mut config = petsona_core::config::AppConfig::default();
        config.state_server.enabled = false;
        config.save(&paths.config_file).expect("isolated config");
        home
    }

    fn create(home: &tempfile::TempDir) -> *mut Engine {
        let path = home.path().to_string_lossy().into_owned();
        let mut handle = ptr::null_mut();
        let options = PetsonaEngineOptions {
            abi_version: ABI_VERSION,
            home: PetsonaStringView {
                ptr: path.as_ptr(),
                len: path.len(),
            },
        };
        assert_eq!(
            unsafe { petsona_engine_create(&options, &mut handle) },
            PetsonaStatus::Ok
        );
        for _ in 0..100 {
            let mut snapshot = PetsonaSnapshot::default();
            let _ = unsafe { petsona_engine_snapshot(handle, &mut snapshot) };
            if snapshot.ready != 0 {
                return handle;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        handle
    }

    #[test]
    fn abi_structs_have_explicit_layout() {
        assert_eq!(ABI_VERSION, 3);
        assert_eq!(align_of::<PetsonaStringView>(), 8);
        assert_eq!(size_of::<PetsonaStringView>(), 16);
        assert_eq!(size_of::<PetsonaEngineOptions>(), 24);
        assert_eq!(size_of::<PetsonaCommand>(), 40);
        assert_eq!(align_of::<PetsonaSnapshot>(), 8);
        assert_eq!(size_of::<PetsonaSnapshot>(), 64);
    }

    #[test]
    fn empty_home_creates_a_ready_engine_without_a_pet() {
        let home = temp_home();
        let handle = create(&home);
        let mut snapshot = PetsonaSnapshot::default();
        assert_eq!(
            unsafe { petsona_engine_snapshot(handle, &mut snapshot) },
            PetsonaStatus::Ok
        );
        assert_ne!(snapshot.ready, 0);
        assert_eq!(snapshot.faulted, 0);
        assert_eq!(snapshot.has_pet, 0);
        unsafe { petsona_engine_destroy(handle) };
        assert!(home.path().join("config.json").is_file());
        let _ = fs::remove_file(home.path().join("petsona.lock"));
    }

    #[test]
    fn invalid_command_is_rejected_without_touching_the_worker() {
        let home = temp_home();
        let handle = create(&home);
        let command = PetsonaCommand {
            kind: 999,
            reserved: 0,
            value: 0.0,
            ttl_ms: 0,
            text: PetsonaStringView::default(),
        };
        assert_eq!(
            unsafe { petsona_engine_command(handle, &command) },
            PetsonaStatus::InvalidArgument
        );
        let required =
            petsona_engine_copy_text(handle, PetsonaTextField::Error as u32, ptr::null_mut(), 0);
        assert!(required > 0);
        unsafe { petsona_engine_destroy(handle) };
    }
}
