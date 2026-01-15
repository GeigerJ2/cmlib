# cmlib

A Rust library for parsing computational materials science file formats.

## Features

Currently supports parsing:

- **VASP POSCAR/CONTCAR files** - Crystal structure files from VASP (Vienna Ab initio Simulation Package)
  - Direct and Cartesian coordinates
  - Selective dynamics
  - Velocities (predictor-corrector MD)

- **Quantum ESPRESSO input files** - pw.x input files

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
cmlib = { path = "../cmlib" }  # Update with actual path or version when published
```

### Parsing VASP POSCAR files

```rust
use cmlib::parse_poscar;

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
        println!("Elements: {:?}", poscar.element_names);
        println!("Atoms: {}", poscar.positions.len());
    }
    Err(e) => eprintln!("Parse error: {:?}", e),
}
```

### Parsing Quantum ESPRESSO input files

```rust
use cmlib::parse_pw_input;

let input = std::fs::read_to_string("pw.in").unwrap();
match parse_pw_input(&input) {
    Ok((_, qe_input)) => {
        // Process QE input
    }
    Err(e) => eprintln!("Parse error: {:?}", e),
}
```

## Examples

Run the examples to see the parser in action:

```bash
# VASP POSCAR parsing examples
cargo run --example parse_poscar
```

## Development

Built using the [nom](https://github.com/rust-bakery/nom) parser combinator library.

### Running tests

```bash
cargo test
```

### Building documentation

```bash
cargo doc --open
```

## License

(Add your license here)

## Contributing

(Add contributing guidelines if this is a collaborative project)
