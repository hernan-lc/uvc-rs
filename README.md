# UVC Rust Engine

Rust-first Android UVC camera engine workspace. The current implementation is a compile-safe foundation for multi-camera concurrency validation, not a complete Android UVC driver.

## Current status

Completed against `plan.md` and `.kilo/plans/rust-uvc-engine.md`:

- Cargo workspace with `uvc-core`, `uvc-driver`, `uvc-jni`, and `uvc-cli`.
- Pure-Rust core types for camera identity, formats, frames, bounded frame channels, errors, and pipeline traits.
- Deterministic fake multi-camera backend that streams multiple synthetic cameras concurrently.
- CLI validation command for fake multi-camera runs.
- UVC descriptor parsing models with synthetic descriptor tests.
- Optional `rusb` feature plus backend, device, endpoint, interface, transfer, and device-profile abstractions.
- rusb-backed device discovery, active-config UVC interface parsing, device open, claim, alternate-setting activation, libusb async ISO multi-transfer ring, UVC packet assembly, MJPEG boundary detection, and assembled-frame sink integration.
- Android file-descriptor identity wrapper and Kotlin-facing JNI exports in `uvc-jni`.
- Workspace formatting, checks, and tests are passing.

Not complete yet:

- No hardware-validated UVC stream, decoded-frame pipeline, or Android rendering path.
- No Android NDK integration or Android file-descriptor-to-libusb path.
- No Kotlin companion classes or Android surface, `ANativeWindow`, or `HardwareBuffer` path.
- No performance benchmark suite.
- No `LICENSE` file yet, despite the workspace package license metadata.

## Workspace layout

```text
crates/
  uvc-core/
    Pure Rust data model, error types, frame channel, and pipeline trait.
  uvc-driver/
    UVC descriptor parser, backend traits, rusb-backed device discovery/session management, libusb async ISO multi-transfer ring, UVC packet/MJPEG assembly, MJPEG frame sink adapter, fake deterministic camera backend, performance validation example, and concurrency validation harness.
  uvc-jni/
    Android USB file-descriptor identity wrapper, opaque native engine handle, and JNI exports for initialize/start/stop/control/poll/release.
  uvc-cli/
    Local CLI tool for fake multi-camera validation.
```

## Commands

```bash
cargo fmt --all
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p uvc-driver --example fake-multi-camera-perf -- --cameras 4 --seconds 1 --fps 30 --width 64 --height 64 --capacity-frames 128
cargo check -p uvc-driver --features rusb
```

## What next

Recommended order:

1. Validate the libusb async ISO multi-transfer ring on desktop Linux with UVC hardware and measure packet loss/recovery.
2. Add decoded MJPEG-to-RGBA/YUV sink integration for assembled frames.
3. Add Android target checks once the NDK and libusb build environment are configured.
4. Move Android file-descriptor handling from a placeholder into a real `libusb_wrap_sys_device` boundary behind an Android feature.
5. Add Kotlin companion classes for the JNI exports.
6. Add benchmarks for frame buffer reuse, bounded-channel latency, and fake multi-camera throughput.

## Current milestone coverage

| Milestone | Status |
| --- | --- |
| Workspace skeleton | Complete |
| Core types and error model | Complete |
| Fake multi-camera pipeline | Complete |
| UVC descriptor and format negotiation | Complete |
| Android FD wrapper design | Placeholder only |
| Real USB backend | Device discovery, session management, libusb async ISO ring, UVC/MJPEG assembly, and assembled-frame sink complete; hardware validation and decoded MJPEG sink pending |
| JNI binding layer | JNI exports, opaque native engine handles, and fake-camera smoke path complete; Android NDK/libusb wrapping and Kotlin companion classes pending |
| Performance validation | Fake multi-camera throughput, consumer buffer reuse, and bounded-channel latency example complete; criterion benchmarks pending |
