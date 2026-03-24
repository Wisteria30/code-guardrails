use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

struct Case {
    label: &'static str,
    args: Vec<String>,
}

fn project_root() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .and_then(|d| {
            d.ancestors()
                .find(|a| a.join("Cargo.toml").exists())
                .map(|a| a.to_path_buf())
        })
        .unwrap_or_else(|| env::current_dir().expect("failed to get cwd"))
}

fn run_once(command: &str, args: &[String], cwd: &std::path::Path) -> f64 {
    let start = Instant::now();
    Command::new(command)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("failed to run command");
    start.elapsed().as_secs_f64()
}

fn benchmark(
    label: &str,
    command: &str,
    args: &[String],
    cwd: &std::path::Path,
    iterations: usize,
    warmup: usize,
) {
    for _ in 0..warmup {
        run_once(command, args, cwd);
    }
    let samples: Vec<f64> = (0..iterations)
        .map(|_| run_once(command, args, cwd))
        .collect();
    let mean = samples.iter().sum::<f64>() / samples.len() as f64 * 1000.0;
    let min = samples.iter().cloned().fold(f64::INFINITY, f64::min) * 1000.0;
    let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * 1000.0;
    println!("{label:20} mean={mean:8.2}ms min={min:8.2}ms max={max:8.2}ms");
}

fn main() {
    let mut iterations: usize = 5;
    let mut warmup: usize = 1;

    let args: Vec<String> = env::args().skip(1).collect();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--iterations" => {
                iterations = iter
                    .next()
                    .expect("missing value for --iterations")
                    .parse()
                    .expect("invalid --iterations")
            }
            "--warmup" => {
                warmup = iter
                    .next()
                    .expect("missing value for --warmup")
                    .parse()
                    .expect("invalid --warmup")
            }
            _ => {
                eprintln!("Usage: bench [--iterations N] [--warmup N]");
                std::process::exit(1);
            }
        }
    }

    let root = project_root();
    let engine = root
        .join("target")
        .join("release")
        .join("code-guardrails-engine");
    if !engine.exists() {
        eprintln!("Build the Rust engine first: cargo build --release");
        std::process::exit(1);
    }
    let engine_str = engine.to_str().unwrap().to_string();

    let cases = vec![
        Case {
            label: "clean changed-only",
            args: vec![
                engine_str.clone(),
                "scan-file".into(),
                "--file".into(),
                "fixtures/python/fallback/should_pass/no_fallback.py".into(),
                "--config-dir".into(),
                ".".into(),
            ],
        },
        Case {
            label: "violating changed-only",
            args: vec![
                engine_str.clone(),
                "scan-file".into(),
                "--file".into(),
                "fixtures/python/fallback/should_fail/or_default.py".into(),
                "--config-dir".into(),
                ".".into(),
            ],
        },
        Case {
            label: "sparse full scan",
            args: vec![
                engine_str.clone(),
                "scan-tree".into(),
                "--root".into(),
                "fixtures/python/fallback/should_pass".into(),
                "--config-dir".into(),
                ".".into(),
            ],
        },
        Case {
            label: "dense full scan",
            args: vec![
                engine_str.clone(),
                "scan-tree".into(),
                "--root".into(),
                "fixtures".into(),
                "--config-dir".into(),
                ".".into(),
            ],
        },
        Case {
            label: "raw ast-grep",
            args: vec![
                "scan".into(),
                "--json=stream".into(),
                "fixtures/python/fallback/should_fail/or_default.py".into(),
            ],
        },
    ];

    for case in &cases {
        let (cmd, cmd_args) = if case.label == "raw ast-grep" {
            ("ast-grep", case.args.as_slice())
        } else {
            let (first, rest) = case.args.split_first().unwrap();
            (first.as_str(), rest)
        };
        benchmark(case.label, cmd, cmd_args, &root, iterations, warmup);
    }
}
