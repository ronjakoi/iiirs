use nom::{
    Finish, IResult, Parser,
    branch::alt,
    bytes::complete::tag,
    character::complete::digit1,
    combinator::{all_consuming, map_res, recognize},
    multi::separated_list1,
    number::{float, recognize_float, u32},
    sequence::preceded,
};
use std::str::FromStr;

fn main() {
    println!("Hello, world!");
}

#[derive(Debug, PartialEq)]
enum Region {
    Full,
    Square,
    Absolute { x: u32, y: u32, w: u32, h: u32 },
    Percent { x: f32, y: f32, w: f32, h: f32 },
}

fn parse_int(input: &str) -> IResult<&str, u32> {
    map_res(digit1, |s: &str| s.parse()).parse(input)
}

fn parse_float_quad(input: &str) -> IResult<&str, [f32; 4]> {
    let (rem, quad) = (
        float(),
        preceded(tag(","), float()),
        preceded(tag(","), float()),
        preceded(tag(","), float()),
    )
        .parse(input)?;
    Ok((rem, [quad.0, quad.1, quad.2, quad.3]))
}

fn parse_int_quad(input: &str) -> IResult<&str, [u32; 4]> {
    let (rem, quad) = (
        parse_int,
        preceded(tag(","), parse_int),
        preceded(tag(","), parse_int),
        preceded(tag(","), parse_int),
    )
        .parse(input)?;
    Ok((rem, [quad.0, quad.1, quad.2, quad.3]))
}

impl FromStr for Region {
    type Err = nom::error::Error<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let region: Result<Self, nom::error::Error<_>> = match s {
            "full" => Ok::<Region, Self::Err>(Self::Full),
            "square" => Ok(Self::Square),
            _ => {
                if let Ok((_, quad)) =
                    preceded(tag("pct:"), all_consuming(parse_float_quad))
                        .parse(s)
                        .finish()
                {
                    Ok(Self::Percent {
                        x: quad[0],
                        y: quad[1],
                        w: quad[2],
                        h: quad[3],
                    })
                } else {
                    let (_, quad) =
                        all_consuming(parse_int_quad).parse(s).finish()?;
                    Ok(Self::Absolute {
                        x: quad[0],
                        y: quad[1],
                        w: quad[2],
                        h: quad[3],
                    })
                }
            }
        };
        region.map_err(|e: nom::error::Error<_>| nom::error::Error {
            input: e.input.to_string(),
            code: e.code,
        })
    }
}
