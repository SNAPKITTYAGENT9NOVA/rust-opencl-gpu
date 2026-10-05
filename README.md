# rust-opencl-gpu

[![CI](https://github.com/SNAPKITTYAGENT9NOVA/rust-opencl-gpu/actions/workflows/ci.yml/badge.svg)](https://github.com/SNAPKITTYAGENT9NOVA/rust-opencl-gpu/actions/workflows/ci.yml)

Rust + OpenCL (via the [`ocl`](https://crates.io/crates/ocl) crate) experiments in composable GPU kernels, plus a SystemVerilog bit-string accelerator.

## Layout

| Path | Contents |
|------|----------|
| `src/functor/` | Library (`gpu` crate): composable element-wise GPU operations over a `u8` Boolean buffer (`0` = false, `1` = true) |
| `tests/negate_gpu.rs` | Device tests: every operation runs as an OpenCL kernel and is checked against a host oracle |
| `src/main.rs` | Binary: runs a small OpenCL kernel through a `Kernel` trait, then the `Negate` and agent-workflow demos, on the first available device |
| `src/demo.rs` | Standalone host-only demo of the same kernel design; needs no GPU and is not part of the Cargo build (`rustc src/demo.rs`) |
| `benchmark.cu` | CUDA benchmark of L1 cache pollution from streaming loads (separate from the Rust crate) |
| `bit_accelerator/` | SystemVerilog bit-string accelerator: RTL, testbench, Why3 proofs, ISA, docs (see `bit_accelerator/README.md`) |
| `.github/workflows/ci.yml` | CI: Rust fmt/clippy/tests on PoCL, minimum Rust version, accelerator lint/simulation/proofs |
| `LICENSE` | SnapKitty Bit Hardware Source License |
| `OpenCL-SDK-v2025.07.23-Win-x64/` | Vendored Windows OpenCL SDK |
| `notes.txt` | Dependency note |

## The `functor` library

- **Kernels:** `Negate` (`output[i] = input[i] ^ 1`) and `Add` (`output[i] = a[i] + b[i]`, wrapping `u8`). `Negate` rejects any input byte other than 0 or 1; `Add` of two true values gives 2, so `Negate(Add(x, y))` is NOR only where `x` and `y` are never both true.
- **`Expr`:** an expression tree such as `Negate(Add(x, y))`. Every node runs as an OpenCL kernel; `Negate` also reads its input back to the host to check it is canonical before launching.
- **Agents:** `ValidationAgent`, `ExecutionAgent`, `VerificationAgent` and `Workflow` split input validation, execution and output checking.
- **Canonical form:** `bools_to_gpu`, `gpu_to_bools`, `is_canonical` and `validate_buffer` convert and check the 0/1 representation.
- **Kernel registry:** `KERNEL_REGISTRY` and `validate_registry` declare each kernel's name, parameters, minimum work items and safety notes.
- **Errors:** `GpuError` covers OpenCL failures, invalid counts, short buffers, missing `Expr` inputs, empty buffers and zero local sizes.

## Requirements

- Rust 1.85 or newer (edition 2024).
- An OpenCL runtime with at least one device. On Ubuntu 24.04,
  `sudo apt-get install pocl-opencl-icd ocl-icd-opencl-dev` provides a CPU device.
- On Windows, the vendored SDK provides the headers and `OpenCL.lib`.

## Build, run and test

```sh
cargo build --release
cargo run --release                     # src/main.rs
cargo fmt --check
cargo clippy --all-targets -- -D warnings
RUST_TEST_THREADS=1 cargo test          # 8 host-only unit tests + 36 device tests
```

Device tests fail, rather than skip, when no OpenCL device is available.

Run the tests serially on PoCL. PoCL 5.0 can abort with a
`pocl_release_dlhandle_cache` assertion when several OpenCL contexts are released
from different threads at once. With one test thread the suite passed 40 of 40
runs, against 26 of 40 with parallel threads. Real GPU drivers do not use
PoCL's code path.

`benchmark.cu` needs the CUDA toolkit and an NVIDIA GPU; it is not built by Cargo or CI.

## Bit-string accelerator

The hardware design is documented in [`bit_accelerator/README.md`](bit_accelerator/README.md).
`cd bit_accelerator && make test` runs lint, the testbench under Verilator and
Icarus, and the Why3 proofs (needs `verilator iverilog why3 z3`, then
`why3 config detect`). A copy of this tree also lives in the `bit-string-accelerator` repository.
