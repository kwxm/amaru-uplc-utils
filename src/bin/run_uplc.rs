//! Parse a textual UPLC program and evaluate it with `amaru-uplc`.
//!
//! Usage: run-uplc <FILE.uplc> [v1|v2|v3]
//!
//! `amaru-uplc` has no textual printer, so the result is printed flat-encoded.

use std::{env, fs};

use amaru_kernel::ProtocolVersion;
use amaru_uplc::{arena::Arena, syn::parse_program};
use amaru_uplc_utils::{die, evaluate_and_report, parse_version};

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: run-uplc <FILE.uplc> [v1|v2|v3]");
        std::process::exit(2);
    };
    let version = parse_version(args.next().as_deref());

    if path.ends_with(".flat") {
        die(format!(
            "run-uplc expects a textual program; {path} is flat-encoded"
        ));
    }

    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| die(format!("cannot read {path}: {error}")));
    let arena = Arena::new();
    let source_ref: &mut String = arena.alloc(source);
    let program = parse_program(&arena, source_ref.as_str(), ProtocolVersion::default())
        .into_result()
        .unwrap_or_else(|errors| die(format!("cannot parse {path}: {errors:?}")));

    evaluate_and_report(&arena, program, version);
}
