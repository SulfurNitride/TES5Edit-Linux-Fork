//! xedit_transpiler - Converts Delphi wb* definition files to Rust code.
//!
//! Usage: xedit_transpiler <path-to-wbDefinitionsXXX.pas>
//!
//! Reads a Delphi .pas file containing wbRecord() definitions and generates
//! Rust code that constructs the equivalent RecordDef/SubrecordDef/FieldDef tree.

mod codegen;
mod parser;

use anyhow::{Context, Result};
use std::env;
use std::fs;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: xedit_transpiler <path-to-wbDefinitions.pas>");
        eprintln!();
        eprintln!("Reads a Delphi .pas file and outputs Rust code that constructs");
        eprintln!("RecordDef values matching the wb* definitions found in the file.");
        std::process::exit(1);
    }

    let path = &args[1];
    eprintln!("Reading: {}", path);

    let source = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path))?;

    eprintln!("File size: {} bytes, {} lines", source.len(), source.lines().count());

    // Parse all wbRecord definitions
    let records = parser::parse_records(&source);
    eprintln!("Found {} wbRecord definitions", records.len());

    // Print parse statistics to stderr
    parser::print_stats(&records);

    // Print a summary of each record
    eprintln!("=== Records ===");
    for rec in &records {
        eprintln!(
            "  {} ({}) - {} members (line {})",
            rec.signature,
            rec.name,
            rec.members.len(),
            rec.line_number,
        );
    }
    eprintln!();

    // Generate Rust code to stdout
    let rust_code = codegen::generate_all(&records);
    println!("{}", rust_code);

    eprintln!("Done. Generated {} bytes of Rust code.", rust_code.len());

    Ok(())
}
