use nom::{
    Err, IResult, Parser,
    bytes::complete::{is_not, tag, take, take_while},
    character::complete::{anychar, not_line_ending, space0},
    combinator::{ParserIterator, all_consuming, cut, iterator, opt, recognize},
    error::{Error, ErrorKind},
    sequence::{preceded, separated_pair},
};

use crate::utils::parser::*;

pub fn comment(input: &str) -> IResult<&str, &str> {
    recognize((tag(";"), not_line_ending)).parse(input)
}

pub fn command(input: &str) -> IResult<&str, (&str, &str)> {
    preceded(
        tag("#"),
        cut(separated_pair(
            is_not(": \t;\r\n"),
            opt(tag(":")).and(space0),
            opt(is_not(";\r\n"))
                .map(Option::unwrap_or_default)
                .map(str::trim_end),
        )),
    )
    .parse(input)
}

type CommandResult<'a, O> = Result<O, Err<Error<&'a str>>>;

pub fn title<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, &'a str> {
    if command != "TITLE" {
        return Err(Err::Error(Error::new(command, ErrorKind::Tag)));
    }

    Ok(value)
}

pub struct ObjectList<'a, P> {
    pub measure: u16,
    pub channel: u8,
    pub iter: ParserIterator<&'a str, Error<&'a str>, P>,
}

fn measure(input: &str) -> IResult<&str, u16> {
    (
        take(1usize).map_res(|m| u16::from_str_radix(m, 36)),
        take(2usize).map_res(str::parse::<u16>),
    )
        .map(|(m, mm)| m * 100 + mm)
        .parse(input)
}

fn channel(input: &str) -> IResult<&str, u8> {
    take(2usize)
        .map_res(|cc| u8::from_str_radix(cc, 16))
        .parse(input)
}

pub fn object_list<'a>(
    command: &'a str,
    value: &'a str,
) -> CommandResult<'a, ObjectList<'a, impl Parser<&'a str, Output = u16, Error = Error<&'a str>>>> {
    let (_, (measure, channel)) = all_consuming((measure, channel)).parse(command)?;

    let radix = 36; // TODO: to be taken from `channel` info somehow.
    let digit = move |i| anychar.map_opt(|c| c.to_digit(radix)).parse(i);

    let digit_ignoring_underscore =
        move |i| digit.or(preceded(take_while(|c| c == '_'), digit)).parse(i);

    let iter = iterator(
        value,
        (
            cut_not_eof(digit_ignoring_underscore),
            cut(digit_ignoring_underscore),
        )
            .map(move |(a, b)| (a * radix + b) as u16),
    );

    Ok(ObjectList {
        measure,
        channel,
        iter,
    })
}
