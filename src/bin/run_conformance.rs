//! Run the test cases produced by `generate-conformance-tests` through
//! `amaru-uplc` and compare the outcome with the recorded expectations.
//!
//! Usage: run-conformance <DIR> [--plutus-version v1|v2|v3] [--no-budgets]
//!                           [--filter TEXT] [--verbose]
//!
//! For each case directory containing `<case>.flat` (or `<case>.uplc`) the
//! program is decoded/parsed, evaluated, and the result compared with
//! `<case>.flat.expected` (or `<case>.uplc.expected`). Budgets are compared
//! with `<case>.budget.expected` unless `--no-budgets` is given.

use std::{
    env,
    error::Error,
    fs, mem,
    path::{Path, PathBuf},
    process,
};

use amaru_kernel::{PlutusVersion, ProtocolVersion};
use amaru_uplc::{arena::Arena, binder::DeBruijn, flat, syn::parse_program};

const FAILURE_MARKERS: [&str; 3] = ["evaluation failure", "out of budget", "parse/decode error"];

#[derive(Clone, Copy)]
enum Format {
    Flat,
    Text,
}

struct Options {
    root: PathBuf,
    version: PlutusVersion,
    check_budgets: bool,
    filter: Option<String>,
    verbose: bool,
}

fn main() {
    let opts = match parse_args() {
        Ok(opts) => opts,
        Err(message) => {
            eprintln!("error: {message}");
            process::exit(2);
        }
    };

    let mut inputs = Vec::new();
    if let Err(error) = collect_inputs(&opts.root, &mut inputs) {
        eprintln!("error: cannot read {}: {error}", opts.root.display());
        process::exit(2);
    }
    inputs.sort();

    let mut checked = 0;
    let mut failed = 0;
    for input in inputs {
        let label = input
            .strip_prefix(&opts.root)
            .unwrap_or(&input)
            .display()
            .to_string();
        if let Some(filter) = &opts.filter {
            if !label.contains(filter) {
                continue;
            }
        }
        checked += 1;
        match check_case(&opts, &input) {
            Ok(()) => {
                if opts.verbose {
                    println!("ok   {label}");
                }
            }
            Err(message) => {
                failed += 1;
                println!("FAIL {label}: {message}");
            }
        }
    }

    println!(
        "checked {checked} cases: {} passed, {failed} failed",
        checked - failed
    );
    if failed > 0 {
        process::exit(1);
    }
}

fn parse_args() -> Result<Options, Box<dyn Error>> {
    let mut root = None;
    let mut version = PlutusVersion::V3;
    let mut check_budgets = true;
    let mut filter = None;
    let mut verbose = false;
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--plutus-version" => {
                let value = args.next().ok_or("--plutus-version needs a value")?;
                version = match value.as_str() {
                    "v3" => PlutusVersion::V3,
                    "v2" => PlutusVersion::V2,
                    "v1" => PlutusVersion::V1,
                    other => return Err(format!("unknown Plutus version '{other}'").into()),
                };
            }
            "--no-budgets" => check_budgets = false,
            "--filter" => filter = Some(args.next().ok_or("--filter needs a value")?),
            "--verbose" => verbose = true,
            "-h" | "--help" => {
                println!(
                    "usage: run-conformance <DIR> [--plutus-version v1|v2|v3] [--no-budgets] [--filter TEXT] [--verbose]"
                );
                process::exit(0);
            }
            other if root.is_none() => root = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument '{other}'").into()),
        }
    }

    Ok(Options {
        root: root.ok_or("usage: run-conformance <DIR> [options]")?,
        version,
        check_budgets,
        filter,
        verbose,
    })
}

fn collect_inputs(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_inputs(&path, out)?;
        } else if format_of(&path).is_some() {
            out.push(path);
        }
    }
    Ok(())
}

fn format_of(path: &Path) -> Option<Format> {
    match path.extension().and_then(|extension| extension.to_str())? {
        "flat" => Some(Format::Flat),
        "uplc" => Some(Format::Text),
        _ => None,
    }
}

fn sibling(input: &Path, suffix: &str) -> PathBuf {
    let name = input
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    input.with_file_name(format!("{name}{suffix}"))
}

fn budget_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|stem| stem.to_string_lossy())
        .unwrap_or_default();
    input.with_file_name(format!("{stem}.budget.expected"))
}

fn check_case(opts: &Options, input: &Path) -> Result<(), String> {
    let arena = Arena::new();
    let protocol = ProtocolVersion::default();
    let format = format_of(input).ok_or_else(|| "not a case input".to_string())?;

    let program = load_program(&arena, format, input, opts.version, protocol)?;

    let mut result = program.eval_version(&arena, opts.version);
    let info = mem::take(&mut result.info);
    let actual = result.into_term();

    let expected_path = sibling(input, ".expected");
    let expected_bytes = fs::read(&expected_path)
        .map_err(|error| format!("cannot read {}: {error}", expected_path.display()))?;
    let expected_failure = FAILURE_MARKERS
        .iter()
        .any(|marker| String::from_utf8_lossy(&expected_bytes).trim() == *marker);

    match (actual, expected_failure) {
        (Err(_), true) => {}
        (Err(error), false) => {
            return Err(format!(
                "expected a successful result, but evaluation failed: {error}"
            ));
        }
        (Ok(_), true) => {
            return Err("expected evaluation to fail, but it succeeded".to_string());
        }
        (Ok(actual_term), false) => {
            let expected =
                load_program_from_bytes(&arena, format, &expected_bytes, opts.version, protocol)
                    .map_err(|error| format!("cannot read the expected result: {error}"))?;
            if expected.term != actual_term {
                return Err("the result differs from the .expected file".to_string());
            }
        }
    }

    if opts.check_budgets && !expected_failure {
        let path = budget_path(input);
        let expected = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if !FAILURE_MARKERS
            .iter()
            .any(|marker| expected.trim() == *marker)
        {
            let actual_budget = format!(
                "({{cpu: {}\n| mem: {}}})",
                info.consumed_budget.cpu, info.consumed_budget.mem
            );
            if actual_budget != expected.trim() {
                return Err(format!(
                    "budget differs: expected {}, got {actual_budget}",
                    expected.trim()
                ));
            }
        }
    }

    Ok(())
}

fn load_program<'a>(
    arena: &'a Arena,
    format: Format,
    path: &Path,
    version: PlutusVersion,
    protocol: ProtocolVersion,
) -> Result<&'a amaru_uplc::program::Program<'a, DeBruijn>, String> {
    match format {
        Format::Flat => {
            let bytes = fs::read(path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let (program, trailing) = flat::decode::<DeBruijn>(arena, &bytes, version, protocol)
                .map_err(|error| format!("cannot decode {}: {error}", path.display()))?;
            if trailing != 0 {
                return Err(format!("{} has {trailing} trailing bytes", path.display()));
            }
            Ok(program)
        }
        Format::Text => {
            let source = fs::read_to_string(path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let source_ref: &'a mut String = arena.alloc(source);
            let source: &'a str = source_ref.as_str();
            parse_program(arena, source, protocol)
                .into_result()
                .map_err(|errors| format!("cannot parse {}: {errors:?}", path.display()))
        }
    }
}

fn load_program_from_bytes<'a>(
    arena: &'a Arena,
    format: Format,
    bytes: &'a [u8],
    version: PlutusVersion,
    protocol: ProtocolVersion,
) -> Result<&'a amaru_uplc::program::Program<'a, DeBruijn>, String> {
    match format {
        Format::Flat => {
            let (program, trailing) = flat::decode::<DeBruijn>(arena, bytes, version, protocol)
                .map_err(|error| format!("cannot decode the expected result: {error}"))?;
            if trailing != 0 {
                return Err(format!("the expected result has {trailing} trailing bytes"));
            }
            Ok(program)
        }
        Format::Text => {
            let source = std::str::from_utf8(bytes)
                .map_err(|error| format!("the expected result is not valid UTF-8: {error}"))?;
            parse_program(arena, source, protocol)
                .into_result()
                .map_err(|errors| format!("cannot parse the expected result: {errors:?}"))
        }
    }
}
