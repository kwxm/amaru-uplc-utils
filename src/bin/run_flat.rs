//! Decode a flat-encoded UPLC program and evaluate it with `amaru-uplc`.
//!
//! Usage: run-flat <FILE.flat> [v1|v2|v3]

use std::{env, fs};

use amaru_kernel::ProtocolVersion;
use amaru_uplc::{arena::Arena, binder::DeBruijn, flat};
use amaru_uplc_utils::{die, evaluate_and_report, parse_version};

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: run-flat <FILE.flat> [v1|v2|v3]");
        std::process::exit(2);
    };
    let version = parse_version(args.next().as_deref());

    if path.ends_with(".uplc") {
        die(format!(
            "run-flat expects a flat-encoded program; {path} is textual"
        ));
    }

    let bytes = fs::read(&path).unwrap_or_else(|error| die(format!("cannot read {path}: {error}")));
    let arena = Arena::new();
    let (program, trailing) =
        flat::decode::<DeBruijn>(&arena, &bytes, version, ProtocolVersion::default())
            .unwrap_or_else(|error| die(format!("cannot decode {path}: {error}")));
    if trailing != 0 {
        die(format!(
            "{path} has {trailing} trailing bytes after the program"
        ));
    }

    evaluate_and_report(&arena, program, version);
}
