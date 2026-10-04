use ocl::{ProQue, Result};

trait Kernel: Sized {
    fn src() -> &'static str;
    fn name() -> &'static str;
    fn local_size() -> usize;
    fn run(&self, proque: &ProQue) -> Result<Vec<f32>>;
}

struct Add;

impl Kernel for Add {
    fn src() -> &'static str {
        r#"
            __kernel void add(__global float* c) {
                c[get_global_id(0)] = get_local_id(0);
            }
        "#
    }
    fn name() -> &'static str { "add" }
    fn local_size() -> usize { 4 }

    fn run(&self, proque: &ProQue) -> Result<Vec<f32>> {
        let buffer = proque.create_buffer::<f32>()?;
        let kernel = proque.kernel_builder(Self::name())
            .arg(&buffer)
            .build()?;
        unsafe { kernel.cmd().local_work_size(Self::local_size()).enq()?; }
        let mut result = vec![0.0f32; 128];
        buffer.cmd().read(&mut result).enq()?;
        Ok(result)
    }
}

fn format_output(data: &[f32]) -> String {
    data.iter()
        .enumerate()
        .fold(String::new(), |acc, (i, &val)| {
            if i > 0 && i % 16 == 0 {
                format!("{}\n{:>3} ", acc, val as i32)
            } else {
                format!("{}{:>3} ", acc, val as i32)
            }
        })
}

fn execute<K: Kernel>(kernel: K, proque: &ProQue) -> Result<Vec<f32>> {
    kernel.run(proque)
}

fn main() -> Result<()> {
    let proque = ProQue::builder()
        .src(Add::src())
        .dims(128)
        .build()?;

    let result = execute(Add, &proque)?;
    println!("{}", format_output(&result));

    negate_demo();

    Ok(())
}

fn negate_demo() {
    use gpu::functor::{GpuContext, Negate, UnaryOp};

    let ctx = match GpuContext::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("negate demo: {e}");
            std::process::exit(1);
        }
    };
    let input = [true, false, true, true, false];
    let once = Negate.run_bools(&ctx, &input).expect("negate failed");
    let twice = Negate.then(Negate).run_bools(&ctx, &input).expect("negate∘negate failed");
    println!("\ndevice : {}", ctx.device_name().unwrap_or_default());
    println!("input  : {input:?}");
    println!("¬input : {once:?}");
    println!("¬¬input: {twice:?}");
    assert_eq!(twice, input);
}
