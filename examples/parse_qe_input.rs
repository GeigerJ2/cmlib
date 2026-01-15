//! Example: Parsing Quantum ESPRESSO pw.x input files
//!
//! This example demonstrates how to parse pw.x input files from Quantum ESPRESSO.
//! Run with: cargo run --example parse_qe_input

use cmlib::parse_pw_input;

fn main() {
    // Example 1: Simple H2 molecule SCF calculation
    println!("=== Example 1: H2 molecule SCF calculation ===\n");

    let input = r"
&control
    calculation = 'scf',
    prefix = 'h2',
    pseudo_dir = './pseudo/',
    outdir = './tmp/'
/
&system
    ibrav = 1,
    celldm(1) = 10.0,
    nat = 2,
    ntyp = 1,
    ecutwfc = 25.0
/
&electrons
    conv_thr = 1.0d-8
/
ATOMIC_SPECIES
 H 1.008 H.pbe-rrkjus_psl.1.0.0.UPF
ATOMIC_POSITIONS angstrom
 H  0.00 0.00 -0.37
 H  0.00 0.00  0.37
K_POINTS gamma
";

    match parse_pw_input(input) {
        Ok((remaining, qe_input)) => {
            println!("Successfully parsed QE input!");
            println!("Remaining unparsed: {:?}", remaining.trim());
            println!("\nParsed structure:");
            println!("{:#?}", qe_input);
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            std::process::exit(1);
        }
    }

    // Example 2: Silicon bulk with automatic k-points
    println!("\n\n=== Example 2: Silicon bulk with automatic k-points ===\n");

    let input_si = r"
&control
    calculation = 'scf',
    prefix = 'silicon',
    pseudo_dir = './pseudo/',
    outdir = './tmp/'
/
&system
    ibrav = 2,
    celldm(1) = 10.2,
    nat = 2,
    ntyp = 1,
    ecutwfc = 30.0
/
&electrons
/
ATOMIC_SPECIES
 Si 28.086 Si.pbe-n-rrkjus_psl.1.0.0.UPF
ATOMIC_POSITIONS crystal
 Si 0.00 0.00 0.00
 Si 0.25 0.25 0.25
K_POINTS automatic
 4 4 4 1 1 1
";

    match parse_pw_input(input_si) {
        Ok((remaining, qe_input)) => {
            println!("Successfully parsed QE input!");
            println!("Remaining unparsed: {:?}", remaining.trim());
            println!("\nParsed structure:");
            println!("{:#?}", qe_input);
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            std::process::exit(1);
        }
    }

    // Example 3: With explicit cell parameters
    println!("\n\n=== Example 3: With explicit CELL_PARAMETERS ===\n");

    let input_cell = r"
&control
    calculation = 'relax',
    prefix = 'graphene'
/
&system
    ibrav = 0,
    nat = 2,
    ntyp = 1,
    ecutwfc = 40.0
/
&electrons
/
&ions
/
ATOMIC_SPECIES
 C 12.011 C.pbe-n-kjpaw_psl.1.0.0.UPF
CELL_PARAMETERS angstrom
 2.46  0.00  0.00
 1.23  2.13  0.00
 0.00  0.00 15.00
ATOMIC_POSITIONS crystal
 C 0.0 0.0 0.5
 C 0.333333 0.666667 0.5
K_POINTS automatic
 12 12 1 0 0 0
";

    match parse_pw_input(input_cell) {
        Ok((remaining, qe_input)) => {
            println!("Successfully parsed QE input!");
            println!("Remaining unparsed: {:?}", remaining.trim());
            println!("\nParsed structure:");
            println!("{:#?}", qe_input);
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            std::process::exit(1);
        }
    }
}
