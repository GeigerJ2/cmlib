use nom::{
    bytes::complete::{escaped, tag, take_until, take_until1, take_while1},
    character::{
        complete::{char, newline, not_line_ending, space0},
        streaming::multispace0,
    },
    combinator::{cut, map, opt},
    error::{context, ContextError, ParseError, VerboseError},
    multi::{many0, many_till},
    sequence::{delimited, preceded, terminated, tuple},
    AsChar, IResult, Parser,
};

// #[derive(Debug, Default)]
// struct TotalEnergies {
//     // For QE<6.5 there is also Harris_foulkes estimate energy, I didn't parse it.
//     total: Option<f64>,
//     // estimated scf accuracy
//     accuracy: Option<f64>,
//     // one-electron contribution
//     one_electron: Option<f64>,
//     // hartree contribution
//     hartree: Option<f64>,
//     // smearing
//     smearing: Option<f64>,
//     // XXX: ... more
//     // this may not exist
//     solvation: Option<f64>,
// }

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
fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, VerboseError<&str>>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, VerboseError<&str>>,
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
fn bare_number(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    map(
        take_while1(|c: char| {
            c.is_dec_digit() || c == '-' || c == '+' || c == '.' || c == 'E' || c == 'e'
        }),
        |s: &str| s.to_string(),
    )(input)
}

fn parse_key_value<'a>(
    delimiter: &'a str,
) -> impl Fn(&'a str) -> IResult<&'a str, (String, String, Option<String>), VerboseError<&str>> {
    move |input: &'a str| {
        let (input, key) = context("parse aa", terminated(take_until(delimiter), tag(delimiter)))(input)?;
        let (input, value) = context("number", ws(bare_number))(input)?;

        // Optionally parse a unit to the end of the line.
        let (input, unit_opt) = opt(preceded(space0, not_line_ending))(input)?;
        let unit_opt = unit_opt.map(|u| u.to_string());

        Ok((input, (key.trim().into(), value, unit_opt)))
    }
}

fn take_until_and_consume<'a>(
    term: &'a str,
) -> impl Fn(&'a str) -> IResult<&'a str, (&'a str, &'a str), VerboseError<&str>> {
    move |input: &'a str| {
        let (input, (pre, term)) = tuple((take_until(term), tag(term)))(input)?;
        Ok((input, (pre, term)))
    }
}

type TotalEnergies = (Option<f64>, Option<f64>, Option<f64>);

/// parsing total_energy, accuracy, smearing from following section
/// !    total energy              =     -96.34322116 Ry
///      estimated scf accuracy    <          2.4E-10 Ry
///      smearing contrib. (-TS)   =       0.00000000 Ry
///      internal energy E=F+TS    =     -96.34322116 Ry
fn parse_tot_energy(input: &str) -> IResult<&str, TotalEnergies, VerboseError<&str>> {
    // The final energy section lead with a "!"
    let (input, _) = preceded(multispace0, ws(tag("!")))(input)?;
    let (input, (_, total_energy_str, _)) = terminated(parse_key_value(" = "), newline)(input)?;
    let (input, (_, energy_accuracy_str, _)) = terminated(parse_key_value(" < "), newline)(input)?;
    let (input, (_, smearing_energy_str, _)) = terminated(parse_key_value(" = "), newline)(input)?;

    let total = Some(total_energy_str.parse::<f64>().unwrap());
    let accuracy = Some(energy_accuracy_str.parse::<f64>().unwrap());
    let smearing = Some(smearing_energy_str.parse::<f64>().unwrap());

    Ok((input, (total, accuracy, smearing)))
}

fn parse_energy_components(input: &str) -> IResult<&str, Vec<(String, f64)>, VerboseError<&str>> {
    let (input, _) = take_until_and_consume("E is the sum of the following terms:")(input)?;
    let (input, (contribs, _)) =
        many_till(terminated(parse_key_value(" = "), newline), tag("\n\n"))(input)?;

    // discard the unit
    // TODO: convert to eV
    let contribs: Vec<_> = contribs
        .iter()
        .map(|(k, v, _u)| {
            let v = v.parse::<f64>().unwrap();
            (k.to_owned(), v)
        })
        .collect();

    Ok((input, contribs))
}

fn parse_final_mag(input: &str) -> IResult<&str, (String, String), VerboseError<&str>> {
    let (input, _) = take_until("total magnetization")(input)?;
    let (input, (_, tot_mag, _)) = terminated(parse_key_value(" = "), newline)(input)?;
    let (input, (_, abs_mag, _)) = terminated(parse_key_value(" = "), newline)(input)?;

    Ok((input, (tot_mag, abs_mag)))
}

#[cfg(test)]
mod tests {
    use nom::Finish;

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
        let (_, got) = parse_tot_energy(input).unwrap();
        dbg!(got);

        let e = context("energy component", parse_energy_components)(input).finish().err().unwrap();
        dbg!(e);
    }

    // #[test]
    // fn final_a() {
    //     let s = "2.4E-10";
    //     let value: f64 = s.parse().expect("Failed to parse f64");
    //     println!("{}", value); // prints: 2.4e-10
    // }
}
