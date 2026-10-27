use std::{
    fmt::Write as _,
    fs, io,
    time::{SystemTime, UNIX_EPOCH},
};

use arrow_array::{Array, cast::AsArray};
use arrow_schema::DataType;

use super::{Measurement, config::Args};

pub const HEADER: &str = "rows,null_every,operation,repetition,elapsed_ns,input_value_bytes,output_value_bytes,scenario,run_id,run_index,warmups,repetitions,input_buffer_bytes,output_buffer_bytes,order_index,rustc,target,profile,opt_level,target_features,rustflags,cpu,logical_cpus,unix_time_secs,protocol_version";

pub struct Metadata {
    pub run_id: String,
    pub run_index: usize,
    unix_time_secs: u64,
    cpu: String,
    logical_cpus: usize,
}

impl Metadata {
    pub fn collect(run_index: usize) -> Self {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before Unix epoch");
        // Best effort on Linux. Other systems explicitly record unknown.
        let cpu = fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|text| {
                text.lines().find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    matches!(key.trim(), "model name" | "Hardware").then(|| value.trim().to_owned())
                })
            })
            .unwrap_or_else(|| "unknown".into());
        Self {
            run_id: format!("{}-{}", since_epoch.as_nanos(), std::process::id()),
            run_index,
            unix_time_secs: since_epoch.as_secs(),
            cpu,
            logical_cpus: std::thread::available_parallelism().map_or(0, usize::from),
        }
    }
}

pub(super) fn value_payload_bytes(array: &dyn Array) -> usize {
    match array.data_type() {
        DataType::Utf8 => array.as_string::<i32>().value_data().len(),
        DataType::UInt16 => array.len() * 2,
        DataType::UInt32 => array.len() * 4,
        DataType::UInt64 | DataType::Float64 => array.len() * 8,
        other => panic!("no value payload size for {other}"),
    }
}

/// Linear interpolation at (n - 1) * p (type-7 quantile).
pub(super) fn percentile(sorted: &[u128], proportion: usize) -> f64 {
    assert!(!sorted.is_empty() && proportion <= 100);
    let position = (sorted.len() - 1) as f64 * proportion as f64 / 100.0;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f64;
    sorted[lower] as f64 * (1.0 - fraction) + sorted[upper] as f64 * fraction
}

pub(super) struct Stats {
    pub median_ms: f64,
    pub p25_ms: f64,
    pub p75_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}

impl Stats {
    pub fn from_samples(samples: &[u128]) -> Self {
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        Self {
            median_ms: percentile(&sorted, 50) / 1e6,
            p25_ms: percentile(&sorted, 25) / 1e6,
            p75_ms: percentile(&sorted, 75) / 1e6,
            min_ms: sorted[0] as f64 / 1e6,
            max_ms: sorted[sorted.len() - 1] as f64 / 1e6,
        }
    }
}

pub(super) fn csv_field(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// Called only after all measurements for an operation have finished.
pub(super) fn write_measurement(
    cfg: &Args,
    metadata: &Metadata,
    result: &Measurement,
    stdout: &mut impl io::Write,
    stderr: &mut impl io::Write,
) -> io::Result<()> {
    let mut csv = String::new();
    for (repetition, elapsed_ns) in result.samples.iter().enumerate() {
        write!(
            csv,
            "{},{},{},{repetition},{elapsed_ns},{},{},{},{},{},{},{},{},{},{},",
            result.rows,
            cfg.null_every,
            result.operation.label(),
            result.input_value_bytes,
            result.output_value_bytes,
            result.scenario.label(),
            metadata.run_id,
            metadata.run_index,
            cfg.warmups,
            cfg.repetitions,
            result.input_buffer_bytes,
            result.output_buffer_bytes,
            result.order_index,
        )
        .expect("formatting into String");
        for field in [
            env!("VARYNTH_BENCH_RUSTC"),
            env!("VARYNTH_BENCH_TARGET"),
            env!("VARYNTH_BENCH_PROFILE"),
            env!("VARYNTH_BENCH_OPT_LEVEL"),
            env!("VARYNTH_BENCH_CARGO_CFG_TARGET_FEATURE"),
            env!("VARYNTH_BENCH_RUSTFLAGS"),
            &metadata.cpu,
        ] {
            write!(csv, "{},", csv_field(field)).expect("formatting into String");
        }
        writeln!(
            csv,
            "{},{},2",
            metadata.logical_cpus, metadata.unix_time_secs
        )
        .expect("formatting into String");
    }
    stdout.write_all(csv.as_bytes())?;
    let stats = Stats::from_samples(&result.samples);
    writeln!(
        stderr,
        "{} scenario={} rows={} null_every={} run={} median={:.3} ms p25={:.3} ms p75={:.3} ms min={:.3} ms max={:.3} ms input_buffer_bytes={} output_buffer_bytes={}",
        result.operation.label(),
        result.scenario.label(),
        result.rows,
        cfg.null_every,
        metadata.run_index,
        stats.median_ms,
        stats.p25_ms,
        stats.p75_ms,
        stats.min_ms,
        stats.max_ms,
        result.input_buffer_bytes,
        result.output_buffer_bytes,
    )
}
