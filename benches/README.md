# Arrow casting experiment

```sh
cargo bench --bench type-casting-experiment -- --help
cargo bench --bench type-casting-experiment -- --rows 1000000 --runs 3 > casts.csv
cargo bench --bench type-casting-experiment -- --operations utf8_to_f64 --scenarios small,large,mixed --rows 1000000 --null-every 2 > parsing.csv
cargo test --test type_casting_experiment
```

The default is **one million rows**, three warmups, ten recorded repetitions,
no nulls, all seven operations, the `mixed` scenario and one process. Larger
working sets remain available with `--rows 1000000,10000000,50000000`.
`--operations` and `--scenarios` accept comma-separated lists; omitted operations
are never prepared. `--null-every N` produces approximately 1/N nulls using a
deterministic hash; 0 disables nulls, 1 makes all rows null.

## Data

| Scenario | Unsigned integers | Float64 | Parser input |
| --- | --- | --- | --- |
| `small` | 0–999 | Nonnegative values below 143 | Short integers; decimal floats |
| `large` | Near the source type's maximum | Positive and negative values around 1e100 | Long integers; scientific floats |
| `mixed` | Varying widths; u64 alternates short and high-bit values | Signed zero, negatives, ordinary fractions, extreme finite values and subnormals | Decimal and scientific floats; varying integer lengths |

Each scenario is deterministic. Text is generated with Rust formatting directly,
independently of Arrow's formatter. There is no retained numeric copy of parser
inputs. These scenarios describe finite, valid conversions; invalid strings,
NaN and infinities require separate experiments. The new `mixed` data differs
from the previous generator: compare results only within the same protocol.

## Measurement protocol

One operation is prepared, checked, warmed up and measured at a time. Its input
is then released before preparing the next operation. This reduces live memory
and permits filtering. Unlike the previous family-based protocol, repetitions
of an operation are consecutive and reuse the same input. This measures a reused
working set, whose cache residency depends on size, rather than cold-cache latency.

The timed region includes Arrow's default cast dispatch, conversion and output
allocation. Preparation, validation, output destruction, statistics and I/O are
excluded. A validation cast runs even with `--warmups 0`; that setting therefore
does not mean a cold first call. Inputs of up to 4096 rows are checked exhaustively;
larger inputs use stratified samples. Independent boundary fixtures also run.

`--runs N` starts N **sequential fresh processes**, each with its own preparation
and warmups. Operation order rotates by process index. A complete cycle needs as
many runs as selected operations; fewer runs only partially balance positions.
CSV records include `run_index` and `order_index`. Keep process-level medians
separate when assessing variation between runs; repetitions in one process are
not independent experiments. There is no CPU pinning or frequency control, so
background load and thermal drift can still affect results. Larger row sizes and
scenarios retain their requested order.

## Output and reproducibility

Stdout is CSV; stderr contains per-operation medians, quartiles and extrema.
The original seven columns remain first, followed by:

- Scenario, process identity/index, warmups, repetitions and operation order.
- Input/output Arrow buffer capacities, including offsets and validity buffers.
- Compiler version, target, Cargo profile, optimization level, target features,
  encoded compiler flags, CPU model, available logical CPUs and Unix time.
- `protocol_version=2`, identifying this generator and scheduling protocol.

Build metadata comes from Cargo at compilation time through `build.rs`, not from
whichever compiler happens to be installed when the binary runs. CPU identification
is best effort via Linux `/proc/cpuinfo`; other systems record `unknown`. Keep
`Cargo.lock` and the exact source revision with exported results: metadata does
not capture dependency versions or arbitrary changes to external configuration.

`input_value_bytes` and `output_value_bytes` exclude offsets and validity buffers;
they are not total memory traffic. Buffer capacities are also **not process peak
RSS**: temporary allocations, allocator retention and shared buffers prevent that
interpretation. Very large explicitly requested inputs can still exhaust RAM or
exceed Arrow Utf8's 32-bit offsets. No automatic memory limit is inferred.

## Tests and implementation

The benchmark entry point handles CLI/process orchestration. `type_casting/`
separates configuration, input generation, validation, measurement and reporting.
`tests/type_casting_experiment.rs` compiles the same modules with Rust's normal
test harness, so `cargo test` executes their tests without running large benchmarks.
`cargo test --all-targets` also invokes the harness-free benchmark entry point,
which exits without running measurements when Cargo's `--bench` flag is absent.
This also supports test discovery with `cargo test --all-targets -- --list`.
To launch the compiled benchmark executable directly, include `--bench`.
