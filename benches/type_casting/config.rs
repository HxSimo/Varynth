use std::str::FromStr;

use arrow_schema::DataType;

pub const USAGE: &str = "cargo bench --bench type-casting-experiment -- [--rows 1000000,10000000,50000000] [--warmups 3] [--repetitions 10] [--null-every 0] [--operations u16_to_u32,utf8_to_f64] [--scenarios mixed,small,large] [--runs 1]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    U16ToU32,
    U32ToU64,
    U16ToU64,
    U64ToUtf8,
    Utf8ToU64,
    F64ToUtf8,
    Utf8ToF64,
}

impl Operation {
    pub const ALL: [Self; 7] = [
        Self::U16ToU32,
        Self::U32ToU64,
        Self::U16ToU64,
        Self::U64ToUtf8,
        Self::Utf8ToU64,
        Self::F64ToUtf8,
        Self::Utf8ToF64,
    ];

    pub fn label(self) -> &'static str {
        self.spec().0
    }

    pub fn target(self) -> DataType {
        self.spec().1
    }

    fn spec(self) -> (&'static str, DataType) {
        match self {
            Self::U16ToU32 => ("u16_to_u32", DataType::UInt32),
            Self::U32ToU64 => ("u32_to_u64", DataType::UInt64),
            Self::U16ToU64 => ("u16_to_u64", DataType::UInt64),
            Self::U64ToUtf8 => ("u64_to_utf8", DataType::Utf8),
            Self::Utf8ToU64 => ("utf8_to_u64", DataType::UInt64),
            Self::F64ToUtf8 => ("f64_to_utf8", DataType::Utf8),
            Self::Utf8ToF64 => ("utf8_to_f64", DataType::Float64),
        }
    }
}

impl FromStr for Operation {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|op| op.label() == text)
            .ok_or_else(|| format!("unknown operation: {text}"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Mixed,
    Small,
    Large,
}

impl Scenario {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mixed => "mixed",
            Self::Small => "small",
            Self::Large => "large",
        }
    }
}

impl FromStr for Scenario {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "mixed" => Ok(Self::Mixed),
            "small" => Ok(Self::Small),
            "large" => Ok(Self::Large),
            _ => Err(format!("unknown scenario: {text}")),
        }
    }
}

#[derive(Debug)]
pub struct Args {
    pub rows: Vec<usize>,
    pub warmups: usize,
    pub repetitions: usize,
    pub null_every: usize,
    pub operations: Vec<Operation>,
    pub scenarios: Vec<Scenario>,
    pub runs: usize,
}

impl Args {
    pub fn parse(items: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut out = Self {
            // A bounded default; larger working sets remain explicitly selectable.
            rows: vec![1_000_000],
            warmups: 3,
            repetitions: 10,
            null_every: 0,
            operations: Operation::ALL.to_vec(),
            scenarios: vec![Scenario::Mixed],
            runs: 1,
        };
        let mut items = items.into_iter();
        while let Some(flag) = items.next() {
            let mut value = || {
                items
                    .next()
                    .ok_or_else(|| format!("missing value for {flag}"))
            };
            match flag.as_str() {
                // Cargo appends this flag even for harness=false.
                "--bench" => {}
                "--rows" => out.rows = parse_list(&value()?)?,
                "--warmups" => out.warmups = parse_count(&flag, &value()?)?,
                "--repetitions" => out.repetitions = parse_count(&flag, &value()?)?,
                "--null-every" => out.null_every = parse_count(&flag, &value()?)?,
                "--operations" => out.operations = parse_list(&value()?)?,
                "--scenarios" => out.scenarios = parse_list(&value()?)?,
                "--runs" => out.runs = parse_count(&flag, &value()?)?,
                _ => return Err(format!("unknown argument: {flag}")),
            }
        }
        if out.rows.contains(&0) || out.repetitions == 0 || out.runs == 0 {
            return Err("rows, repetitions and runs must be positive".into());
        }
        out.warmups
            .checked_add(out.repetitions)
            .ok_or("warmups + repetitions overflow")?;
        Ok(out)
    }
}

fn parse_count(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("invalid value for {flag}: {value}"))
}

fn parse_list<T>(value: &str) -> Result<Vec<T>, String>
where
    T: FromStr + PartialEq,
    T::Err: std::fmt::Display,
{
    let mut result = Vec::new();
    for item in value.split(',') {
        let parsed = item
            .parse::<T>()
            .map_err(|error| format!("invalid list item {item:?}: {error}"))?;
        if result.contains(&parsed) {
            return Err(format!("duplicate list item: {item}"));
        }
        result.push(parsed);
    }
    Ok(result)
}
