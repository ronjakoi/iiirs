use image::ImageFormat;
use nom::{
    Finish, IResult, Parser,
    branch::{alt, permutation},
    bytes::complete::{tag, take_until1},
    character::complete::{alphanumeric1, char, digit0, digit1},
    combinator::{all_consuming, map, map_res, opt, recognize},
    sequence::{preceded, separated_pair, terminated},
};
use std::{num::NonZeroU32, str::FromStr};

#[derive(Debug, PartialEq)]
pub struct ImageRequest {
    pub identifier: String,
    pub region: Region,
    pub size: Size,
    pub rotation: Rotation,
    pub quality: Quality,
    pub format: ImageFormat,
}

#[derive(Debug, PartialEq, Default)]
pub enum Region {
    #[default]
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

#[derive(Debug, PartialEq, Default)]
pub struct Size {
    pub allow_upscale: bool,
    pub maintain_ratio: bool,
    pub kind: SizeKind,
}

#[derive(Debug, PartialEq, Default)]
pub enum Quality {
    #[default]
    Color,
    Gray,
    Bitonal,
    Default,
}

#[derive(Debug, PartialEq, Default)]
pub struct Rotation {
    pub deg: RotationDeg,
    pub mirror: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum RotationDeg {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl FromStr for ImageRequest {
    type Err = nom::error::Error<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, request) = parse_image_request(s).finish()?;
        Ok(request)
    }
}

fn parse_image_request(input: &str) -> IResult<&str, ImageRequest> {
    let (i, identifier) =
        terminated(parse_identifier, tag("/")).parse(input)?;
    let (i, region) = terminated(parse_region, tag("/")).parse(i)?;
    let (i, size) = terminated(parse_size, tag("/")).parse(i)?;
    let (i, rotation) = terminated(parse_rotation, tag("/")).parse(i)?;
    let (i, (quality, format)) =
        all_consuming(separated_pair(parse_quality, tag("."), parse_format))
            .parse(i)?;
    Ok((
        i,
        ImageRequest {
            identifier,
            region,
            size,
            rotation,
            quality,
            format,
        },
    ))
}

fn parse_identifier(input: &str) -> IResult<&str, String> {
    map(take_until1("/"), String::from).parse(input)
}

/// Parse from text a floating point number that disallows Inf, NaN, e and
/// negatives
fn parse_iiif_float(input: &str) -> IResult<&str, f32> {
    map_res(
        alt((
            recognize((digit0, char('.'), digit1)),
            recognize(digit1::<&str, _>),
        )),
        str::parse,
    )
    .parse(input)
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

fn parse_region(input: &str) -> IResult<&str, Region> {
    alt((
        map(tag("full"), |_| Region::Full),
        map(tag("square"), |_| Region::Square),
        map(preceded(tag("pct:"), parse_float_quad), |(x, y, w, h)| {
            Region::Percent { x, y, w, h }
        }),
        map(parse_int_xywh, |(x, y, w, h)| Region::Absolute {
            x,
            y,
            w,
            h,
        }),
    ))
    .parse(input)
}

impl FromStr for Region {
    type Err = nom::error::Error<String>;

    #[allow(clippy::many_single_char_names)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, region) = parse_region(s).finish()?;
        Ok(region)
    }
}

#[derive(Debug, PartialEq, Default)]
pub enum SizeKind {
    #[default]
    Max,
    Width(NonZeroU32),
    Height(NonZeroU32),
    Percent(f32),
    WidthHeight {
        w: NonZeroU32,
        h: NonZeroU32,
    },
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
        map(terminated(parse_nonzerou32, tag(",")), SizeKind::Height),
        map(preceded(tag(","), parse_nonzerou32), SizeKind::Width),
        map(preceded(tag("pct:"), parse_iiif_float), |pct| {
            SizeKind::Percent(pct)
        }),
    ))
    .parse(input)
}

fn parse_size(input: &str) -> IResult<&str, Size> {
    let (i, (allow_upscale, maintain_ratio)) =
        permutation((parse_upscale, parse_maintain_ratio)).parse(input)?;
    let (i, image_size) = parse_sizekind(i)?;

    Ok((
        i,
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

fn parse_quality(input: &str) -> IResult<&str, Quality> {
    alt((
        map(tag("color"), |_| Quality::Color),
        map(tag("gray"), |_| Quality::Gray),
        map(tag("bitonal"), |_| Quality::Bitonal),
        map(tag("default"), |_| Quality::Default),
    ))
    .parse(input)
}

impl FromStr for Quality {
    type Err = nom::error::Error<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, quality) = parse_quality(s).finish()?;
        Ok(quality)
    }
}

fn parse_rotation_deg(input: &str) -> IResult<&str, RotationDeg> {
    alt((
        map(alt((tag("0"), tag("360"))), |_| RotationDeg::Deg0),
        map(tag("90"), |_| RotationDeg::Deg90),
        map(tag("180"), |_| RotationDeg::Deg180),
        map(tag("270"), |_| RotationDeg::Deg270),
    ))
    .parse(input)
}

fn parse_rotation(input: &str) -> IResult<&str, Rotation> {
    map(
        (opt(tag("!")), parse_rotation_deg),
        |(m, deg): (Option<&str>, RotationDeg)| Rotation {
            deg,
            mirror: m.is_some(),
        },
    )
    .parse(input)
}

pub fn parse_format(input: &str) -> IResult<&str, ImageFormat> {
    map_res(alphanumeric1, |ext| {
        ImageFormat::from_extension(ext)
            .ok_or(nom::error::Error::new(input, nom::error::ErrorKind::MapRes))
    })
    .parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rotation() {
        assert_eq!(
            parse_rotation("0"),
            Ok((
                "",
                Rotation {
                    deg: RotationDeg::Deg0,
                    mirror: false
                }
            ))
        );

        assert_eq!(
            parse_rotation("!25"),
            Ok((
                "",
                Rotation {
                    deg: RotationDeg::Deg90,
                    mirror: true
                }
            ))
        );

        assert!(parse_rotation("flip").is_err());
        assert!(parse_rotation("-180").is_err());
        assert!(parse_rotation("45").is_err());
    }
}
