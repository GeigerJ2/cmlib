//! Example: Parsing VASP POSCAR files
//!
//! This example demonstrates how to parse POSCAR/CONTCAR files from VASP.
//! Run with: cargo run --example parse_poscar

use cmlib::parse_poscar;

fn main() {
    // Example 1: Basic POSCAR with Direct coordinates
    println!("=== Example 1: Basic POSCAR (Direct coordinates) ===\n");

    let poscar_content = r"Si bulk structure
1.0
5.43 0.0 0.0
0.0 5.43 0.0
0.0 0.0 5.43
Si
2
Direct
0.0 0.0 0.0
0.25 0.25 0.25
";

    match parse_poscar(poscar_content) {
        Ok((_, poscar)) => {
            println!("Comment: {}", poscar.comment);
            println!("Scaling factor: {}", poscar.scaling_factor);
            println!("Elements: {:?}", poscar.element_names);
            println!("Atom counts: {:?}", poscar.element_counts);
            println!("Coordinate type: {:?}", poscar.coordinate_type);
            println!("Number of atoms: {}", poscar.positions.len());

            println!("\nAtomic positions:");
            for (i, pos) in poscar.positions.iter().enumerate() {
                println!("  Atom {}: {} {} {}",
                    i, pos.position.0, pos.position.1, pos.position.2);
            }
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            std::process::exit(1);
        }
    }

    // Example 2: POSCAR with Selective Dynamics
    println!("\n\n=== Example 2: POSCAR with Selective Dynamics ===\n");

    let poscar_selective = r"NaCl with constraints
1.0
5.64 0.0 0.0
0.0 5.64 0.0
0.0 0.0 5.64
Na Cl
1 1
Selective dynamics
Direct
0.0 0.0 0.0 T T T
0.5 0.5 0.5 F F F
";

    match parse_poscar(poscar_selective) {
        Ok((_, poscar)) => {
            println!("Comment: {}", poscar.comment);
            println!("Elements: {:?}", poscar.element_names);
            println!("Selective dynamics: {}", poscar.selective_dynamics);

            println!("\nAtomic positions with constraints:");
            for (i, pos) in poscar.positions.iter().enumerate() {
                let element = &poscar.element_names[i];
                print!("  {} {}: {} {} {}",
                    element, i, pos.position.0, pos.position.1, pos.position.2);

                if let Some((x, y, z)) = pos.selective {
                    let flags = format!("{} {} {}",
                        if x { "T" } else { "F" },
                        if y { "T" } else { "F" },
                        if z { "T" } else { "F" }
                    );
                    print!(" [{}]", flags);
                }
                println!();
            }
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            std::process::exit(1);
        }
    }

    // Example 3: Multi-element POSCAR
    println!("\n\n=== Example 3: Multi-element structure ===\n");

    let poscar_multi = r"SiO2 structure
1.0
4.913 0.0 0.0
0.0 4.913 0.0
0.0 0.0 5.405
Si O
2 4
Cartesian
0.0 0.0 0.0
2.456 2.456 0.0
1.228 1.228 1.351
3.685 3.685 1.351
1.228 3.685 4.054
3.685 1.228 4.054
";

    match parse_poscar(poscar_multi) {
        Ok((_, poscar)) => {
            println!("Comment: {}", poscar.comment);
            println!("Elements: {:?}", poscar.element_names);
            println!("Counts: {:?}", poscar.element_counts);
            println!("Coordinate type: {:?}", poscar.coordinate_type);
            println!("Total atoms: {}", poscar.positions.len());

            // Calculate how many atoms of each element
            let mut atom_idx = 0;
            for (elem_idx, count_str) in poscar.element_counts.iter().enumerate() {
                if let Ok(count) = count_str.parse::<usize>() {
                    println!("\n{} atoms ({}x):", poscar.element_names[elem_idx], count);
                    for i in 0..count {
                        let pos = &poscar.positions[atom_idx + i];
                        println!("  {} {} {}",
                            pos.position.0, pos.position.1, pos.position.2);
                    }
                    atom_idx += count;
                }
            }
        }
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            std::process::exit(1);
        }
    }
}
