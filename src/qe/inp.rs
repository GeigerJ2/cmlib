use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while, take_while1},
    character::complete::{line_ending, multispace0, space0},
    combinator::{eof, fail, map, opt, peek},
    error::context,
    multi::{many1, many_m_n, many_till},
    sequence::{delimited, preceded, tuple},
    IResult,
};

pub type KeyValPair = (String, String);

#[derive(Debug, PartialEq)]
struct AtomicSpecie {
    // label of the atom
    label: String,
    // mass of the atomic species
    mass: String,
    // file containing PP for the species
    pseudo: String,
}

#[derive(Debug, PartialEq)]
struct AtomicPosition {
    // label of the atom
    label: String,
    // atomic position (x, y, z)
    position: Vec<String>,
    // if_pos
    if_pos: Option<(String, String, String)>,
}

type LatticeVector = (String, String, String);
type Kxyzw = (String, String, String, String);

#[derive(Debug, PartialEq)]
enum KPoints {
    Gamma,
    Automatic {
        mesh: (String, String, String),
        offset: (String, String, String),
    },
    Kxyzw {
        // tpiba | crystal | tpiba_b | crystal_b | tpiba_c | crystal_c
        typ: String,

        // number of kxyzw
        nks: String,

        // k_x, k_y, k_z and weight
        lst_xyzw: Vec<Kxyzw>,
    },
}

/// Block type for the block container of the input
#[derive(Debug, PartialEq)]
enum Block {
    // Namelist of QE input is a named list of key/value pairs
    Namelist {
        name: String,
        lst: Vec<KeyValPair>,
    },

    // ATOMIC_SPECIES block has syntax:
    // X Mass_X PseudoPot_X
    // for every element of the structure
    AtomicSpecies(Vec<AtomicSpecie>),

    // ATOMIC_POSITIONS bolck has format:
    //
    // ATOMIC_POSITIONS { alat | bohr | angstrom | crystal | crystal_sg }
    // X(1)  	 x(1)  	 y(1)  	 z(1)  	{ 	 if_pos(1)(1)  	 if_pos(2)(1)  	 if_pos(3)(1)  	}
    // X(2)  	 x(2)  	 y(2)  	 z(2)  	{ 	 if_pos(1)(2)  	 if_pos(2)(2)  	 if_pos(3)(2)  	}
    // . . .
    AtomicPositions {
        // `typ` can be one of:
        // alat | bohr | angstrom | crystal | crystal_sg for how position is defined
        typ: Option<String>,

        // vec of postions
        lst: Vec<AtomicPosition>,
    },

    // CELL_PARAMETERS block has syntax:
    //
    // CELL_PARAMETERS { alat | bohr | angstrom }
    //  v1(1)  	 v1(2)  	 v1(3)
    //  v2(1)  	 v2(2)  	 v2(3)
    //  v3(1)  	 v3(2)  	 v3(3)
    CellParameters {
        // `typ` can be one of { alat | bohr | angstrom }
        typ: Option<String>,

        // vec of lattice constant
        vecs: Vec<LatticeVector>,
    },

    // K_POINTS block
    KPointsCard(KPoints),
}

#[derive(Debug, PartialEq)]
pub struct QEInput {
    blocks: Vec<Block>,
}

/// wms remove white space before and after the inner parser
fn wms<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(multispace0, inner, multispace0)
}

/// ws remove white space (different from `wms` will not remove line break) before and after the inner parser
fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(space0, inner, space0)
}

// Parse a bare (unquoted) identifier or keyword (e.g. calculation, prefix)
// `alphanumeric`, `_`, `(`, `)`, `.`, `-`, `/` in the parsed string.
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

// Parse a single-quoted string (e.g. 'pseudo/')
fn single_quoted_string(input: &str) -> IResult<&str, String> {
    delimited(
        tag("'"),
        map(take_until("'"), |s: &str| s.to_string()),
        tag("'"),
    )(input)
}

/// Parse a double-quoted string (e.g. "pseudo/")
/// While using double quotes is not encouraged.
fn double_quoted_string(input: &str) -> IResult<&str, String> {
    delimited(
        tag("\""),
        map(take_until("\""), |s: &str| s.to_string()),
        tag("\""),
    )(input)
}

/// Parse inner ``curly_braces`` string (e.g. "{alat}")
fn curly_braces_string(input: &str) -> IResult<&str, String> {
    delimited(
        tag("{"),
        map(take_until("}"), |s: &str| s.to_string()),
        tag("}"),
    )(input)
}

/// Parse a maybe comment key/value pair starting with `!`
fn parse_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = opt(preceded(tag("!"), take_while(|c| c != '\n')))(input)?;
    Ok((input, ()))
}

/// Parse key value pairs in a namelist, e.g. "calculation = 'scf'".
/// The terminate comma in the end of each pair is optional
fn parse_kv(input: &str) -> IResult<&str, KeyValPair> {
    let (input, (k, _, v, _maybe_comma)) = tuple((
        wms(bare_ident),
        wms(tag("=")),
        wms(alt((
            single_quoted_string,
            map(double_quoted_string, |s| {
                eprintln!("Waring: in {input}, double quotes used, use single quotes instead");
                s
            }),
            bare_ident,
            map(tag("''"), |_| String::new()),
            map(tag("\"\""), |_| String::new()),
        ))),
        opt(wms(tag(","))), // optional trailing comma
    ))(input)?;

    Ok((input, (k, v)))
}

fn parse_namelist(input: &str) -> IResult<&str, Block> {
    // Expect namelist start with &
    let (input, _) = wms(tag("&"))(input)?;
    let (input, name) = wms(bare_ident)(input)?;

    // parse key/value pairs until slash
    let (input, (items, _slash)) = many_till(
        wms(alt((map(parse_kv, Some), map(parse_comment, |()| None)))),
        wms(tag("/")),
    )(input)?;

    let kv_lst = items.into_iter().flatten().collect();

    Ok((input, Block::Namelist { name, lst: kv_lst }))
}

fn parse_species_line(input: &str) -> IResult<&str, AtomicSpecie> {
    let (input, label) = wms(bare_ident)(input)?;
    let (input, mass) = wms(bare_ident)(input)?;
    let (input, pseudo) = wms(bare_ident)(input)?;

    Ok((
        input,
        AtomicSpecie {
            label,
            mass,
            pseudo,
        },
    ))
}

fn parse_atomic_species(input: &str) -> IResult<&str, Block> {
    let (input, _) = wms(tag("ATOMIC_SPECIES"))(input)?;
    let (input, species) = many1(wms(parse_species_line))(input)?;

    Ok((input, Block::AtomicSpecies(species)))
}

fn parse_position_line(input: &str) -> IResult<&str, AtomicPosition> {
    let (input, label) = wms(bare_ident)(input)?;
    let (input, (position, curly_bra_or_line_ending)) = many_till(
        ws(bare_ident),
        alt((peek(tag("{")), peek(line_ending), eof)),
    )(input)?;

    let if_pos = if curly_bra_or_line_ending == "{" {
        let (input, _) = tag("{")(input)?;
        let (input, triple) = tuple((wms(bare_ident), wms(bare_ident), wms(bare_ident)))(input)?;
        // discard `}`
        let (_, _) = tag("}")(input)?;
        Some(triple)
    } else {
        None
    };

    Ok((
        input,
        AtomicPosition {
            label,
            position,
            if_pos,
        },
    ))
}

fn parse_atomic_positions(input: &str) -> IResult<&str, Block> {
    let (input, _) = wms(tag("ATOMIC_POSITIONS"))(input)?;
    let (input, typ) = wms(opt(curly_braces_string))(input)?;
    let (input, lst) = many1(parse_position_line)(input)?;

    Ok((input, Block::AtomicPositions { typ, lst }))
}

fn parse_vector_line(input: &str) -> IResult<&str, LatticeVector> {
    let (input, vv) = tuple((ws(bare_ident), ws(bare_ident), ws(bare_ident)))(input)?;
    let (input, _) = line_ending(input)?;

    Ok((input, vv))
}

fn parse_cell_parameters(input: &str) -> IResult<&str, Block> {
    let (input, _) = wms(tag("CELL_PARAMETERS"))(input)?;
    let (input, typ) = wms(opt(curly_braces_string))(input)?;
    let (input, vecs) = many1(parse_vector_line)(input)?;

    assert_eq!(
        vecs.len(),
        3,
        "too many lattice vector for CELL_PARAMETERS, expect 3"
    );

    Ok((input, Block::CellParameters { typ, vecs }))
}

fn parse_kxyzw_line(input: &str) -> IResult<&str, Kxyzw> {
    let (input, kxyzw) = tuple((
        ws(bare_ident),
        ws(bare_ident),
        ws(bare_ident),
        ws(bare_ident),
    ))(input)?;
    let (input, _) = line_ending(input)?;

    Ok((input, kxyzw))
}

fn parse_k_points(input: &str) -> IResult<&str, Block> {
    let (input, _) = wms(tag("K_POINTS"))(input)?;
    let (input, typ) = wms(opt(curly_braces_string))(input)?;

    // the typ requires to be known before parsing rest. Set to default to "tbipa" accourding to
    // INPUT_PW.html
    let typ = typ.unwrap_or(String::from("tpiba"));
    let (input, kpt) = match typ.as_str() {
        "gamma" => (input, KPoints::Gamma),
        "automatic" => {
            let (input, mesh) = tuple((ws(bare_ident), ws(bare_ident), ws(bare_ident)))(input)?;
            let (_, offset) = tuple((ws(bare_ident), ws(bare_ident), ws(bare_ident)))(input)?;

            (input, KPoints::Automatic { mesh, offset })
        }
        "tpiba" | "crystal" | "tpiba_b" | "crystal_b" | "tpiba_c" | "crystal_c" => {
            let (input, nks) = wms(bare_ident)(input)?;
            let (input, lst_xyzw) = many1(parse_kxyzw_line)(input)?;

            (input, KPoints::Kxyzw { typ, nks, lst_xyzw })
        }
        // XXX: test this error propagated up
        _ => return context("unknown typ for K_POINTS card", fail)(input),
    };

    Ok((input, Block::KPointsCard(kpt)))
}

/// parse pw input
pub fn parse_pw_input(input: &str) -> IResult<&str, QEInput> {
    // Repeatedly parse recognized block until can't
    let (input, blocks) = many1(wms(alt((
        parse_namelist,
        parse_atomic_species,
        parse_atomic_positions,
        parse_cell_parameters,
        parse_k_points,
    ))))(input)?;

    Ok((input, QEInput { blocks }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namelist_kv() {
        let test_cases = [
            (r"calculation = 'scf',", "calculation".into(), "scf".into()),
            (r"nstep = 100,", "nstep".into(), "100".into()),
            (r"tstress = .false.,", "tstress".into(), ".false.".into()),
            (r"calculation = '',", "calculation".into(), String::new()),
            (r#"calculation = "","#, "calculation".into(), String::new()),
            (
                r#"calculation = "scf","#,
                "calculation".into(),
                "scf".into(),
            ),
            (r"calculation = 'scf'", "calculation".into(), "scf".into()),
            (r"calculation = scf,", "calculation".into(), "scf".into()),
            (r"calculation =scf,", "calculation".into(), "scf".into()),
            (r" calculation =scf,", "calculation".into(), "scf".into()),
        ];

        for (input, expected_k, expected_v) in test_cases {
            let (_, kv) = parse_kv(input).unwrap();
            assert_eq!(kv, (expected_k, expected_v));
        }
    }

    #[test]
    fn namelist() {
        let input = r"
&control
    pseudo_dir  = 'pseudo/'
    calculation = 'scf',
    prefix = 'Si_exc1',
    title = ''
/
";

        let (_, got) = parse_namelist(input).unwrap();
        assert_eq!(
            got,
            Block::Namelist {
                name: "control".into(),
                lst: vec![
                    ("pseudo_dir".into(), "pseudo/".into()),
                    ("calculation".into(), "scf".into()),
                    ("prefix".into(), "Si_exc1".into()),
                    ("title".into(), "".into())
                ]
            }
        );
    }

    #[test]
    fn namelist_skip_comment() {
        let input = r"
&control
    pseudo_dir  = 'pseudo/'
    calculation = 'scf',
    ! any kind of comment
    prefix = 'Si_exc1',
    title = ''
/
";

        let (_, got) = parse_namelist(input).unwrap();
        assert_eq!(
            got,
            Block::Namelist {
                name: "control".into(),
                lst: vec![
                    ("pseudo_dir".into(), "pseudo/".into()),
                    ("calculation".into(), "scf".into()),
                    ("prefix".into(), "Si_exc1".into()),
                    ("title".into(), "".into())
                ]
            }
        );
    }

    #[test]
    fn atomic_species() {
        let input = r"
ATOMIC_SPECIES
 Si 28.086  Si.pbe-n-rrkjus_psl.1.0.0.UPF
 H 1.0008   H.pz-vbc.UPF
";

        let (_, got) = parse_atomic_species(input).unwrap();
        assert_eq!(
            got,
            Block::AtomicSpecies(vec![
                AtomicSpecie {
                    label: "Si".into(),
                    mass: "28.086".into(),
                    pseudo: "Si.pbe-n-rrkjus_psl.1.0.0.UPF".into()
                },
                AtomicSpecie {
                    label: "H".into(),
                    mass: "1.0008".into(),
                    pseudo: "H.pz-vbc.UPF".into()
                },
            ])
        );
    }

    #[test]
    fn atomic_position_line() {
        let inp = "H  0.00 0.00 -0.35";

        let (_, got) = parse_position_line(inp).unwrap();
        assert_eq!(
            got,
            AtomicPosition {
                label: "H".into(),
                position: vec!["0.00".into(), "0.00".into(), "-0.35".into(),],
                if_pos: None,
            }
        );

        // pw.x allow simple algebraic expression
        let inp = "H  1/3   1/2*3^(-1/2)   0";

        let (_, got) = parse_position_line(inp).unwrap();
        assert_eq!(
            got,
            AtomicPosition {
                label: "H".into(),
                position: vec!["1/3".into(), "1/2*3^(-1/2)".into(), "0".into()],
                if_pos: None,
            }
        );

        let (_, got) = parse_position_line(inp).unwrap();
        assert_eq!(
            got,
            AtomicPosition {
                label: "H".into(),
                position: vec!["1/3".into(), "1/2*3^(-1/2)".into(), "0".into()],
                if_pos: None,
            }
        );

        let inp = "H  0.00 0.00 -0.35 {0 0 0}";

        let (_, got) = parse_position_line(inp).unwrap();
        assert_eq!(
            got,
            AtomicPosition {
                label: "H".into(),
                position: vec!["0.00".into(), "0.00".into(), "-0.35".into(),],
                if_pos: Some(("0".into(), "0".into(), "0".into())),
            }
        );
    }

    #[test]
    fn atomic_positions() {
        let input = r"
ATOMIC_POSITIONS {angstrom}
 H  0.00 0.00 -0.35
 H  0.00 0.00  0.35 {0 0 0}
";

        let (_, got) = parse_atomic_positions(input).unwrap();
        assert_eq!(
            got,
            Block::AtomicPositions {
                typ: Some("angstrom".into()),
                lst: vec![
                    AtomicPosition {
                        label: "H".into(),
                        position: vec!["0.00".into(), "0.00".into(), "-0.35".into(),],
                        if_pos: None,
                    },
                    AtomicPosition {
                        label: "H".into(),
                        position: vec!["0.00".into(), "0.00".into(), "0.35".into(),],
                        if_pos: Some(("0".into(), "0".into(), "0".into())),
                    }
                ],
            }
        );
    }

    #[test]
    fn atomic_positions_wyckoff() {
        let input = r"
ATOMIC_POSITIONS {angstrom}
     H  1a
     H  8g   x
     H  24m  x y
";

        let (_, got) = parse_atomic_positions(input).unwrap();
        assert_eq!(
            got,
            Block::AtomicPositions {
                typ: Some("angstrom".into()),
                lst: vec![
                    AtomicPosition {
                        label: "H".into(),
                        position: vec!["1a".into()],
                        if_pos: None,
                    },
                    AtomicPosition {
                        label: "H".into(),
                        position: vec!["8g".into(), "x".into()],
                        if_pos: None,
                    },
                    AtomicPosition {
                        label: "H".into(),
                        position: vec!["24m".into(), "x".into(), "y".into()],
                        if_pos: None,
                    },
                ],
            }
        );
    }

    #[test]
    fn cell_parameters() {
        let input = r"
CELL_PARAMETERS {alat}
-1.0 1.0 1.0
 1.0 -1.0 1.0
 1.0 1.0 -1.0
";
        let (_, got) = parse_cell_parameters(input).unwrap();
        assert_eq!(
            got,
            Block::CellParameters {
                typ: Some("alat".into()),
                vecs: vec![
                    ("-1.0".into(), "1.0".into(), "1.0".into()),
                    ("1.0".into(), "-1.0".into(), "1.0".into()),
                    ("1.0".into(), "1.0".into(), "-1.0".into()),
                ]
            }
        );

        let input = r"
CELL_PARAMETERS
-1.0 1.0 1.0
 1.0 -1.0 1.0
 1.0 1.0 -1.0
";
        let (_, got) = parse_cell_parameters(input).unwrap();
        assert_eq!(
            got,
            Block::CellParameters {
                typ: None,
                vecs: vec![
                    ("-1.0".into(), "1.0".into(), "1.0".into()),
                    ("1.0".into(), "-1.0".into(), "1.0".into()),
                    ("1.0".into(), "1.0".into(), "-1.0".into()),
                ]
            }
        );
    }

    #[test]
    fn kpoints_card() {
        let input = r"
K_POINTS {gamma}
";
        let (_, got) = parse_k_points(input).unwrap();
        assert_eq!(got, Block::KPointsCard(KPoints::Gamma));

        let input = r"
K_POINTS {automatic}
 2 2 2 1 1 1
";
        let (_, got) = parse_k_points(input).unwrap();
        assert_eq!(
            got,
            Block::KPointsCard(KPoints::Automatic {
                mesh: ("2".into(), "2".into(), "2".into()),
                offset: ("1".into(), "1".into(), "1".into())
            })
        );
        let input = r"
K_POINTS
2  
    0 0 0 1
    0.5 0.5 0.5 1
";
        let (_, got) = parse_k_points(input).unwrap();
        assert_eq!(
            got,
            Block::KPointsCard(KPoints::Kxyzw {
                typ: "tpiba".into(),
                nks: "2".into(),
                lst_xyzw: vec![
                    ("0".into(), "0".into(), "0".into(), "1".into()),
                    ("0.5".into(), "0.5".into(), "0.5".into(), "1".into()),
                ],
            })
        );
    }

    #[test]
    fn pw_input() {
        let input = r"
 &control
    calculation='scf',
 /
 &system
    ibrav = 1,
    celldm(1) =10.0,
    nat=2, ntyp=1,
    ecutwfc = 25.0
 /
 &electrons
 /
ATOMIC_SPECIES
 H 1.0008   H.pz-vbc.UPF
ATOMIC_POSITIONS {angstrom}
 H  0.00 0.00 -0.35
 H  0.00 0.00  0.35
K_POINTS {automatic}
 2 2 2 1 1 1
";
        let (_, got) = parse_pw_input(input).unwrap();
        let expect = QEInput {
            blocks: vec![
                Block::Namelist {
                    name: "control".into(),
                    lst: vec![("calculation".into(), "scf".into())],
                },
                Block::Namelist {
                    name: "system".into(),
                    lst: vec![
                        ("ibrav".into(), "1".into()),
                        ("celldm(1)".into(), "10.0".into()),
                        ("nat".into(), "2".into()),
                        ("ntyp".into(), "1".into()),
                        ("ecutwfc".into(), "25.0".into()),
                    ],
                },
                Block::Namelist {
                    name: "electrons".into(),
                    lst: vec![],
                },
                Block::AtomicSpecies(vec![AtomicSpecie {
                    label: "H".into(),
                    mass: "1.0008".into(),
                    pseudo: "H.pz-vbc.UPF".into(),
                }]),
                Block::AtomicPositions {
                    typ: Some("angstrom".into()),
                    lst: vec![
                        AtomicPosition {
                            label: "H".into(),
                            position: vec!["0.00".into(), "0.00".into(), "-0.35".into()],
                            if_pos: None,
                        },
                        AtomicPosition {
                            label: "H".into(),
                            position: vec!["0.00".into(), "0.00".into(), "0.35".into()],
                            if_pos: None,
                        },
                    ],
                },
                Block::KPointsCard(KPoints::Automatic {
                    mesh: ("2".into(), "2".into(), "2".into()),
                    offset: ("1".into(), "1".into(), "1".into()),
                }),
            ],
        };
        assert_eq!(got, expect);
    }
}
