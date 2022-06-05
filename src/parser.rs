use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_until, take_while},
    character::{
        complete::{alpha1, alphanumeric1, char, crlf, multispace0, not_line_ending},
        is_digit,
    },
    combinator::{recognize, success},
    error::{Error, ErrorKind, ParseError},
    error_position,
    multi::{fold_many0, many0_count},
    sequence::{delimited, pair, preceded, separated_pair, terminated},
    IResult,
};

use crate::wgsl::{PType, StructSlot, StructSlotOptions, TType};

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

fn pscalar(name: &str, typed: PType) -> impl FnMut(&str) -> IResult<&str, PType> {
    let name = name.to_owned();
    move |input: &str| {
        return tag(&name[..])(input).map(|(rest, _)| (rest, typed));
    }
}

fn scalar(input: &str) -> IResult<&str, TType> {
    alt((
        pscalar("bool", PType::Bool),
        pscalar("i32", PType::I32),
        pscalar("i64", PType::I64),
        pscalar("u32", PType::U32),
        pscalar("u64", PType::U64),
        pscalar("f16", PType::F16),
        pscalar("f32", PType::F32),
        pscalar("f64", PType::F64),
    ))(input)
    .map(|(rest, typed)| (rest, TType::Scalar(typed)))
}

fn vector(input: &str) -> IResult<&str, TType> {
    let (rest, n) = delimited(tag("vec"), take(1usize), tag("<"))(input)?;
    let (rest, result) = terminated(scalar, tag(">"))(rest)?;

    if let TType::Scalar(scalar_type) = result {
        return Ok((
            rest,
            TType::Vector(
                n.parse().or(Err(nom_error("n dim couldn't be parsed")))?,
                scalar_type,
            ),
        ));
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
                m: m.parse().or(Err(nom_error("m dim couldn't be parsed")))?,
                n: n.parse().or(Err(nom_error("n dim couldn't be parsed")))?,
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

    let (rest, (e, n)) = delimited(tag("array<"), p, tag(">"))(input)?;
    Ok((
        rest,
        TType::Array(
            n.parse().or(Err(nom_error("n dim couldn't be parsed")))?,
            e.into(),
        ),
    ))
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
    let options = structslot_option(comment).map(|(_, opt)| opt).ok();

    Ok((
        rest,
        StructSlot {
            identifier: identifier.to_owned(),
            typed,
            options,
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

    let (_, struct_slots) = fold_many0(
        struct_slot,
        || Vec::with_capacity(10),
        |mut acc: Vec<_>, item| {
            acc.push(item);
            acc
        },
    )(result)?;

    Ok((rest, struct_slots))
}

pub fn adjustment_for_safe_insert(file_content: &str, name: &str) -> Option<usize> {
    let (input, r1) = take_until::<_, _, Error<&str>>(name)(file_content).ok()?;
    let (rest, _) = preceded::<_, _, _, Error<&str>, _, _>(
        terminated(tag(name), alt((tag(" "), tag("")))),
        delimited(char('{'), take_until("}"), char('}')),
    )(input)
    .ok()?;

    Some(r1.len() + input.len() - rest.len() + 3)
}

#[derive(Debug)]
pub enum ShaderOptions {
    Texture {
        path: PathBuf,
        name: String,
        u_addr_mode: Option<wgpu::AddressMode>,
        v_addr_mode: Option<wgpu::AddressMode>,
        w_addr_mode: Option<wgpu::AddressMode>,
    },
    Something,
}

impl ShaderOptions {
    pub fn is_texture_opt(&self) -> bool {
        match self {
            ShaderOptions::Texture { .. } => true,
            _ => false,
        }
    }

    pub fn texture(path: &Path, name: &String) -> ShaderOptions {
        ShaderOptions::Texture {
            path: path.into(),
            name: name.to_owned(),
            u_addr_mode: None,
            v_addr_mode: None,
            w_addr_mode: None,
        }
    }
}

type Arguments<'a> = HashMap<&'a str, &'a str>;

fn nom_error(input: &str) -> nom::Err<Error<&str>> {
    nom::Err::Error(error_position!(input, ErrorKind::Fail))
}

// EWW
fn opt_and_value(input: &str) -> IResult<&str, (&str, &str)> {
    let (rest, name) = take_until("=")(input)?;

    if name.is_empty() {
        return Err(nom_error(input));
    }

    let (rest, value) = take_while(|c: char| c != ',')(&rest[1..])?;

    if value.is_empty() {
        return Err(nom_error(&rest[1..]));
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

fn texture(opt: &str) -> IResult<&str, ShaderOptions> {
    let (rest, _) = tag("texture")(opt)?;
    let (rest, arguments) = arguments(rest)?;

    Ok((
        rest,
        ShaderOptions::Texture {
            path: arguments.get("path").ok_or(nom_error(rest))?.into(),
            name: arguments.get("name").ok_or(nom_error(rest))?.to_string(),
            u_addr_mode: arguments.get("u_mode").and_then(|x| address_mode(x)),
            v_addr_mode: arguments.get("v_mode").and_then(|x| address_mode(x)),
            w_addr_mode: arguments.get("w_mode").and_then(|x| address_mode(x)),
        },
    ))
}

fn something(opt: &str) -> IResult<&str, ShaderOptions> {
    tag("something")(opt).map(|(rest, _)| (rest, ShaderOptions::Something))
}

pub fn shader_option(opt: &str) -> IResult<&str, ShaderOptions> {
    alt((texture, something))(opt)
}

pub fn parse_options(file_content: &str) -> IResult<&str, Vec<ShaderOptions>> {
    let (input, _) = take_until("// Shadey")(file_content)?;
    let (rest, _) = tag("// Shadey")(input)?;
    let any_comment1 = delimited(ws(tag("//")), shader_option, crlf);

    Ok(fold_many0(
        any_comment1,
        Vec::new,
        |mut acc: Vec<_>, item| {
            acc.push(item);
            acc
        },
    )(rest)?)
}

fn range(comment: &str) -> IResult<&str, StructSlotOptions> {
    let (rest, _) = tag("range")(comment)?;
    let (rest2, arguments) = arguments(rest)?;

    let min = arguments
        .get("min")
        .ok_or(nom_error(rest))?
        .parse::<f32>()
        .or(Err(nom_error("Min couldn't be parsed")))?;
    let max = arguments
        .get("max")
        .ok_or(nom_error(rest))?
        .parse::<f32>()
        .or(Err(nom_error("Max couldn't be parsed")))?;
    Ok((rest2, StructSlotOptions::Slider { range: min..=max }))
}

pub fn structslot_option(comment: &str) -> IResult<&str, StructSlotOptions> {
    range(comment)
}
