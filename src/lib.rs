//! Shared helpers for the `amaru-uplc-utils` programs.

use std::{fmt::Write as _, mem, process};

use amaru_kernel::PlutusVersion;
use amaru_uplc::{arena::Arena, binder::DeBruijn, flat, program::Program};

/// Read an optional Plutus version from the command line, exiting if it is not
/// one of `v1`, `v2` or `v3`.
pub fn parse_version(arg: Option<&str>) -> PlutusVersion {
    match arg {
        None | Some("v3") => PlutusVersion::V3,
        Some("v2") => PlutusVersion::V2,
        Some("v1") => PlutusVersion::V1,
        Some(other) => die(format!(
            "unknown Plutus version '{other}' (expected v1, v2 or v3)"
        )),
    }
}

/// Report a fatal problem and exit with status 2.
pub fn die(message: String) -> ! {
    eprintln!("error: {message}");
    process::exit(2);
}

/// Encode bytes as lowercase hexadecimal.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Print the program version, the consumed budget and the result, re-encoding
/// the result as flat. Exits with status 1 if evaluation fails.
pub fn evaluate_and_report<'a>(
    arena: &'a Arena,
    program: &'a Program<'a, DeBruijn>,
    version: PlutusVersion,
) {
    println!(
        "program version: {}.{}.{}",
        program.version.major, program.version.minor, program.version.patch
    );

    let mut result = program.eval_version(arena, version);
    let info = mem::take(&mut result.info);
    println!(
        "budget: ({{cpu: {}\n| mem: {}}})",
        info.consumed_budget.cpu, info.consumed_budget.mem
    );

    match result.into_term() {
        Ok(term) => {
            let result_term = Program::new(arena, program.version, term);
            let encoded = flat::encode(result_term)
                .unwrap_or_else(|error| die(format!("cannot encode the result: {error}")));
            println!("evaluation: success");
            println!("result (flat): {}", hex(&encoded));
        }
        Err(error) => {
            println!("evaluation: failure: {error}");
            process::exit(1);
        }
    }
}
