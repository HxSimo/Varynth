use std::{fmt::Write, sync::Arc};

use arrow_array::types::{ArrowPrimitiveType, Float64Type, UInt16Type, UInt32Type, UInt64Type};
use arrow_array::{ArrayRef, PrimitiveArray, builder::StringBuilder};

use super::config::{Operation, Scenario};

/// SplitMix64 finalizer: deterministic nulls, independent of value parity.
pub(super) fn is_null(index: usize, null_every: usize) -> bool {
    if null_every == 0 {
        return false;
    }
    let mut bits = (index as u64).wrapping_add(0x9e3779b97f4a7c15);
    bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d049bb133111eb);
    (bits ^ (bits >> 31)).is_multiple_of(null_every as u64)
}

fn integer_value(i: usize) -> u64 {
    (i as u64).wrapping_mul(17).wrapping_add(3)
}

fn unsigned_value(i: usize, scenario: Scenario, max: u64) -> u64 {
    let value = integer_value(i);
    match scenario {
        Scenario::Small => value % 1000,
        Scenario::Large => max - value % 1000,
        Scenario::Mixed => {
            if i.is_multiple_of(2) {
                value & max
            } else {
                (value | (1_u64 << 63)) & max
            }
        }
    }
}

fn float_value(i: usize, scenario: Scenario) -> f64 {
    match scenario {
        Scenario::Small => (integer_value(i) % 1000) as f64 / 7.0,
        Scenario::Large => {
            let magnitude = 1e100 + (i % 1000) as f64 * 1e97;
            if i.is_multiple_of(2) {
                magnitude
            } else {
                -magnitude
            }
        }
        Scenario::Mixed => match i % 10 {
            0 => 0.0,
            1 => -0.0,
            2 => -1.5,
            3 => integer_value(i) as f64 / 7.0,
            4 => 1e-100,
            5 => -1e100,
            6 => f64::MIN_POSITIVE,
            7 => f64::from_bits(1),
            8 => f64::MAX,
            _ => -f64::MAX,
        },
    }
}

fn primitive<T: ArrowPrimitiveType>(
    rows: usize,
    null_every: usize,
    value: impl Fn(usize) -> T::Native,
) -> ArrayRef {
    let array: PrimitiveArray<T> = if null_every == 0 {
        PrimitiveArray::from_iter_values((0..rows).map(value))
    } else {
        (0..rows)
            .map(|i| (!is_null(i, null_every)).then(|| value(i)))
            .collect()
    };
    Arc::new(array)
}

/// Build parser inputs directly, independently of Arrow's formatter and without
/// retaining an additional numeric array. All preparation is outside timing.
fn text(rows: usize, null_every: usize, scenario: Scenario, floats: bool) -> ArrayRef {
    let mut builder = StringBuilder::new();
    let mut buffer = String::with_capacity(32);
    for i in 0..rows {
        if is_null(i, null_every) {
            builder.append_null();
            continue;
        }
        buffer.clear();
        if floats {
            let value = float_value(i, scenario);
            if scenario != Scenario::Small
                && (i.is_multiple_of(2)
                    || value.abs() >= 1e16
                    || (value != 0.0 && value.abs() < 1e-6))
            {
                // 17 significant digits preserve finite f64 values on roundtrip.
                write!(buffer, "{value:.16e}").expect("formatting into String");
            } else {
                write!(buffer, "{value}").expect("formatting into String");
            }
        } else {
            write!(buffer, "{}", unsigned_value(i, scenario, u64::MAX))
                .expect("formatting into String");
        }
        builder.append_value(&buffer);
    }
    Arc::new(builder.finish())
}

pub(super) fn prepare(
    operation: Operation,
    scenario: Scenario,
    rows: usize,
    null_every: usize,
) -> ArrayRef {
    match operation {
        Operation::U16ToU32 | Operation::U16ToU64 => {
            primitive::<UInt16Type>(rows, null_every, |i| {
                unsigned_value(i, scenario, u16::MAX as u64) as u16
            })
        }
        Operation::U32ToU64 => primitive::<UInt32Type>(rows, null_every, |i| {
            unsigned_value(i, scenario, u32::MAX as u64) as u32
        }),
        Operation::U64ToUtf8 => {
            primitive::<UInt64Type>(rows, null_every, |i| unsigned_value(i, scenario, u64::MAX))
        }
        Operation::F64ToUtf8 => {
            primitive::<Float64Type>(rows, null_every, |i| float_value(i, scenario))
        }
        Operation::Utf8ToU64 => text(rows, null_every, scenario, false),
        Operation::Utf8ToF64 => text(rows, null_every, scenario, true),
    }
}
