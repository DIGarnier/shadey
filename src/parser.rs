use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_until, take_while},
    character::{
        complete::{alpha1, alphanumeric1, char, crlf, multispace0, not_line_ending},
        is_digit,
    },
    combinator::{recognize, success},
    error::ParseError,
    multi::{fold_many0, many0_count},
    sequence::{delimited, pair, preceded, separated_pair, terminated},
    IResult,
};

use crate::wgsl::{PType, StructSlot, TType};

fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_")))),
    ))(input)
}

fn scalar(input: &str) -> IResult<&str, TType> {
    let (rest, result) = alt((
        tag("bool"),
        tag("i32"),
        tag("i64"),
        tag("u32"),
        tag("u64"),
        tag("f16"),
        tag("f32"),
        tag("f64"),
    ))(input)?;

    let val = match result {
        "bool" => PType::Bool,
        "i32" => PType::I32,
        "i64" => PType::I64,
        "u32" => PType::U32,
        "u64" => PType::U64,
        "f16" => PType::F16,
        "f32" => PType::F32,
        "f64" => PType::F64,
        _ => unreachable!(),
    };

    Ok((rest, TType::Scalar(val)))
}

fn vector(input: &str) -> IResult<&str, TType> {
    let (rest, n) = delimited(tag("vec"), take(1usize), tag("<"))(input)?;

    let (rest, result) = terminated(scalar, tag(">"))(rest)?;

    if let TType::Scalar(scalar_type) = result {
        return Ok((rest, TType::Vector(n.parse().unwrap(), scalar_type)));
    }
    unreachable!();
}

fn matrix(input: &str) -> IResult<&str, TType> {
    let (rest, m_x_n) = delimited(tag("mat"), take(3usize), tag("<"))(input)?;

    let (_, (m, n)) = separated_pair(take(1usize), tag("x"), take(1usize))(m_x_n)?;

    let (rest, result) = terminated(scalar, tag(">"))(rest)?;

    if let TType::Scalar(scalar_type) = result {
        return Ok((
            rest,
            TType::Matrix {
                m: m.parse().unwrap(),
                n: n.parse().unwrap(),
                typed: scalar_type,
            },
        ));
    }
    unreachable!();
}

fn array(input: &str) -> IResult<&str, TType> {
    let p = separated_pair(
        alt((scalar, vector, matrix)),
        ws(tag(",")),
        take_while(|c: char| is_digit(c as u8)),
    );

    let (rest, e_comma_n) = delimited(tag("array<"), p, tag(">"))(input)?;
    let (e, n) = e_comma_n;
    Ok((rest, TType::Array(n.parse().unwrap(), e.into())))
}

fn typer(input: &str) -> IResult<&str, TType> {
    let (rest, type_value) = alt((scalar, vector, matrix, array))(input)?;

    let (rest, _) = alt((tag(";"), tag("")))(rest)?;

    Ok((rest, type_value))
}

fn comment(input: &str) -> IResult<&str, &str> {
    delimited(ws(tag("//")), not_line_ending, crlf)(input)
}

fn struct_slot(input: &str) -> IResult<&str, StructSlot> {
    let (rest, (identifier, typed)) = separated_pair(ws(identifier), tag(":"), ws(typer))(input)?;

    let (rest, comment) = alt((comment, success("")))(rest)?;

    Ok((
        rest,
        StructSlot {
            identifier: identifier.to_owned(),
            typed,
            comment: comment.to_owned(),
        },
    ))
}

pub fn parse_struct_named<'fc>(
    file_content: &'fc str,
    name: &str,
) -> IResult<&'fc str, Vec<StructSlot>> {
    let (input, _) = take_until(name)(file_content)?;
    let (_, result) = preceded(
        terminated(tag(name), alt((tag(" "), tag("")))),
        delimited(char('{'), take_until("}"), char('}')),
    )(input)?;

    let (rest, struct_slots) = fold_many0(struct_slot, Vec::new, |mut acc: Vec<_>, item| {
        acc.push(item);
        acc
    })(result)?;

    Ok((rest, struct_slots))
}
