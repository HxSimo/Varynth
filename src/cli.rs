use std::env;
use std::process::ExitCode;

use crate::jsonl::{self, ExtensionCheck};

/// How the caller asked the program to treat the input file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Auto,
    Jsonl,
}

/// Path of the input file and the requested format.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub path: String,
    pub format: Format,
}

/// A usage error. The process should exit with status 2.
#[derive(Debug)]
pub enum UsageError {
    NoFile { program: String },
    TooManyArguments { program: String },
    MissingFormatValue { program: String },
    DuplicateFormat { program: String },
    UnsupportedFormat { program: String, value: String },
    UnknownOption { program: String, option: String },
}

impl UsageError {
    /// Print the error and the expected usage line.
    pub fn report(&self) {
        let program = match self {
            Self::NoFile { program }
            | Self::TooManyArguments { program }
            | Self::MissingFormatValue { program }
            | Self::DuplicateFormat { program }
            | Self::UnsupportedFormat { program, .. }
            | Self::UnknownOption { program, .. } => program,
        };

        match self {
            Self::NoFile { .. } => eprintln!("error: no file provided"),
            Self::TooManyArguments { .. } => eprintln!("error: too many arguments"),
            Self::MissingFormatValue { .. } => eprintln!("error: missing value for --format"),
            Self::DuplicateFormat { .. } => eprintln!("error: --format given more than once"),
            Self::UnsupportedFormat { value, .. } => {
                eprintln!("error: unsupported format '{value}', only jsonl is accepted");
            }
            Self::UnknownOption { option, .. } => eprintln!("error: unknown option '{option}'"),
        }

        eprintln!("usage: {program} <file> [--format jsonl]");
    }
}

/// Read process arguments, validate the input file, and return the exit status.
pub fn run() -> ExitCode {
    let args = match parse(env::args()) {
        Ok(args) => args,
        Err(err) => {
            err.report();
            return ExitCode::from(2);
        }
    };

    let extension = match args.format {
        Format::Auto => ExtensionCheck::Required,
        Format::Jsonl => ExtensionCheck::Skip,
    };

    if let Err(message) = jsonl::validate_jsonl_file(&args.path, extension) {
        eprintln!("error: {message}");
        return ExitCode::from(1);
    }

    match args.format {
        Format::Auto => println!("JSONL detected automatically: {}", args.path),
        Format::Jsonl => println!("JSONL format requested: {}", args.path),
    }
    ExitCode::SUCCESS
}

/// Parse `args`, whose first item is the program name.
pub fn parse<I>(args: I) -> Result<Args, UsageError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let program = args.next().unwrap_or_else(|| "varynth".to_string());

    let mut path = None;
    let mut format = None;

    while let Some(arg) = args.next() {
        if arg == "--format" || arg.starts_with("--format=") {
            let value = format_value(&program, &arg, &mut args)?;
            if format.is_some() {
                return Err(UsageError::DuplicateFormat { program });
            }
            if value != "jsonl" {
                return Err(UsageError::UnsupportedFormat { program, value });
            }
            format = Some(Format::Jsonl);
        } else if arg.starts_with('-') {
            return Err(UsageError::UnknownOption {
                program,
                option: arg,
            });
        } else if arg.is_empty() {
            return Err(UsageError::NoFile { program });
        } else if path.is_some() {
            return Err(UsageError::TooManyArguments { program });
        } else {
            path = Some(arg);
        }
    }

    let Some(path) = path else {
        return Err(UsageError::NoFile { program });
    };

    Ok(Args {
        path,
        format: format.unwrap_or(Format::Auto),
    })
}

/// Read the value of `--format <value>` or `--format=<value>`.
fn format_value<I>(program: &str, arg: &str, args: &mut I) -> Result<String, UsageError>
where
    I: Iterator<Item = String>,
{
    if let Some(value) = arg.strip_prefix("--format=") {
        if value.is_empty() {
            return Err(UsageError::MissingFormatValue {
                program: program.to_string(),
            });
        }
        return Ok(value.to_string());
    }

    match args.next() {
        Some(value) if !value.is_empty() && !value.starts_with('-') => Ok(value),
        _ => Err(UsageError::MissingFormatValue {
            program: program.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Args, UsageError> {
        parse(args.iter().map(|arg| (*arg).to_string()))
    }

    #[test]
    fn rejects_a_missing_path() {
        let err = parse_args(&["varynth"]).unwrap_err();
        assert!(matches!(err, UsageError::NoFile { .. }));
    }

    #[test]
    fn rejects_an_empty_path() {
        let err = parse_args(&["varynth", ""]).unwrap_err();
        assert!(matches!(err, UsageError::NoFile { .. }));
    }

    #[test]
    fn rejects_extra_arguments() {
        let err = parse_args(&["varynth", "a.jsonl", "b.jsonl"]).unwrap_err();
        assert!(matches!(err, UsageError::TooManyArguments { .. }));
    }

    #[test]
    fn accepts_one_path_and_detects_the_format() {
        let args = parse_args(&["varynth", "events.jsonl"]).unwrap();
        assert_eq!(
            args,
            Args {
                path: "events.jsonl".to_string(),
                format: Format::Auto,
            }
        );
    }

    #[test]
    fn accepts_format_after_the_path() {
        let args = parse_args(&["varynth", "data.txt", "--format", "jsonl"]).unwrap();
        assert_eq!(
            args,
            Args {
                path: "data.txt".to_string(),
                format: Format::Jsonl,
            }
        );
    }

    #[test]
    fn accepts_format_before_the_path() {
        let args = parse_args(&["varynth", "--format", "jsonl", "data.txt"]).unwrap();
        assert_eq!(args.path, "data.txt");
        assert_eq!(args.format, Format::Jsonl);
    }

    #[test]
    fn accepts_format_with_an_equals_sign() {
        let args = parse_args(&["varynth", "--format=jsonl", "data.txt"]).unwrap();
        assert_eq!(args.format, Format::Jsonl);
    }

    #[test]
    fn rejects_an_unsupported_format() {
        let err = parse_args(&["varynth", "data.txt", "--format", "csv"]).unwrap_err();
        assert!(matches!(
            err,
            UsageError::UnsupportedFormat { ref value, .. } if value == "csv"
        ));
    }

    #[test]
    fn rejects_a_missing_format_value() {
        let err = parse_args(&["varynth", "data.txt", "--format"]).unwrap_err();
        assert!(matches!(err, UsageError::MissingFormatValue { .. }));
    }

    #[test]
    fn rejects_a_repeated_format() {
        let err = parse_args(&["varynth", "--format", "jsonl", "--format", "jsonl"]).unwrap_err();
        assert!(matches!(err, UsageError::DuplicateFormat { .. }));
    }
}
