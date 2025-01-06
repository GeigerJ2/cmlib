use std::collections::HashMap;

use nom::{
    bytes::complete::{tag, take_while1},
    character::complete::multispace0,
    combinator::map,
    sequence::delimited,
    IResult,
};

#[derive(Debug, PartialEq)]
struct Namelist {
    name: String,
    kv: HashMap<String, String>,
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

fn parse_namelist(input: &str) -> IResult<&str, Namelist> {
    // Expect namelist start with &
    let (input, _) = ws(tag("&"))(input)?;
    let (input, name) = ws(bare_ident)(input)?;
    let kv = HashMap::new();

    Ok((input, Namelist { name, kv }))
}

#[cfg(test)]
mod tests {
    use crate::parse_namelist;

    use super::*;

    #[test]
    fn it_works() {
        let input = r#"
&control
    pseudo_dir  = 'pseudo/'
    calculation = 'scf',
    prefix = 'Si_exc1',
    title = ''
/
"#;

        let namelist = parse_namelist(input).unwrap();
        dbg!(namelist);
    }
}
