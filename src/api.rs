use nom::{
    Finish, IResult, Parser,
    branch::{alt, permutation},
    bytes::complete::tag,
    character::complete::{char, digit0, digit1},
    combinator::{all_consuming, map, map_res, opt, recognize},
    sequence::{preceded, separated_pair, terminated},
};
use std::num::NonZeroU32;
use std::str::FromStr;

pub struct ImageRequest {
    region: Region,
    size: Size,
    rotation: Rotation,
    quality: Quality,
}

/// Parse from text a floating point number that disallows Inf, NaN, e and
/// negatives
fn parse_iiif_float(input: &str) -> IResult<&str, f32> {
    map_res(
        alt((
            recognize(digit1::<&str, _>),
            recognize((digit0, char('.'), digit1)),
        )),
        str::parse,
    )
    .parse(input)
}

#[derive(Debug, PartialEq)]
pub enum Region {
    Full,
    Square,
    Absolute {
        x: u32,
        y: u32,
        w: NonZeroU32,
        h: NonZeroU32,
    },
    Percent {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

fn parse_unsigned<T: FromStr>(input: &str) -> IResult<&str, T> {
    map_res(digit1, |s: &str| s.parse()).parse(input)
}

fn parse_float_quad(input: &str) -> IResult<&str, (f32, f32, f32, f32)> {
    let (rem, quad) = (
        parse_iiif_float,
        preceded(tag(","), parse_iiif_float),
        preceded(tag(","), parse_iiif_float),
        preceded(tag(","), parse_iiif_float),
    )
        .parse(input)?;
    Ok((rem, quad))
}

fn parse_nonzerou32(input: &str) -> IResult<&str, NonZeroU32> {
    map_res(parse_unsigned, |x: u32| NonZeroU32::try_from(x)).parse(input)
}

fn parse_int_xywh(
    input: &str,
) -> IResult<&str, (u32, u32, NonZeroU32, NonZeroU32)> {
    let (rem, quad) = (
        parse_unsigned,
        preceded(tag(","), parse_unsigned),
        preceded(tag(","), parse_nonzerou32),
        preceded(tag(","), parse_nonzerou32),
    )
        .parse(input)?;
    Ok((rem, quad))
}

fn parse_int_quad(input: &str) -> IResult<&str, (u32, u32, u32, u32)> {
    let (rem, quad) = (
        parse_unsigned,
        preceded(tag(","), parse_unsigned),
        preceded(tag(","), parse_unsigned),
        preceded(tag(","), parse_unsigned),
    )
        .parse(input)?;
    Ok((rem, quad))
}

impl FromStr for Region {
    type Err = nom::error::Error<String>;

    #[allow(clippy::many_single_char_names)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let region: Result<Self, nom::error::Error<_>> = match s {
            "full" => Ok::<Region, Self::Err>(Self::Full),
            "square" => Ok(Self::Square),
            _ => {
                if let Ok((_, (x, y, w, h))) =
                    preceded(tag("pct:"), all_consuming(parse_float_quad))
                        .parse(s)
                        .finish()
                {
                    Ok(Self::Percent { x, y, w, h })
                } else {
                    let (_, (x, y, w, h)) =
                        all_consuming(parse_int_xywh).parse(s).finish()?;
                    Ok(Self::Absolute { x, y, w, h })
                }
            }
        };
        region.map_err(|e: nom::error::Error<_>| nom::error::Error {
            input: e.input.to_string(),
            code: e.code,
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct Size {
    allow_upscale: bool,
    maintain_ratio: bool,
    kind: SizeKind,
}

#[derive(Debug, PartialEq)]
enum SizeKind {
    Max,
    Width(NonZeroU32),
    Height(NonZeroU32),
    Percent(f32),
    WidthHeight { w: NonZeroU32, h: NonZeroU32 },
}

fn parse_upscale(input: &str) -> IResult<&str, bool> {
    map(opt(tag("^")), |upscale| upscale.is_some()).parse(input)
}

fn parse_maintain_ratio(input: &str) -> IResult<&str, bool> {
    map(opt(tag("!")), |maintain_ratio| maintain_ratio.is_some()).parse(input)
}

fn parse_sizekind(input: &str) -> IResult<&str, SizeKind> {
    alt((
        map(tag("max"), |_| SizeKind::Max),
        map(
            separated_pair(parse_nonzerou32, tag(","), parse_nonzerou32),
            |(w, h)| SizeKind::WidthHeight { w, h },
        ),
        map(preceded(tag(","), parse_nonzerou32), SizeKind::Height),
        map(terminated(parse_nonzerou32, tag(",")), SizeKind::Width),
        map(preceded(tag("pct:"), parse_iiif_float), |pct| {
            SizeKind::Percent(pct)
        }),
    ))
    .parse(input)
}

fn parse_size(input: &str) -> IResult<&str, Size> {
    let (i, (allow_upscale, maintain_ratio)) =
        permutation((parse_upscale, parse_maintain_ratio)).parse(input)?;
    let (_, image_size) = all_consuming(parse_sizekind).parse(i)?;

    Ok((
        "",
        Size {
            allow_upscale,
            maintain_ratio,
            kind: image_size,
        },
    ))
}

impl FromStr for Size {
    type Err = nom::error::Error<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, size) = parse_size(s).finish()?;
        Ok(size)
    }
}

enum Quality {
    Color,
    Gray,
    Bitonal,
    Default,
}

impl FromStr for Quality {
    type Err = nom::error::Error<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let res: Result<(_, _), nom::error::Error<_>> = all_consuming(alt((
            map(tag("color"), |_| Self::Color),
            map(tag("gray"), |_| Self::Gray),
            map(tag("bitonal"), |_| Self::Bitonal),
            map(tag("default"), |_| Self::Default),
        )))
        .parse(s)
        .finish();

        Ok(res?.1)
    }
}

struct Rotation {
    deg: f32,
    mirror: bool,
}

impl FromStr for Rotation {
    type Err = nom::error::Error<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, (mirror, deg)) = all_consuming((
            map(opt(tag("!")), |m| m.is_some()),
            parse_iiif_float,
        ))
        .parse(s)
        .finish()?;
        Ok(Self { deg, mirror })
    }
}
