// Cargo also compiles this module for the harness=false benchmark, where #[test]
// functions are omitted. The integration target actually discovers and runs them.
#[allow(unused_imports)]
use super::*;

#[test]
fn quantiles_interpolate_even_samples() {
    assert_eq!(report::percentile(&[10, 20, 30, 40], 50), 25.0);
    assert_eq!(report::percentile(&[10, 20, 30, 40], 25), 17.5);
    assert_eq!(report::percentile(&[10], 75), 10.0);
}

#[test]
fn statistics_preserve_samples_and_units() {
    let samples = [40_000_000, 10_000_000, 30_000_000, 20_000_000];
    let stats = report::Stats::from_samples(&samples);
    assert_eq!(stats.median_ms, 25.0);
    assert_eq!(stats.p25_ms, 17.5);
    assert_eq!(stats.p75_ms, 32.5);
    assert_eq!(stats.min_ms, 10.0);
    assert_eq!(stats.max_ms, 40.0);
    assert_eq!(samples[0], 40_000_000);
}

#[test]
fn null_mask_preserves_both_value_classes() {
    for parity in 0..2 {
        let nulls = (parity..10000)
            .step_by(2)
            .filter(|&i| data::is_null(i, 2))
            .count();
        assert!((2200..2800).contains(&nulls));
    }
    assert!(!(0..100).any(|i| data::is_null(i, 0)));
    assert!((0..100).all(|i| data::is_null(i, 1)));
}

#[test]
fn rotation_balances_positions_across_processes() {
    for count in [1, 2, 3, 7] {
        for position in 0..count {
            let mut seen: Vec<_> = (0..count)
                .map(|run_index| case_order(count, run_index).nth(position).unwrap())
                .collect();
            seen.sort_unstable();
            assert_eq!(seen, (0..count).collect::<Vec<_>>());
        }
    }
}

#[test]
fn boundary_casts_roundtrip() {
    validation::validate_small_inputs();
}

#[test]
fn parser_accepts_filters_and_cargo_flag() {
    let cfg = Args::parse(
        [
            "--bench",
            "--rows",
            "1,4097",
            "--operations",
            "utf8_to_f64,u16_to_u32",
            "--scenarios",
            "large,small",
            "--runs",
            "3",
            "--warmups",
            "0",
        ]
        .map(str::to_owned),
    )
    .expect(config::USAGE);
    assert_eq!(cfg.rows, [1, 4097]);
    assert_eq!(cfg.operations, [Operation::Utf8ToF64, Operation::U16ToU32]);
    assert_eq!(cfg.scenarios, [Scenario::Large, Scenario::Small]);
    assert_eq!(cfg.runs, 3);
    assert_eq!(cfg.warmups, 0);
}

#[test]
fn parser_rejects_invalid_configurations() {
    for arguments in [
        vec!["--rows", "0"],
        vec!["--rows", ""],
        vec!["--rows", "1,1"],
        vec!["--repetitions", "0"],
        vec!["--runs", "0"],
        vec!["--warmups", "-1"],
        vec!["--operations", ""],
        vec!["--operations", "missing"],
        vec!["--operations", "u16_to_u32,u16_to_u32"],
        vec!["--scenarios", "missing"],
        vec!["--scenarios", "small,small"],
        vec!["--unknown"],
        vec!["--rows"],
    ] {
        assert!(
            Args::parse(arguments.iter().map(|s| (*s).to_owned())).is_err(),
            "{arguments:?}"
        );
    }
    assert!(
        Args::parse([
            "--warmups".to_owned(),
            usize::MAX.to_string(),
            "--repetitions".to_owned(),
            "1".to_owned()
        ])
        .is_err()
    );
}

#[test]
fn parser_default_limits_working_set() {
    let cfg = Args::parse([]).expect(config::USAGE);
    assert_eq!(cfg.rows, [1_000_000]);
    assert_eq!(cfg.scenarios, [Scenario::Mixed]);
    assert_eq!(cfg.operations.len(), 7);
    assert_eq!(cfg.runs, 1);
}

#[test]
fn independent_text_inputs_preserve_numeric_values() {
    use arrow_array::{
        cast::AsArray,
        types::{Float64Type, UInt64Type},
    };
    use arrow_schema::DataType;
    for scenario in [Scenario::Small, Scenario::Large, Scenario::Mixed] {
        for (numeric_op, text_op, target) in [
            (Operation::U64ToUtf8, Operation::Utf8ToU64, DataType::UInt64),
            (
                Operation::F64ToUtf8,
                Operation::Utf8ToF64,
                DataType::Float64,
            ),
        ] {
            let numeric = data::prepare(numeric_op, scenario, 1000, 2);
            let text = data::prepare(text_op, scenario, 1000, 2);
            let parsed = cast(text.as_ref(), &target).unwrap();
            assert_eq!(numeric.nulls(), parsed.nulls());
            for i in 0..numeric.len() {
                if numeric.is_null(i) {
                    continue;
                }
                match target {
                    DataType::UInt64 => assert_eq!(
                        numeric.as_primitive::<UInt64Type>().value(i),
                        parsed.as_primitive::<UInt64Type>().value(i)
                    ),
                    DataType::Float64 => assert_eq!(
                        numeric.as_primitive::<Float64Type>().value(i).to_bits(),
                        parsed.as_primitive::<Float64Type>().value(i).to_bits()
                    ),
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[test]
fn csv_escapes_quotes_commas_and_newlines() {
    assert_eq!(
        report::csv_field("cpu,\"model\"\nflags"),
        "\"cpu,\"\"model\"\"\nflags\""
    );
}

#[test]
fn runner_covers_scenarios_nulls_and_sampling_boundary() {
    for null_every in [0, 1, 2] {
        let cfg = Args::parse([
            "--rows".into(),
            "1,4096,4097".into(),
            "--warmups".into(),
            "0".into(),
            "--repetitions".into(),
            "2".into(),
            "--scenarios".into(),
            "small,large,mixed".into(),
            "--null-every".into(),
            null_every.to_string(),
        ])
        .expect(config::USAGE);
        let metadata = Metadata::collect(1);
        let mut csv = Vec::new();
        let mut summary = Vec::new();
        run(&cfg, &metadata, &mut csv, &mut summary).unwrap();
        assert_eq!(
            String::from_utf8(csv).unwrap().lines().count(),
            3 * 3 * 7 * 2
        );
        assert_eq!(
            String::from_utf8(summary).unwrap().lines().count(),
            3 * 3 * 7
        );
        assert!(report::HEADER.contains("input_buffer_bytes"));
    }
}
