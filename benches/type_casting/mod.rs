pub mod config;
mod data;
pub mod report;
mod validation;

use std::{hint::black_box, io, time::Instant};

use arrow_array::Array;
use arrow_cast::cast::cast;

use config::{Args, Operation, Scenario};
use report::{Metadata, value_payload_bytes, write_measurement};

struct Measurement {
    rows: usize,
    operation: Operation,
    scenario: Scenario,
    order_index: usize,
    samples: Vec<u128>,
    input_value_bytes: usize,
    output_value_bytes: usize,
    input_buffer_bytes: usize,
    output_buffer_bytes: usize,
}

/// Each operation occupies every position once per complete cycle of processes.
fn case_order(count: usize, run_index: usize) -> impl Iterator<Item = usize> {
    (0..count).map(move |offset| (offset + run_index % count) % count)
}

fn measure(
    cfg: &Args,
    operation: Operation,
    scenario: Scenario,
    rows: usize,
    order_index: usize,
) -> Measurement {
    // Only the selected input and one output live at a time.
    let source = data::prepare(operation, scenario, rows, cfg.null_every);
    let target = operation.target();
    let output = cast(source.as_ref(), &target).expect("validation cast failed");
    validation::verify(source.as_ref(), output.as_ref(), &target);
    let mut result = Measurement {
        rows,
        operation,
        scenario,
        order_index,
        samples: Vec::with_capacity(cfg.repetitions),
        input_value_bytes: value_payload_bytes(source.as_ref()),
        input_buffer_bytes: source.get_buffer_memory_size(),
        output_value_bytes: value_payload_bytes(output.as_ref()),
        output_buffer_bytes: output.get_buffer_memory_size(),
    };
    drop(output);
    for round in 0..cfg.warmups + cfg.repetitions {
        let started = Instant::now();
        let output = cast(black_box(source.as_ref()), black_box(&target));
        let elapsed_ns = started.elapsed().as_nanos();
        let output = output
            .unwrap_or_else(|error| panic!("{} cast failed: {error}", result.operation.label()));
        black_box(&output);
        // Allocation and conversion are timed; output destruction is excluded.
        drop(output);
        if round >= cfg.warmups {
            result.samples.push(elapsed_ns);
        }
    }
    result
}

pub fn run(
    cfg: &Args,
    metadata: &Metadata,
    stdout: &mut impl io::Write,
    stderr: &mut impl io::Write,
) -> io::Result<()> {
    validation::validate_small_inputs();
    for &rows in &cfg.rows {
        for &scenario in &cfg.scenarios {
            for (order_index, index) in
                case_order(cfg.operations.len(), metadata.run_index).enumerate()
            {
                let result = measure(cfg, cfg.operations[index], scenario, rows, order_index);
                write_measurement(cfg, metadata, &result, stdout, stderr)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
