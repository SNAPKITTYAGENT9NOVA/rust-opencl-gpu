# rust-opencl-gpu

Rust + OpenCL (via the [`ocl`](https://crates.io/crates/ocl) crate) experiments in composable GPU kernels, plus a SystemVerilog bit-string accelerator.

## Layout

| Path | Contents |
|------|----------|
| `src/functor/` | Library (`gpu` crate): composable element-wise GPU operations over a `u8` Boolean buffer (`0` = false, `1` = canonical true) |
| `src/main.rs` | Binary: runs a small OpenCL kernel through a `Kernel` trait on the first available device |
| `src/demo.rs` | Standalone host-only demo of the same kernel design; needs no GPU and is not part of the Cargo build (`rustc src/demo.rs`) |
| `benchmark.cu` | CUDA benchmark of L1 cache pollution from streaming loads (separate from the Rust crate) |
| `bit_accelerator/` | SystemVerilog bit-string accelerator: RTL, testbenches, Why3 proofs, ISA, docs (see `bit_accelerator/README.md`) |
| `OpenCL-SDK-v2025.07.23-Win-x64/` | Vendored Windows OpenCL SDK |
| `notes.txt` | Dependency note |

## The `functor` library

- **Kernels:** `Negate` (`output[i] = input[i] ^ 1`, always canonical output) and `Add` (`output[i] = a[i] + b[i]`, wrapping `u8`).
- **`Expr`:** an expression tree such as `Negate(Add(x, y))`. Every node runs as an OpenCL kernel; nothing is computed on the host.
- **Agents:** `ValidationAgent`, `ExecutionAgent`, `VerificationAgent` and `Workflow` split input validation, execution and output checking.
- **Canonical form:** `bools_to_gpu`, `gpu_to_bools`, `is_canonical` and `validate_buffer` convert and check the 0/1 representation.
- **Kernel registry:** `KERNEL_REGISTRY` and `validate_registry` declare each kernel's name, parameters, minimum work items and safety notes.
- **Errors:** `GpuError` covers OpenCL failures, invalid counts, short buffers, missing `Expr` inputs, empty buffers and zero local sizes.

## Requirements

- Rust (edition 2024, so a recent stable toolchain).
- An OpenCL runtime and a device (GPU or CPU) with its ICD installed.
- On Windows, the vendored SDK provides the headers and `OpenCL.lib`; elsewhere, install your platform's OpenCL development package.

## Build and run

```sh
cargo build --release
cargo run --release        # runs src/main.rs
```

## Bit-string accelerator

The hardware design is documented in [`bit_accelerator/README.md`](bit_accelerator/README.md). A copy of this tree also lives in the `bit-string-accelerator` repository.
