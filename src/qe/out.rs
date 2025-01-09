use nom::{
    bytes::complete::{tag, take_until, take_until1, take_while1},
    character::{
        complete::{newline, not_line_ending, space0},
        streaming::multispace0,
    },
    combinator::{map, opt},
    multi::many0,
    sequence::{delimited, preceded, terminated, tuple},
    AsChar, IResult,
};

#[derive(Debug, Default)]
struct TotalEnergies {
    // For QE<6.5 there is also Harris_foulkes estimate energy, I didn't parse it.
    total: Option<f64>,
    // estimated scf accuracy
    accuracy: Option<f64>,
    // one-electron contribution
    one_electron: Option<f64>,
    // hartree contribution
    hartree: Option<f64>,
    // smearing
    smearing: Option<f64>,
    // XXX: ... more
    // this may not exist
    solvation: Option<f64>,
}

#[derive(Debug)]
enum Block {
    // total energy is F=E-TS and the terms sum up to E
    TotalEnergies(TotalEnergies),
}

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

// Parse a bare (unquoted) number
// It is spaces separated and can contain: ".", "-", "+", "E" and 10 based digitals
fn bare_number(input: &str) -> IResult<&str, String> {
    map(
        take_while1(|c: char| {
            c.is_dec_digit() || c == '-' || c == '+' || c == '.' || c == 'E' || c == 'e'
        }),
        |s: &str| s.to_string(),
    )(input)
}

fn parse_key_value<'a>(
    delimiter: &'a str,
) -> impl Fn(&'a str) -> IResult<&'a str, (String, String, Option<String>)> {
    move |input: &'a str| {
        let (input, key) = terminated(take_until(delimiter), tag(delimiter))(input)?;
        let (input, value) = ws(bare_number)(input)?;

        // Optionally parse a unit to the end of the line.
        let (input, unit_opt) = opt(preceded(space0, not_line_ending))(input)?;
        let unit_opt = unit_opt.map(|u| u.to_string());

        Ok((input, (key.trim().into(), value, unit_opt)))
    }
}

fn take_until_and_consume<'a>(
    term: &'a str,
) -> impl Fn(&'a str) -> IResult<&'a str, (&'a str, &'a str)> {
    move |input: &'a str| {
        let (input, (pre, term)) = tuple((take_until(term), tag(term)))(input)?;
        Ok((input, (pre, term)))
    }
}

fn parse_final_energies(input: &str) -> IResult<&str, Block> {
    // initialize with default
    let mut energies = TotalEnergies::default();

    // The final energy section lead with a "!"
    let (input, _) = preceded(multispace0, ws(tag("!")))(input)?;
    let (input, (_, total_energy_str, _)) = terminated(parse_key_value(" = "), newline)(input)?;
    let (input, (_, energy_accuracy_str, _)) = terminated(parse_key_value(" < "), newline)(input)?;
    let (input, (_, smearing_energy_str, _)) = terminated(parse_key_value(" = "), newline)(input)?;

    energies.total = Some(total_energy_str.parse::<f64>().unwrap());
    energies.accuracy = Some(energy_accuracy_str.parse::<f64>().unwrap());
    energies.smearing = Some(smearing_energy_str.parse::<f64>().unwrap());

    let (input, _) = take_until_and_consume("E is the sum of the following terms:")(input)?;
    let (input, contribs) = many0(terminated(parse_key_value(" = "), newline))(input)?;

    for (key, value, _) in contribs {
        match key.as_str() {
            "one-electron contribution" => {
                let value = value.parse::<f64>().unwrap();
                energies.one_electron = Some(value);
            }
            "hartree contribution" => {
                let value = value.parse::<f64>().unwrap();
                energies.hartree = Some(value);
            }
            _ => eprintln!("not ready"),
        }
    }

    Ok((input, Block::TotalEnergies(energies)))
}

fn parse_final_mag(input: &str) -> IResult<&str, (String, String)> {
    let (input, _) = take_until("total magnetization")(input)?;
    let (input, (_, tot_mag, _)) = terminated(parse_key_value(" = "), newline)(input)?;
    let (input, (_, abs_mag, _)) = terminated(parse_key_value(" = "), newline)(input)?;

    Ok((input, (tot_mag, abs_mag)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_energy_and_magnetization() {
        let input = r"
!    total energy              =     -96.34322116 Ry
     estimated scf accuracy    <          2.4E-10 Ry
     smearing contrib. (-TS)   =       0.00000000 Ry
     internal energy E=F+TS    =     -96.34322116 Ry

     The total energy is F=E-TS. E is the sum of the following terms:
     one-electron contribution =     -78.44986955 Ry
     hartree contribution      =      40.56509198 Ry
     xc contribution           =     -14.80531522 Ry

     total magnetization       =    -0.00 Bohr mag/cell
     absolute magnetization    =     0.00 Bohr mag/cell
";
        let (_, got) = parse_final_energies(input).unwrap();
        dbg!(got);

        let (_, got) = parse_final_mag(input).unwrap();
        dbg!(got);
    }

    #[test]
    fn final_a() {
        let s = "2.4E-10";
        let value: f64 = s.parse().expect("Failed to parse f64");
        println!("{}", value); // prints: 2.4e-10
    }
}
