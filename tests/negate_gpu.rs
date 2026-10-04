//! Executable OpenCL tests for the Negate functor. Every negation runs as an OpenCL kernel.
//! Tests FAIL (never silently skip) when no OpenCL device is available.

use gpu::functor::{Add, Expr, GpuContext, GpuError, Negate, UnaryOp};

const SENTINEL: u8 = 0xAA;

fn ctx() -> GpuContext {
    GpuContext::new().expect("OpenCL context creation / kernel compilation failed (no usable OpenCL device?)")
}

fn ctx_local(n: usize) -> GpuContext {
    GpuContext::with_local_size(n).expect("OpenCL context creation failed")
}

/// Deterministic xorshift so no extra dependencies are needed.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn bools(&mut self, n: usize) -> Vec<bool> {
        (0..n).map(|_| self.next() & 1 == 1).collect()
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| (self.next() >> 8) as u8).collect()
    }
}

fn expected_not(input: &[bool]) -> Vec<bool> {
    input.iter().map(|&b| !b).collect()
}

#[test]
fn device_is_reported() {
    let c = ctx();
    let name = c.device_name().expect("device name");
    println!("OpenCL device: {name} (local size {})", c.local_size());
    assert!(!name.is_empty());
}

#[test]
fn scalar_laws_on_device() {
    let c = ctx();
    assert_eq!(Negate.run_bools(&c, &[true]).unwrap(), vec![false], "¬true = false");
    assert_eq!(Negate.run_bools(&c, &[false]).unwrap(), vec![true], "¬false = true");
    assert!(!Negate::denote(true));
    assert!(Negate::denote(false));
    for x in [true, false] {
        let twice = Negate.then(Negate).run_bools(&c, &[x]).unwrap();
        assert_eq!(twice, vec![x], "¬¬x = x for {x}");
    }
}

#[test]
fn small_buffers() {
    let c = ctx();
    let cases: [&[bool]; 5] = [&[], &[true], &[false], &[true, false], &[false, true]];
    for case in cases {
        let got = Negate.run_bools(&c, case).unwrap();
        assert_eq!(got, expected_not(case), "input {case:?}");
    }
}

#[test]
fn non_power_of_two_sizes_every_element() {
    let c = ctx();
    let mut rng = Rng(0x9E3779B97F4A7C15);
    for &n in &[1usize, 3, 7, 17, 31, 63, 65, 127, 257, 1000, 4096] {
        for input in [rng.bools(n), vec![true; n], vec![false; n]] {
            let got = Negate.run_bools(&c, &input).unwrap();
            assert_eq!(got.len(), n);
            assert_eq!(got, expected_not(&input), "n = {n}");
        }
    }
}

#[test]
fn large_buffers() {
    let c = ctx();
    let mut rng = Rng(12345);
    for &n in &[1usize << 20, 1_000_003] {
        let input = rng.bools(n);
        let got = Negate.run_bools(&c, &input).unwrap();
        assert_eq!(got, expected_not(&input), "n = {n}");
    }
}

#[test]
fn nonzero_bytes_are_true_and_output_is_canonical() {
    let c = ctx();
    let raw: Vec<u8> = vec![0, 1, 2, 3, 127, 128, 255, 0, 255];
    let inp = c.upload_raw(&raw).unwrap();
    let out = c.alloc_filled(raw.len(), SENTINEL).unwrap();
    Negate.launch(&c, &inp, &out, raw.len()).unwrap();
    let got = c.download_raw(&out).unwrap();
    let want: Vec<u8> = raw.iter().map(|&b| (b == 0) as u8).collect();
    assert_eq!(got, want);
}

#[test]
fn involution_two_gpu_launches_with_temporary() {
    let c = ctx();
    let mut rng = Rng(777);
    for &n in &[1usize, 3, 7, 17, 31, 63, 65, 127, 257, 1000, 4096, 100_003] {
        let input = rng.bools(n);
        let inp = c.upload_bools(&input).unwrap();
        let tmp = c.alloc_filled(n, SENTINEL).unwrap();
        let out = c.alloc_filled(n, SENTINEL).unwrap();
        Negate.launch(&c, &inp, &tmp, n).unwrap();
        Negate.launch(&c, &tmp, &out, n).unwrap();

        let tmp_raw = c.download_raw(&tmp).unwrap();
        let want_tmp: Vec<u8> = input.iter().map(|&b| (!b) as u8).collect();
        assert_eq!(tmp_raw, want_tmp, "temporary after first negation, n = {n}");

        let out_raw = c.download_raw(&out).unwrap();
        let want: Vec<u8> = input.iter().map(|&b| b as u8).collect();
        assert_eq!(out_raw, want, "output == input after two negations, n = {n}");
    }
}

#[test]
fn involution_via_compose_and_expr() {
    let c = ctx();
    let mut rng = Rng(4242);
    for &n in &[1usize, 17, 65, 1000, 4096] {
        let input = rng.bools(n);
        assert_eq!(Negate.then(Negate).run_bools(&c, &input).unwrap(), input, "Compose n={n}");
        assert_eq!(
            Negate.then(Negate).then(Negate).run_bools(&c, &input).unwrap(),
            expected_not(&input),
            "¬¬¬x = ¬x, n={n}"
        );

        let inp = c.upload_bools(&input).unwrap();
        let e = Expr::negate(Expr::negate(Expr::input(0)));
        let out = e.eval(&c, &[&inp], n).unwrap();
        assert_eq!(c.download_bools(&out, n).unwrap(), input, "Expr n={n}");
    }
}

#[test]
fn negate_does_not_mutate_input() {
    let c = ctx();
    let mut rng = Rng(99);
    for &n in &[1usize, 63, 4096] {
        let raw = rng.bytes(n);
        let inp = c.upload_raw(&raw).unwrap();
        let out = c.alloc(n).unwrap();
        Negate.launch(&c, &inp, &out, n).unwrap();
        assert_eq!(c.download_raw(&inp).unwrap(), raw, "input unchanged after Negate, n={n}");

        Negate.then(Negate).launch(&c, &inp, &out, n).unwrap();
        assert_eq!(c.download_raw(&inp).unwrap(), raw, "input unchanged after Compose, n={n}");

        let e = Expr::negate(Expr::add(Expr::input(0), Expr::input(0)));
        e.eval(&c, &[&inp], n).unwrap();
        assert_eq!(c.download_raw(&inp).unwrap(), raw, "input unchanged after Expr, n={n}");
    }
}

#[test]
fn negate_of_add_matches_host_oracle() {
    let c = ctx();
    let mut rng = Rng(2024);
    for &n in &[1usize, 7, 65, 1000, 4096] {
        // Boolean inputs: Negate(Add(x, y)) is NOR.
        let x = rng.bools(n);
        let y = rng.bools(n);
        let bx = c.upload_bools(&x).unwrap();
        let by = c.upload_bools(&y).unwrap();
        let e = Expr::negate(Expr::add(Expr::input(0), Expr::input(1)));
        let out = e.eval(&c, &[&bx, &by], n).unwrap();
        let want: Vec<bool> = x.iter().zip(&y).map(|(&a, &b)| !(a || b)).collect();
        assert_eq!(c.download_bools(&out, n).unwrap(), want, "NOR, n={n}");

        // Arbitrary bytes, including wrapping sums that hit 0.
        let rx = rng.bytes(n);
        let ry: Vec<u8> = rx.iter().map(|&a| a.wrapping_neg()).collect(); // sum == 0 everywhere
        let (ux, uy) = (c.upload_raw(&rx).unwrap(), c.upload_raw(&ry).unwrap());
        let out = e.eval(&c, &[&ux, &uy], n).unwrap();
        assert_eq!(c.download_raw(&out).unwrap(), vec![1u8; n], "wrapping sum 0 => true, n={n}");

        // ¬¬(x+y) = canonical(x+y)
        let e2 = Expr::negate(e.clone());
        let out2 = e2.eval(&c, &[&bx, &by], n).unwrap();
        let want2: Vec<bool> = x.iter().zip(&y).map(|(&a, &b)| a || b).collect();
        assert_eq!(c.download_bools(&out2, n).unwrap(), want2, "¬¬(x+y), n={n}");
    }
}

#[test]
fn expr_input_passthrough_and_missing_input() {
    let c = ctx();
    let input = vec![true, false, true];
    let inp = c.upload_bools(&input).unwrap();
    let out = Expr::input(0).eval(&c, &[&inp], 3).unwrap();
    assert_eq!(c.download_bools(&out, 3).unwrap(), input);
    match Expr::input(1).eval(&c, &[&inp], 3) {
        Err(GpuError::MissingInput { index: 1, supplied: 1 }) => {}
        other => panic!("expected MissingInput, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn add_functor_on_device() {
    let c = ctx();
    let (a, b) = (vec![1u8, 2, 250, 0, 255], vec![1u8, 3, 10, 0, 1]);
    let (ba, bb) = (c.upload_raw(&a).unwrap(), c.upload_raw(&b).unwrap());
    let out = c.alloc(5).unwrap();
    Add.launch(&c, &ba, &bb, &out, 5).unwrap();
    let want: Vec<u8> = a.iter().zip(&b).map(|(x, y)| x.wrapping_add(*y)).collect();
    assert_eq!(c.download_raw(&out).unwrap(), want);
}

#[test]
fn boundary_guard_no_write_past_count() {
    // Output/input buffers are LONGER than `count`; global size is rounded up past `count`.
    // Any thread with gid >= count that wrote would clobber the sentinel tail.
    for &local in &[1usize, 3, 8, 64, 256] {
        let c = ctx_local(local);
        let l = c.local_size();
        let mut counts = vec![1usize, 2, 5, 17, 100];
        for d in [l.saturating_sub(1), l, l + 1, 2 * l - 1, 2 * l + 1] {
            if d > 0 {
                counts.push(d);
            }
        }
        for count in counts {
            let pad = 3 * l + 7;
            let len = count + pad;
            let mut raw = vec![0u8; len];
            for (i, r) in raw.iter_mut().enumerate() {
                *r = (i % 2) as u8; // tail holds data too: a stray read/write would be visible
            }
            let inp = c.upload_raw(&raw).unwrap();
            let out = c.alloc_filled(len, SENTINEL).unwrap();
            Negate.launch(&c, &inp, &out, count).unwrap();

            let got = c.download_raw(&out).unwrap();
            for i in 0..count {
                assert_eq!(got[i], (raw[i] == 0) as u8, "local={l} count={count} i={i} (stale or wrong)");
            }
            for i in count..len {
                assert_eq!(got[i], SENTINEL, "OOB write at i={i}, local={l} count={count}");
            }
            assert_eq!(c.download_raw(&inp).unwrap(), raw, "input mutated, local={l} count={count}");
        }
    }
}

#[test]
fn boundary_guard_for_add_too() {
    let c = ctx_local(8);
    let count = 13usize;
    let len = count + 40;
    let a = c.upload_raw(&vec![1u8; len]).unwrap();
    let b = c.upload_raw(&vec![2u8; len]).unwrap();
    let out = c.alloc_filled(len, SENTINEL).unwrap();
    Add.launch(&c, &a, &b, &out, count).unwrap();
    let got = c.download_raw(&out).unwrap();
    assert!(got[..count].iter().all(|&v| v == 3));
    assert!(got[count..].iter().all(|&v| v == SENTINEL), "Add wrote past count");
}

#[test]
fn stale_output_values_are_all_overwritten() {
    let c = ctx();
    for &n in &[1usize, 3, 7, 17, 31, 63, 65, 127, 257, 1000, 4096] {
        let inp = c.upload_bools(&vec![true; n]).unwrap();
        let out = c.alloc_filled(n, SENTINEL).unwrap();
        Negate.launch(&c, &inp, &out, n).unwrap();
        assert!(c.download_raw(&out).unwrap().iter().all(|&v| v == 0), "stale value survived, n={n}");

        // Reuse the same output buffer for the opposite input: no stale 0s may remain.
        let inp2 = c.upload_bools(&vec![false; n]).unwrap();
        Negate.launch(&c, &inp2, &out, n).unwrap();
        assert!(c.download_raw(&out).unwrap().iter().all(|&v| v == 1), "stale value survived on reuse, n={n}");
    }
}

#[test]
fn zero_count_is_noop_and_leaves_output_untouched() {
    let c = ctx();
    let inp = c.upload_raw(&[1, 0, 1, 0]).unwrap();
    let out = c.alloc_filled(4, SENTINEL).unwrap();
    Negate.launch(&c, &inp, &out, 0).unwrap();
    assert_eq!(c.download_raw(&out).unwrap(), vec![SENTINEL; 4]);
    assert_eq!(Negate.then(Negate).run_bools(&c, &[]).unwrap(), Vec::<bool>::new());
}

// ---------------- error handling ----------------

#[test]
fn error_kernel_compilation_failure_is_reported() {
    match GpuContext::with_source("__kernel void broken( { this is not OpenCL C") {
        Err(GpuError::Ocl(e)) => println!("compile error surfaced as expected: {e}"),
        Err(other) => panic!("wrong error variant: {other}"),
        Ok(_) => panic!("invalid OpenCL source compiled"),
    }
}

#[test]
fn error_buffer_too_short() {
    let c = ctx();
    let inp = c.upload_raw(&[1, 0, 1]).unwrap();
    let out = c.alloc_filled(8, SENTINEL).unwrap();
    match Negate.launch(&c, &inp, &out, 5) {
        Err(GpuError::BufferTooShort { which: "input", len: 3, count: 5 }) => {}
        other => panic!("expected BufferTooShort(input), got {other:?}"),
    }
    let short_out = c.alloc_filled(2, SENTINEL).unwrap();
    let inp5 = c.upload_raw(&[1; 5]).unwrap();
    match Negate.launch(&c, &inp5, &short_out, 5) {
        Err(GpuError::BufferTooShort { which: "output", len: 2, count: 5 }) => {}
        other => panic!("expected BufferTooShort(output), got {other:?}"),
    }
    assert_eq!(c.download_raw(&out).unwrap(), vec![SENTINEL; 8], "failed launch must not write");
    assert_eq!(c.download_raw(&short_out).unwrap(), vec![SENTINEL; 2]);
}

#[test]
fn error_invalid_element_count() {
    let c = ctx();
    let inp = c.upload_raw(&[1]).unwrap();
    let out = c.alloc(1).unwrap();
    let too_big = u32::MAX as usize + 1;
    match Negate.launch(&c, &inp, &out, too_big) {
        Err(GpuError::InvalidCount(n)) if n == too_big => {}
        other => panic!("expected InvalidCount, got {other:?}"),
    }
    match Expr::input(0).eval(&c, &[&inp], 0) {
        Err(GpuError::InvalidCount(0)) => {}
        other => panic!("expected InvalidCount(0) from Expr::eval, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn error_invalid_local_size() {
    let mut c = ctx();
    assert!(matches!(c.set_local_size(0), Err(GpuError::InvalidLocalSize)));
    assert!(matches!(GpuContext::with_local_size(0), Err(GpuError::InvalidLocalSize)));
}

#[test]
fn error_buffer_allocation_failure_is_reported() {
    let c = ctx();
    // Zero-length buffers cannot exist in OpenCL (the ocl crate would panic): typed error instead.
    assert!(matches!(c.alloc(0), Err(GpuError::EmptyBuffer)));
    assert!(matches!(c.upload_raw(&[]), Err(GpuError::EmptyBuffer)));
    // Far beyond any device's max allocation size.
    assert!(matches!(c.alloc(usize::MAX / 4), Err(GpuError::Ocl(_))));
}
