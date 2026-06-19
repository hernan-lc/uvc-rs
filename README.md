# UVC Rust Engine

Rust-first Android UVC camera engine workspace. The current implementation is a compile-safe foundation for multi-camera concurrency validation, not a complete Android UVC driver.

## Current status

Completed against `plan.md` and `.kilo/plans/rust-uvc-engine.md`:

- Cargo workspace with `uvc-core`, `uvc-driver`, `uvc-jni`, and `uvc-cli`.
- Pure-Rust core types for camera identity, formats, frames, bounded frame channels, errors, and pipeline traits.
- Deterministic fake multi-camera backend that streams multiple synthetic cameras concurrently.
- CLI validation command for fake multi-camera runs.
- UVC descriptor parsing models with synthetic descriptor tests.
- Optional `rusb` feature plus backend, device, endpoint, interface, and transfer abstractions.
- Placeholder Android file-descriptor identity wrapper in `uvc-jni`.
- Workspace formatting, checks, and tests are passing.

Not complete yet:

- No real UVC transfer loop.
- No Android NDK integration or Android file-descriptor-to-libusb path.
- No JNI exports for Kotlin.
- No Android surface, `ANativeWindow`, or `HardwareBuffer` path.
- No performance benchmark suite.
- No `LICENSE` file yet, despite the workspace package license metadata.

## Workspace layout

```text
crates/
  uvc-core/
    Pure Rust data model, error types, frame channel, and pipeline trait.
  uvc-driver/
    UVC descriptor parser, backend traits, rusb feature skeleton, fake deterministic camera backend, and concurrency validation harness.
  uvc-jni/
    Placeholder Android USB file-descriptor identity wrapper.
  uvc-cli/
    Local CLI tool for fake multi-camera validation.
```

## Commands

```bash
cargo fmt --all
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p uvc-cli -- fake-multi --cameras 4 --seconds 1 --fps 30 --width 16 --height 16 --format yuyv
```

## What next

Recommended order:

1. Validate the optional `rusb` feature on a desktop Linux environment with UVC hardware available.
2. Implement device discovery, UVC interface parsing, and endpoint selection behind the `rusb` feature.
3. Add Android target checks once the NDK and libusb build environment are configured.
4. Move Android file-descriptor handling from a placeholder into a real `libusb_wrap_sys_device` boundary behind an Android feature.
5. Add `jni` exports only after the Rust core and driver APIs are stable.
6. Add benchmarks for fake multi-camera throughput, frame buffer reuse, and bounded-channel latency.

## Current milestone coverage

| Milestone | Status |
| --- | --- |
| Workspace skeleton | Complete |
| Core types and error model | Complete |
| Fake multi-camera pipeline | Complete |
| UVC descriptor and format negotiation | Complete |
| Android FD wrapper design | Placeholder only |
| Real USB backend | Feature skeleton only |
| JNI binding layer | Not started |
| Performance validation | Not started |
