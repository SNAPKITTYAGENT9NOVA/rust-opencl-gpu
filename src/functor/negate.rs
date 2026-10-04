use super::{GpuContext, Result, UnaryOp};
use ocl::Buffer;

pub(super) const KERNEL_SRC: &str = r#"
__kernel void negate_u8(__global const uchar* input,
                        __global uchar* output,
                        const uint count) {
    size_t i = get_global_id(0);
    if (i >= count) {
        return;
    }
    output[i] = (input[i] == 0) ? (uchar)1 : (uchar)0;
}
"#;

/// Logical negation `¬x`: `output[i] = !input[i]`, executed on the OpenCL device.
#[derive(Clone, Copy, Debug, Default)]
pub struct Negate;

impl Negate {
    /// Reference semantics of the functor on a single Boolean.
    pub fn denote(x: bool) -> bool {
        !x
    }
}

impl UnaryOp for Negate {
    fn launch(
        &self,
        ctx: &GpuContext,
        input: &Buffer<u8>,
        output: &Buffer<u8>,
        count: usize,
    ) -> Result<()> {
        let (global, local) =
            ctx.work_sizes(count, &[("input", input.len()), ("output", output.len())])?;
        if count == 0 {
            return Ok(());
        }
        let kernel = ctx
            .proque()
            .kernel_builder("negate_u8")
            .global_work_size(global)
            .local_work_size(local)
            .arg(input)
            .arg(output)
            .arg(count as u32)
            .build()?;
        // SAFETY: arguments match the kernel signature; the kernel guards gid < count and
        // count <= both buffer lengths was validated above.
        unsafe { kernel.enq()? };
        Ok(())
    }
}
