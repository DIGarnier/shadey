use std::{collections::HashMap, path::PathBuf};

use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_until, take_while},
    character::{
        complete::{alpha1, alphanumeric1, char, crlf, multispace0, not_line_ending},
        is_digit,
    },
    combinator::{recognize, success},
    error::{Error, ParseError},
    multi::{fold_many0, many0_count},
    sequence::{delimited, pair, preceded, separated_pair, terminated}, IResult,
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

fn any_comment(input: &str) -> IResult<&str, &str> {
    delimited(ws(tag("//")), not_line_ending, crlf)(input)
}

fn struct_slot(input: &str) -> IResult<&str, StructSlot> {
    let (rest, (identifier, typed)) = separated_pair(ws(identifier), tag(":"), ws(typer))(input)?;

    let (rest, comment) = alt((any_comment, success("")))(rest)?;

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
    let (rest, result) = preceded(
        terminated(tag(name), alt((tag(" "), tag("")))),
        delimited(char('{'), take_until("}"), char('}')),
    )(input)?;

    let (_, struct_slots) = fold_many0(struct_slot, || Vec::with_capacity(10), |mut acc: Vec<_>, item| {
        acc.push(item);
        acc
    })(result)?;

    Ok((rest, struct_slots))
}

pub fn adjustment_for_safe_insert(
    file_content: &str,
    name: &str,
) -> Option<usize> {
    let (input, r1) = take_until::<_,_, Error<&str>>(name)(file_content).ok()?;
    let (rest, _) = preceded::<_,_,_, Error<&str>,_,_>(
        terminated(tag(name), alt((tag(" "), tag("")))),
        delimited(char('{'), take_until("}"), char('}')),
    )(input).ok()?;

    Some(r1.len() + input.len() - rest.len() + 3)
}




#[derive(Debug)]
pub enum ShaderOptions {
    Texture {
        path: PathBuf,
        alias: Option<String>,
        u_addr_mode: Option<wgpu::AddressMode>,
        v_addr_mode: Option<wgpu::AddressMode>,
        w_addr_mode: Option<wgpu::AddressMode>,
    },
}

type Arguments<'a> = HashMap<&'a str, &'a str>;

// EWW
fn opt_and_value(input: &str) -> IResult<&str, (&str, &str)> {
    let (rest, name) = take_until("=")(input)?;

    if name.is_empty() {
        return Err(nom::Err::Failure(Error::new(
            "argument name can't be empty",
            nom::error::ErrorKind::Fail,
        )));
    }

    let (rest, value) = take_while(|c: char| c != ',')(&rest[1..])?;

    if value.is_empty() {
        return Err(nom::Err::Failure(Error::new(
            "value can't be empty",
            nom::error::ErrorKind::Fail,
        )));
    }

    Ok((rest, (name, value)))
}

pub fn arguments(input: &str) -> IResult<&str, Arguments> {
    let (rest, result) = delimited(tag("("), take_until(")"), tag(")"))(input)?;

    let p1 = terminated(opt_and_value, alt((tag(", "), tag(","), tag(""))));
    let (_, args) = fold_many0(p1, HashMap::new, |mut acc: HashMap<_, _>, (k, v)| {
        acc.insert(k, v);
        acc
    })(result)?;

    Ok((rest, args))
}

fn address_mode(input: &str) -> Option<wgpu::AddressMode> {
    match &input.to_lowercase()[..] {
        "clamptoedge" => wgpu::AddressMode::ClampToEdge.into(),
        "clamptoborder" => wgpu::AddressMode::ClampToBorder.into(),
        "repeat" => wgpu::AddressMode::Repeat.into(),
        "mirrorrepeat" => wgpu::AddressMode::MirrorRepeat.into(),
        _ => None,
    }
}

fn texture(arguments: &Arguments) -> Option<ShaderOptions> {
    let path: PathBuf = arguments.get("path")?.into();
    let alias = arguments.get("alias").map(|x| x.to_string());
    let u_addr_mode = arguments.get("u_mode").and_then(|x| address_mode(x));
    let v_addr_mode = arguments.get("v_mode").and_then(|x| address_mode(x));
    let w_addr_mode = arguments.get("w_mode").and_then(|x| address_mode(x));

    Some(ShaderOptions::Texture {
        path,
        alias,
        u_addr_mode,
        v_addr_mode,
        w_addr_mode,
    })
}

pub fn shader_option(opt: &str) -> IResult<&str, ShaderOptions> {
    let (rest, result) = alt((tag("texture"), tag("something")))(opt)?;

    let (rest, arguments) = arguments(rest)?;

    let option = match result {
        "texture" => texture(&arguments),
        _ => unreachable!(),
    };

    Ok((rest, option.unwrap()))
}

pub fn parse_options(file_content: &str) -> IResult<&str, Vec<ShaderOptions>> {
    let (input, _) = take_until("// Shadey")(file_content)?;
    let (rest, _ ) = tag("// Shadey")(input)?;
    let any_comment1 = delimited(ws(tag("//")), shader_option, crlf);

    Ok(fold_many0(any_comment1, Vec::new, |mut acc: Vec<_>, item| {
        acc.push(item);
        acc
    })(rest)?)
}
