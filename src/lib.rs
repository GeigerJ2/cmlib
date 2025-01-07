use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while, take_while1},
    character::complete::multispace0,
    combinator::{map, opt},
    multi::{many1, many_till},
    sequence::{delimited, preceded, tuple},
    IResult,
};

type KeyValPair = (String, String);

/// Block type for the block container of the input
#[derive(Debug, PartialEq)]
enum Block {
    // Namelist of QE input is a named list of key/value pairs
    Namelist { name: String, lst: Vec<KeyValPair> },
}

#[derive(Debug, PartialEq)]
struct QEInput {
    blocks: Vec<Block>,
}

/// ws remove white space before and after the inner parser
fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(multispace0, inner, multispace0)
}

// Parse a bare (unquoted) identifier or keyword (e.g. calculation, prefix)
fn bare_ident(input: &str) -> IResult<&str, String> {
    map(
        take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '(' || c == ')' || c == '.'),
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

/// Parse a maybe comment key/value pair starting with `!`
fn parse_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = opt(preceded(tag("!"), take_while(|c| c != '\n')))(input)?;
    Ok((input, ()))
}

/// Parse key value pairs in a namelist, e.g. "calculation = 'scf'".
/// The terminate comma in the end of each pair is optional
fn parse_kv(input: &str) -> IResult<&str, KeyValPair> {
    let (input, (k, _, v, _maybe_comma)) = tuple((
        ws(bare_ident),
        ws(tag("=")),
        ws(alt((
            single_quoted_string,
            map(double_quoted_string, |s| {
                eprintln!("Waring: in {input}, double quotes used, use single quotes instead");
                s
            }),
            bare_ident,
            map(tag("''"), |_| String::new()),
            map(tag("\"\""), |_| String::new()),
        ))),
        opt(ws(tag(","))), // optional trailing comma
    ))(input)?;

    Ok((input, (k, v)))
}

fn parse_namelist(input: &str) -> IResult<&str, Block> {
    // Expect namelist start with &
    let (input, _) = ws(tag("&"))(input)?;
    let (input, name) = ws(bare_ident)(input)?;

    // parse key/value pairs until slash
    let (input, (items, _slash)) = many_till(
        ws(alt((map(parse_kv, Some), map(parse_comment, |()| None)))),
        ws(tag("/")),
    )(input)?;

    let kv_lst = items.into_iter().flatten().collect();

    Ok((input, Block::Namelist { name, lst: kv_lst }))
}

fn parse_qe_input(input: &str) -> IResult<&str, QEInput> {
    // Repeatedly parse recognized block until can't
    let (input, blocks) = many1(ws(alt((parse_namelist,))))(input)?;

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
    fn namelist_many() {
        let input = r"
&control
    pseudo_dir  = 'pseudo/'
    calculation = 'scf',
    prefix = 'Si_exc1',
    title = ''
/
 &system
    ibrav = 0
    ! ibrav =  -3,
    celldm(1) = 20.385647759,
    nat =  1,
    ntyp = 1,
    ecutwfc = 30
 /
 &electrons
 /
";

        let (_, got) = parse_qe_input(input).unwrap();
        assert_eq!(
            got,
            QEInput {
                blocks: vec![
                    Block::Namelist {
                        name: "control".into(),
                        lst: vec![
                            ("pseudo_dir".into(), "pseudo/".into()),
                            ("calculation".into(), "scf".into()),
                            ("prefix".into(), "Si_exc1".into()),
                            ("title".into(), "".into())
                        ]
                    },
                    Block::Namelist {
                        name: "system".into(),
                        lst: vec![
                            ("ibrav".into(), "0".into()),
                            ("celldm(1)".into(), "20.385647759".into()),
                            ("nat".into(), "1".into()),
                            ("ntyp".into(), "1".into()),
                            ("ecutwfc".into(), "30".into())
                        ]
                    },
                    Block::Namelist {
                        name: "electrons".into(),
                        lst: vec![]
                    },
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
}
