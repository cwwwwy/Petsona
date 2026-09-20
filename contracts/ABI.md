# Petsona native ABI

> ABI version 3 implementation contract. The layout/lifecycle checks below
> are now exercised by Rust and Swift tests, but this is not yet a release
> compatibility promise. The intended
> lifecycle/threading contract is defined in [the task plan](../docs/plans/native-ui-rewrite.md).
> Open findings and the remaining manual checks are tracked in [the execution
> record](../docs/execution/native-ui-rewrite.md). The ABI now forwards
> configuration, memory, persona-management and import-confirmation commands
> to the serialized runtime worker; credentials are not included in snapshots.

`contracts/petsona.h` is the hand-reviewed public C header for the native
frontends. The Rust implementation lives in `crates/petsona-ffi`; the native
macOS target links `libpetsona_ffi.a` statically.

## Ownership and threading

- `petsona_engine_create` allocates an opaque engine and writes its handle to
  the caller-owned pointer. The caller must call `petsona_engine_destroy` once.
- The engine is thread-confined to the thread that created it. The native UI
  main thread owns the handle; background work is internal to Rust.
- `PetsonaStringView` borrows caller memory for the duration of one call. Rust
  never stores the pointer.
- `petsona_engine_copy_text` copies UTF-8 bytes into caller storage. A call with
  a null destination and zero capacity returns the required byte count.
- No Rust allocation crosses the boundary, so frontends never free a Rust
  string with their own allocator.

## Lifecycle

1. Create one engine for one data directory. Creation starts a worker and does
   not load the directory on the caller's UI thread.
2. Call `petsona_engine_tick` when the returned `next_frame_ms` deadline expires;
   the worker also wakes for protocol events and its own deadline.
3. Read a `PetsonaSnapshot` and text fields. `ready=0` means loading,
   `faulted=1` means the handle is terminal and must be destroyed.
4. Send validated commands with `petsona_engine_command`; all borrowed strings
   are copied before the call returns.
5. Destroy the engine after the UI has stopped scheduling ticks; destruction
   joins the worker and releases the instance lock/state server.

After a panic caught by the FFI layer, the operation returns `PETSONA_PANIC`;
the caller must treat the engine as unusable and destroy it. Invalid pointers,
allocator failure and other undefined behavior remain process-fatal.

## Versioning

`PETSONA_ABI_VERSION` is currently `3` and is incremented for incompatible layout or ownership
changes. New fields are appended only when both sides can tolerate the old
size. The native frontends reject an unsupported version before creating an
engine.

## Current extension values

ABI 3 keeps the existing struct layout. Text fields `12–16` expose the current
persona, persona list, non-secret DeepSeek configuration, current-persona
memory projection and a pending pet-import conflict. Command kinds `23–35`
cover DeepSeek/memory updates, fact operations, persona CRUD/import/export and
clearing an import conflict. These values are serialized as UTF-8 JSON or
paths; the worker validates and persists them before publishing the next
projection.
