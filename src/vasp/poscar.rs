use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::{line_ending, multispace0, not_line_ending, space0},
    combinator::{map, opt},
    multi::many1,
    sequence::{delimited, preceded, tuple},
    IResult,
};

/// Type alias for 3D vector (x, y, z) as strings
pub type Vec3 = (String, String, String);

/// Type alias for selective dynamics flags (x, y, z) as booleans
pub type SelectiveFlags = (bool, bool, bool);

/// Coordinate system type
#[derive(Debug, PartialEq, Clone)]
pub enum CoordinateType {
    Direct,    // Fractional coordinates
    Cartesian, // Cartesian coordinates
}

/// Single atomic position with optional selective dynamics and velocity
#[derive(Debug, PartialEq)]
pub struct AtomicPosition {
    /// Atomic coordinates (x, y, z)
    pub position: Vec3,
    /// Selective dynamics flags (optional: T/F for each coordinate)
    pub selective: Option<SelectiveFlags>,
    /// Velocity vector (optional: for predictor-corrector)
    pub velocity: Option<Vec3>,
}

/// Main POSCAR/CONTCAR structure
#[derive(Debug, PartialEq)]
pub struct Poscar {
    /// Comment/title line (line 1)
    pub comment: String,

    /// Universal scaling factor (line 2)
    pub scaling_factor: String,

    /// Lattice vectors (lines 3-5: 3 rows of 3 floats)
    pub lattice_vectors: [Vec3; 3],

    /// Element names (line 6: space-separated)
    pub element_names: Vec<String>,

    /// Element counts (line 7: space-separated integers)
    pub element_counts: Vec<String>,

    /// Whether selective dynamics is enabled (line 8: optional)
    pub selective_dynamics: bool,

    /// Coordinate type (line 9: Direct/Cartesian)
    pub coordinate_type: CoordinateType,

    /// Atomic positions (lines 10+)
    pub positions: Vec<AtomicPosition>,
}

// ===== Helper Functions (from qe/inp.rs) =====

/// wms remove white space before and after the inner parser
/// It mostly used for line parser
fn wms<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(multispace0, inner, multispace0)
}

/// ws remove white space (different from `wms` will not remove line break) before and after the inner parser
/// It mostly used for identifier parser
fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(space0, inner, space0)
}

// Parse a bare (unquoted) identifier or keyword
// `alphanumeric`, `_`, `(`, `)`, `.`, `-`, `/`, `+`, `*`, `^` in the parsed string.
fn bare_ident(input: &str) -> IResult<&str, String> {
    map(
        take_while1(|c: char| {
            c.is_alphanumeric()
                || c == '_'
                || c == '('
                || c == ')'
                || c == '.'
                || c == '-'
                || c == '/'
                || c == '+'
                || c == '*'
                || c == '^'
        }),
        |s: &str| s.to_string(),
    )(input)
}

// ===== VASP-Specific Parsers =====

/// Parse comment line (line 1): arbitrary text until newline
fn parse_comment_line(input: &str) -> IResult<&str, String> {
    map(
        preceded(multispace0, not_line_ending),
        |s: &str| s.to_string(),
    )(input)
}

/// Parse scaling factor (line 2): single float
fn parse_scaling_factor(input: &str) -> IResult<&str, String> {
    preceded(multispace0, ws(bare_ident))(input)
}

/// Parse a single lattice vector (3 floats)
fn parse_lattice_vector(input: &str) -> IResult<&str, Vec3> {
    tuple((ws(bare_ident), ws(bare_ident), ws(bare_ident)))(input)
}

/// Parse all 3 lattice vectors (lines 3-5)
fn parse_lattice_vectors(input: &str) -> IResult<&str, [Vec3; 3]> {
    let (input, _) = multispace0(input)?;
    let (input, v1) = parse_lattice_vector(input)?;
    let (input, _) = line_ending(input)?;
    let (input, v2) = parse_lattice_vector(input)?;
    let (input, _) = line_ending(input)?;
    let (input, v3) = parse_lattice_vector(input)?;
    Ok((input, [v1, v2, v3]))
}

/// Parse element names (line 6): space-separated element symbols
fn parse_element_names(input: &str) -> IResult<&str, Vec<String>> {
    preceded(multispace0, many1(ws(bare_ident)))(input)
}

/// Parse element counts (line 7): space-separated integers
fn parse_element_counts(input: &str) -> IResult<&str, Vec<String>> {
    preceded(multispace0, many1(ws(bare_ident)))(input)
}

/// Parse optional "Selective dynamics" line (line 8)
/// Returns true if found, false otherwise
fn parse_selective_dynamics_line(input: &str) -> IResult<&str, bool> {
    let (input, result) = opt(preceded(
        multispace0,
        alt((
            tag_no_case("Selective dynamics"),
            tag_no_case("Selective Dynamics"),
        )),
    ))(input)?;
    Ok((input, result.is_some()))
}

/// Parse coordinate type (line 9): Direct/Cartesian/D/C/K (case-insensitive)
fn parse_coordinate_type(input: &str) -> IResult<&str, CoordinateType> {
    let (input, _) = multispace0(input)?;
    alt((
        map(
            alt((
                tag_no_case("Direct"),
                tag_no_case("D"),
            )),
            |_| CoordinateType::Direct,
        ),
        map(
            alt((
                tag_no_case("Cartesian"),
                tag_no_case("Cartesian"),
                tag_no_case("C"),
                tag_no_case("K"),
            )),
            |_| CoordinateType::Cartesian,
        ),
    ))(input)
}

/// Parse selective dynamics flags: T/F T/F T/F
fn parse_selective_flags(input: &str) -> IResult<&str, SelectiveFlags> {
    let parse_bool = || {
        alt((
            map(tag("T"), |_| true),
            map(tag("F"), |_| false),
        ))
    };
    tuple((ws(parse_bool()), ws(parse_bool()), ws(parse_bool())))(input)
}

/// Parse a single position line
/// Format: x y z [T/F T/F T/F if selective_dynamics]
fn parse_position_line(selective_dynamics: bool) -> impl Fn(&str) -> IResult<&str, AtomicPosition> {
    move |input: &str| {
        let (input, position) = parse_lattice_vector(input)?;
        let (input, selective) = if selective_dynamics {
            let (input, flags) = parse_selective_flags(input)?;
            (input, Some(flags))
        } else {
            (input, None)
        };
        Ok((
            input,
            AtomicPosition {
                position,
                selective,
                velocity: None, // Will be filled in later if velocities present
            },
        ))
    }
}

/// Parse all atomic positions
fn parse_positions(
    selective_dynamics: bool,
    num_atoms: usize,
) -> impl Fn(&str) -> IResult<&str, Vec<AtomicPosition>> {
    move |input: &str| {
        let mut positions = Vec::new();
        let mut remaining = input;

        for _ in 0..num_atoms {
            let (input, _) = multispace0(remaining)?;
            let (input, pos) = parse_position_line(selective_dynamics)(input)?;
            positions.push(pos);
            remaining = input;
        }

        Ok((remaining, positions))
    }
}

/// Main parser for POSCAR/CONTCAR files
pub fn parse_poscar(input: &str) -> IResult<&str, Poscar> {
    // 1. Parse comment line
    let (input, comment) = parse_comment_line(input)?;
    let (input, _) = line_ending(input)?;

    // 2. Parse scaling factor
    let (input, scaling_factor) = parse_scaling_factor(input)?;
    let (input, _) = line_ending(input)?;

    // 3. Parse lattice vectors (3 lines)
    let (input, lattice_vectors) = parse_lattice_vectors(input)?;
    let (input, _) = line_ending(input)?;

    // 4. Parse element names
    let (input, element_names) = parse_element_names(input)?;
    let (input, _) = line_ending(input)?;

    // 5. Parse element counts
    let (input, element_counts) = parse_element_counts(input)?;
    let (input, _) = line_ending(input)?;

    // Calculate total number of atoms
    let num_atoms: usize = element_counts
        .iter()
        .filter_map(|s| s.parse::<usize>().ok())
        .sum();

    // 6. Check for optional "Selective dynamics" line
    let (input, selective_dynamics) = parse_selective_dynamics_line(input)?;
    let input = if selective_dynamics {
        let (input, _) = line_ending(input)?;
        input
    } else {
        input
    };

    // 7. Parse coordinate type
    let (input, coordinate_type) = parse_coordinate_type(input)?;
    let (input, _) = line_ending(input)?;

    // 8. Parse atomic positions
    let (input, mut positions) = parse_positions(selective_dynamics, num_atoms)(input)?;

    // 9. Try to parse velocities (optional)
    // Try to parse velocity data if there's more content
    let mut remaining = input;
    let mut velocities = Vec::new();
    let mut success = true;

    for _ in 0..num_atoms {
        match multispace0::<_, nom::error::Error<&str>>(remaining) {
            Ok((inp, _)) => match parse_lattice_vector(inp) {
                Ok((inp, vel)) => {
                    velocities.push(vel);
                    remaining = match opt::<_, _, nom::error::Error<&str>, _>(line_ending)(inp) {
                        Ok((inp, _)) => inp,
                        Err(_) => {
                            success = false;
                            break;
                        }
                    };
                }
                Err(_) => {
                    success = false;
                    break;
                }
            },
            Err(_) => {
                success = false;
                break;
            }
        }
    }

    // Attach velocities to positions if we successfully parsed all of them
    let final_input = if success && velocities.len() == num_atoms {
        for (i, vel) in velocities.into_iter().enumerate() {
            positions[i].velocity = Some(vel);
        }
        remaining
    } else {
        // Consume any trailing whitespace when velocities aren't present
        match multispace0::<_, nom::error::Error<&str>>(input) {
            Ok((inp, _)) => inp,
            Err(_) => input,
        }
    };

    Ok((
        final_input,
        Poscar {
            comment,
            scaling_factor,
            lattice_vectors,
            element_names,
            element_counts,
            selective_dynamics,
            coordinate_type,
            positions,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comment_line() {
        let input = "Si bulk structure";
        let (_, comment) = parse_comment_line(input).unwrap();
        assert_eq!(comment, "Si bulk structure");
    }

    #[test]
    fn test_scaling_factor() {
        let input = "1.0\n";
        let (_, factor) = parse_scaling_factor(input).unwrap();
        assert_eq!(factor, "1.0");
    }

    #[test]
    fn test_lattice_vector() {
        let input = "5.43 0.0 0.0";
        let (_, vec) = parse_lattice_vector(input).unwrap();
        assert_eq!(vec, ("5.43".to_string(), "0.0".to_string(), "0.0".to_string()));
    }

    #[test]
    fn test_lattice_vectors() {
        let input = "5.43 0.0 0.0\n0.0 5.43 0.0\n0.0 0.0 5.43";
        let (_, vecs) = parse_lattice_vectors(input).unwrap();
        assert_eq!(vecs.len(), 3);
        assert_eq!(vecs[0], ("5.43".to_string(), "0.0".to_string(), "0.0".to_string()));
    }

    #[test]
    fn test_element_names() {
        let input = "Si O\n";
        let (_, names) = parse_element_names(input).unwrap();
        assert_eq!(names, vec!["Si", "O"]);
    }

    #[test]
    fn test_element_counts() {
        let input = "4 8\n";
        let (_, counts) = parse_element_counts(input).unwrap();
        assert_eq!(counts, vec!["4", "8"]);
    }

    #[test]
    fn test_coordinate_type_direct() {
        let input = "Direct\n";
        let (_, coord_type) = parse_coordinate_type(input).unwrap();
        assert_eq!(coord_type, CoordinateType::Direct);
    }

    #[test]
    fn test_coordinate_type_cartesian() {
        let input = "Cartesian\n";
        let (_, coord_type) = parse_coordinate_type(input).unwrap();
        assert_eq!(coord_type, CoordinateType::Cartesian);
    }

    #[test]
    fn test_coordinate_type_case_insensitive() {
        let input = "direct\n";
        let (_, coord_type) = parse_coordinate_type(input).unwrap();
        assert_eq!(coord_type, CoordinateType::Direct);

        let input = "CARTESIAN\n";
        let (_, coord_type) = parse_coordinate_type(input).unwrap();
        assert_eq!(coord_type, CoordinateType::Cartesian);
    }

    #[test]
    fn test_coordinate_type_single_letter() {
        let input = "D\n";
        let (_, coord_type) = parse_coordinate_type(input).unwrap();
        assert_eq!(coord_type, CoordinateType::Direct);

        let input = "C\n";
        let (_, coord_type) = parse_coordinate_type(input).unwrap();
        assert_eq!(coord_type, CoordinateType::Cartesian);

        let input = "K\n";
        let (_, coord_type) = parse_coordinate_type(input).unwrap();
        assert_eq!(coord_type, CoordinateType::Cartesian);
    }

    #[test]
    fn test_selective_dynamics_line() {
        let input = "Selective dynamics\n";
        let (_, result) = parse_selective_dynamics_line(input).unwrap();
        assert!(result);

        let input = "Selective Dynamics\n";
        let (_, result) = parse_selective_dynamics_line(input).unwrap();
        assert!(result);

        let input = "Direct\n";
        let (_, result) = parse_selective_dynamics_line(input).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_selective_flags() {
        let input = "T F T";
        let (_, flags) = parse_selective_flags(input).unwrap();
        assert_eq!(flags, (true, false, true));

        let input = "F F F";
        let (_, flags) = parse_selective_flags(input).unwrap();
        assert_eq!(flags, (false, false, false));
    }

    #[test]
    fn test_position_line_basic() {
        let input = "0.0 0.0 0.0";
        let (_, pos) = parse_position_line(false)(input).unwrap();
        assert_eq!(pos.position, ("0.0".to_string(), "0.0".to_string(), "0.0".to_string()));
        assert_eq!(pos.selective, None);
    }

    #[test]
    fn test_position_line_with_selective() {
        let input = "0.0 0.0 0.0 T T F";
        let (_, pos) = parse_position_line(true)(input).unwrap();
        assert_eq!(pos.position, ("0.0".to_string(), "0.0".to_string(), "0.0".to_string()));
        assert_eq!(pos.selective, Some((true, true, false)));
    }

    #[test]
    fn test_negative_coordinates() {
        let input = "-0.5 -0.25 0.75";
        let (_, pos) = parse_position_line(false)(input).unwrap();
        assert_eq!(pos.position, ("-0.5".to_string(), "-0.25".to_string(), "0.75".to_string()));
    }

    #[test]
    fn test_scientific_notation() {
        let input = "1.0e-5 2.5E+3 -1.2e-10";
        let (_, pos) = parse_position_line(false)(input).unwrap();
        assert_eq!(pos.position, ("1.0e-5".to_string(), "2.5E+3".to_string(), "-1.2e-10".to_string()));
    }

    #[test]
    fn test_minimal_poscar_direct() {
        let input = r"Si bulk
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
        let (remaining, poscar) = parse_poscar(input).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(poscar.comment, "Si bulk");
        assert_eq!(poscar.scaling_factor, "1.0");
        assert_eq!(poscar.element_names, vec!["Si"]);
        assert_eq!(poscar.element_counts, vec!["2"]);
        assert!(!poscar.selective_dynamics);
        assert_eq!(poscar.coordinate_type, CoordinateType::Direct);
        assert_eq!(poscar.positions.len(), 2);
        assert_eq!(poscar.positions[0].position, ("0.0".to_string(), "0.0".to_string(), "0.0".to_string()));
        assert_eq!(poscar.positions[1].position, ("0.25".to_string(), "0.25".to_string(), "0.25".to_string()));
    }

    #[test]
    fn test_minimal_poscar_cartesian() {
        let input = r"Si bulk
1.0
5.43 0.0 0.0
0.0 5.43 0.0
0.0 0.0 5.43
Si
2
Cartesian
0.0 0.0 0.0
1.35 1.35 1.35
";
        let (_, poscar) = parse_poscar(input).unwrap();
        assert_eq!(poscar.coordinate_type, CoordinateType::Cartesian);
        assert_eq!(poscar.positions.len(), 2);
    }

    #[test]
    fn test_poscar_with_selective_dynamics() {
        let input = r"NaCl with constraints
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
        let (_, poscar) = parse_poscar(input).unwrap();
        assert_eq!(poscar.comment, "NaCl with constraints");
        assert!(poscar.selective_dynamics);
        assert_eq!(poscar.element_names, vec!["Na", "Cl"]);
        assert_eq!(poscar.element_counts, vec!["1", "1"]);
        assert_eq!(poscar.positions.len(), 2);
        assert_eq!(poscar.positions[0].selective, Some((true, true, true)));
        assert_eq!(poscar.positions[1].selective, Some((false, false, false)));
    }

    #[test]
    fn test_poscar_with_velocities() {
        let input = r"Si with velocities
1.0
5.43 0.0 0.0
0.0 5.43 0.0
0.0 0.0 5.43
Si
2
Cartesian
0.0 0.0 0.0
1.35 1.35 1.35
0.1 0.0 0.0
0.0 0.1 0.1
";
        let (_, poscar) = parse_poscar(input).unwrap();
        assert_eq!(poscar.positions.len(), 2);
        assert!(poscar.positions[0].velocity.is_some());
        assert_eq!(
            poscar.positions[0].velocity,
            Some(("0.1".to_string(), "0.0".to_string(), "0.0".to_string()))
        );
        assert!(poscar.positions[1].velocity.is_some());
        assert_eq!(
            poscar.positions[1].velocity,
            Some(("0.0".to_string(), "0.1".to_string(), "0.1".to_string()))
        );
    }

    #[test]
    fn test_poscar_multi_element() {
        let input = r"SiO2 structure
1.0
4.913 0.0 0.0
0.0 4.913 0.0
0.0 0.0 5.405
Si O
4 8
Direct
0.0 0.0 0.0
0.5 0.5 0.0
0.5 0.0 0.5
0.0 0.5 0.5
0.25 0.25 0.25
0.75 0.75 0.25
0.75 0.25 0.75
0.25 0.75 0.75
0.75 0.75 0.75
0.25 0.25 0.75
0.25 0.75 0.25
0.75 0.25 0.25
";
        let (_, poscar) = parse_poscar(input).unwrap();
        assert_eq!(poscar.element_names, vec!["Si", "O"]);
        assert_eq!(poscar.element_counts, vec!["4", "8"]);
        assert_eq!(poscar.positions.len(), 12); // 4 Si + 8 O = 12 atoms
    }

    #[test]
    fn test_poscar_complete_with_all_features() {
        let input = r"Complete test structure
2.5
3.0 0.0 0.0
0.0 3.0 0.0
0.0 0.0 3.0
H He
1 1
Selective Dynamics
D
0.0 0.0 0.0 T F T
0.5 0.5 0.5 F T F
1.0 0.0 0.0
0.0 1.0 0.0
";
        let (_, poscar) = parse_poscar(input).unwrap();
        assert_eq!(poscar.comment, "Complete test structure");
        assert_eq!(poscar.scaling_factor, "2.5");
        assert_eq!(poscar.element_names, vec!["H", "He"]);
        assert_eq!(poscar.element_counts, vec!["1", "1"]);
        assert!(poscar.selective_dynamics);
        assert_eq!(poscar.coordinate_type, CoordinateType::Direct);
        assert_eq!(poscar.positions.len(), 2);
        assert_eq!(poscar.positions[0].selective, Some((true, false, true)));
        assert_eq!(poscar.positions[1].selective, Some((false, true, false)));
        assert!(poscar.positions[0].velocity.is_some());
        assert!(poscar.positions[1].velocity.is_some());
    }
}
