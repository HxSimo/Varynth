use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt16Type, UInt32Type, UInt64Type};
use arrow_array::{Array, PrimitiveArray};
use arrow_cast::cast::cast;
use arrow_schema::DataType;

pub(super) fn verify(input: &dyn Array, output: &dyn Array, target: &DataType) {
    assert_eq!(output.len(), input.len(), "cast row count mismatch");
    assert_eq!(output.data_type(), target, "cast target type mismatch");
    assert_eq!(
        output.null_count(),
        input.null_count(),
        "cast null count mismatch"
    );
    let len = input.len();
    // Exhaustive on small inputs; stratified adjacent pairs cover both value parities
    // on large inputs, without parsing every string before a benchmark.
    let indices: Vec<usize> = if len <= 4096 {
        (0..len).collect()
    } else {
        (0..128)
            .flat_map(|part| {
                let index = (len - 2) / 127 * part;
                [index, index + 1]
            })
            .chain([len - 2, len - 1])
            .collect()
    };
    for index in indices {
        assert_eq!(
            input.is_null(index),
            output.is_null(index),
            "null mismatch at {index}"
        );
        if input.is_null(index) {
            continue;
        }
        let (expected, actual) = interpreted_bits(input, output, target, index);
        assert_eq!(actual, expected, "value mismatch at {index}");
    }
}

/// Compare integers as `u64`, and floats as their bit pattern, parsing Utf8 on either side.
fn interpreted_bits(
    input: &dyn Array,
    output: &dyn Array,
    target: &DataType,
    index: usize,
) -> (u64, u64) {
    match (input.data_type(), target) {
        (DataType::Float64, DataType::Utf8) | (DataType::Utf8, DataType::Float64) => (
            numeric_f64(input, index).to_bits(),
            numeric_f64(output, index).to_bits(),
        ),
        _ => (numeric_u64(input, index), numeric_u64(output, index)),
    }
}

fn numeric_u64(array: &dyn Array, index: usize) -> u64 {
    match array.data_type() {
        DataType::UInt16 => array.as_primitive::<UInt16Type>().value(index) as u64,
        DataType::UInt32 => array.as_primitive::<UInt32Type>().value(index) as u64,
        DataType::UInt64 => array.as_primitive::<UInt64Type>().value(index),
        DataType::Utf8 => parse_u64(array.as_string::<i32>().value(index), index),
        other => panic!("unexpected integer array: {other}"),
    }
}

fn numeric_f64(array: &dyn Array, index: usize) -> f64 {
    match array.data_type() {
        DataType::Float64 => array.as_primitive::<Float64Type>().value(index),
        DataType::Utf8 => parse_f64(array.as_string::<i32>().value(index), index),
        other => panic!("unexpected float array: {other}"),
    }
}

fn parse_u64(text: &str, index: usize) -> u64 {
    text.parse::<u64>()
        .unwrap_or_else(|error| panic!("expected a u64 at {index}, got {text:?}: {error}"))
}

fn parse_f64(text: &str, index: usize) -> f64 {
    text.parse::<f64>()
        .unwrap_or_else(|error| panic!("expected an f64 at {index}, got {text:?}: {error}"))
}

pub(super) fn validate_small_inputs() {
    // Independent boundary fixtures complement the generated benchmark data.
    let narrow = PrimitiveArray::<UInt16Type>::from(vec![Some(0), Some(u16::MAX), None]);
    let medium = PrimitiveArray::<UInt32Type>::from(vec![Some(0), Some(u32::MAX), None]);
    let wide = PrimitiveArray::<UInt64Type>::from(vec![Some(0), Some(u64::MAX), None]);
    let floats = PrimitiveArray::<Float64Type>::from(vec![
        Some(0.0),
        Some(-0.0),
        Some(-1.5),
        Some(f64::MIN_POSITIVE),
        Some(f64::MAX),
        Some(f64::from_bits(1)),
        None,
    ]);
    for (source, targets) in [
        (
            &narrow as &dyn Array,
            vec![DataType::UInt32, DataType::UInt64],
        ),
        (&medium as &dyn Array, vec![DataType::UInt64]),
        (&wide as &dyn Array, vec![DataType::Utf8]),
        (&floats as &dyn Array, vec![DataType::Utf8]),
    ] {
        for target in targets {
            let output = cast(source, &target).expect("boundary cast failed");
            verify(source, output.as_ref(), &target);
            if target == DataType::Utf8 {
                let back =
                    cast(output.as_ref(), source.data_type()).expect("boundary parse failed");
                verify(output.as_ref(), back.as_ref(), source.data_type());
            }
        }
    }
}
