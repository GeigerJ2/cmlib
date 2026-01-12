use nom::{
    bytes::complete::take_while1,
    character::complete::{multispace0, space0},
    combinator::map,
    sequence::delimited,
    IResult,
};

/// wms remove white space before and after the inner parser
/// It mostly used for line parser
pub fn wms<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(multispace0, inner, multispace0)
}

/// ws remove white space (different from `wms` will not remove line break) before and after the inner parser
/// It mostly used for identifier parser
pub fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(space0, inner, space0)
}

/// Parse a bare (unquoted) identifier or keyword
/// `alphanumeric`, `_`, `(`, `)`, `.`, `-`, `/`, `+`, `*`, `^` in the parsed string.
pub fn bare_ident(input: &str) -> IResult<&str, String> {
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
