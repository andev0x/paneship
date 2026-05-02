use crate::core::prompt::PromptContext;
use crate::core::renderer;
use crate::modules::{directory, git, metadata, package, status};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct BenchmarkOptions {
    pub iterations: usize,
    pub panes: usize,
    pub compare_starship: bool,
    pub width: Option<usize>,
    pub cwd: Option<PathBuf>,
    pub exit_code: i32,
}

impl Default for BenchmarkOptions {
    fn default() -> Self {
        Self {
            iterations: 200,
            panes: 4,
            compare_starship: false,
            width: None,
            cwd: None,
            exit_code: 0,
        }
    }
}

pub struct BenchmarkReport {
    pub paneship_avg: Duration,
    pub starship_avg: Option<Duration>,
}

impl std::fmt::Display for BenchmarkReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Benchmark Report")?;
        writeln!(f, "================")?;
        writeln!(f, "Paneship avg:  {:?}", self.paneship_avg)?;
        if let Some(starship) = self.starship_avg {
            writeln!(f, "Starship avg:  {:?}", starship)?;
            let diff = starship.as_secs_f64() / self.paneship_avg.as_secs_f64();
            writeln!(f, "Speedup:       {:.2}x", diff)?;
        }
        Ok(())
    }
}

pub fn run(options: BenchmarkOptions) -> Result<BenchmarkReport, String> {
    use std::sync::Arc;
    use std::thread;

    let options = Arc::new(options);
    let mut handles = vec![];

    let start = Instant::now();
    for _ in 0..options.panes {
        let opt = Arc::clone(&options);
        handles.push(thread::spawn(move || {
            let mut total_duration = Duration::ZERO;
            for _ in 0..opt.iterations {
                let iter_start = Instant::now();
                let context =
                    PromptContext::from_inputs(opt.cwd.clone(), opt.width, opt.exit_code, None);
                renderer::render(&context);
                total_duration += iter_start.elapsed();
            }
            total_duration / opt.iterations as u32
        }));
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let _duration = start.elapsed();

    // Average of averages
    let paneship_avg = results.iter().sum::<Duration>() / results.len() as u32;

    let mut starship_avg = None;
    if options.compare_starship {
        let starship_duration = benchmark_starship(&options)?;
        starship_avg = Some(starship_duration / options.iterations as u32);
    }

    Ok(BenchmarkReport {
        paneship_avg,
        starship_avg,
    })
}

pub fn run_top() -> Result<(), String> {
    let context = PromptContext::from_inputs(None, Some(80), 0, None);
    let iterations = 100;

    let mut results = vec![];

    // Benchmark Directory
    results.push((
        "directory",
        benchmark_module(|| {
            directory::render_with_max_width(&context, 80);
        }),
    ));

    // Benchmark Git
    results.push((
        "git",
        benchmark_module(|| {
            git::render_with_max_width(&context, 80);
        }),
    ));

    // Benchmark Metadata
    results.push((
        "metadata",
        benchmark_module(|| {
            metadata::render_with_max_width(&context, 80);
        }),
    ));

    // Benchmark Package
    results.push((
        "package",
        benchmark_module(|| {
            package::render_with_max_width(&context, 80);
        }),
    ));

    // Benchmark Status
    results.push((
        "status",
        benchmark_module(|| {
            status::render_cursor(&context);
        }),
    ));

    // Sort by duration descending
    results.sort_by_key(|b| std::cmp::Reverse(b.1));

    println!("{:<12} {:>16}", "Component", "Average Runtime");
    println!("{:<12} {:>16}", "---------", "---------------");
    for (name, duration) in results {
        println!("{:<12} {:>16?}", name, duration / iterations);
    }

    Ok(())
}

fn benchmark_module<F>(f: F) -> Duration
where
    F: Fn(),
{
    let iterations = 100;
    // Warm up
    for _ in 0..10 {
        f();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed()
}

fn benchmark_starship(options: &BenchmarkOptions) -> Result<Duration, String> {
    use std::process::Command;

    let cwd = options
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // Warm up
    for _ in 0..5 {
        let _ = Command::new("starship")
            .arg("prompt")
            .arg("--status")
            .arg(options.exit_code.to_string())
            .current_dir(&cwd)
            .output();
    }

    let start = Instant::now();
    for _ in 0..options.iterations {
        let output = Command::new("starship")
            .arg("prompt")
            .arg("--status")
            .arg(options.exit_code.to_string())
            .current_dir(&cwd)
            .output()
            .map_err(|e| format!("failed to execute starship: {e}"))?;

        if !output.status.success() {
            return Err("starship execution failed".to_string());
        }
    }
    Ok(start.elapsed())
}
