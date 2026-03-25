use nom::{
    Err, IResult, Parser,
    bytes::complete::{is_not, tag, tag_no_case, take, take_while},
    character::complete::{anychar, not_line_ending, space0},
    combinator::{ParserIterator, all_consuming, cut, iterator, opt, recognize},
    error::{Error, ErrorKind},
    sequence::{preceded, separated_pair},
};
use utils::parser::cut_not_eof;

use crate::assets::song::loader::dtx::chips::Channel;

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

fn parse_command_tag<'a>(command: &'a str, tag: &'static str) -> CommandResult<'a, ()> {
    if command.eq_ignore_ascii_case(tag) {
        Ok(())
    } else {
        Err(Err::Error(Error::new(command, ErrorKind::Tag)))
    }
}

fn parse_command_tag_zz<'a>(command: &'a str, tag: &'static str) -> CommandResult<'a, u16> {
    all_consuming(preceded(
        tag_no_case(tag),
        take(2usize).map_res(|zz| u16::from_str_radix(zz, 36)),
    ))
    .parse(command)
    .map(|(_, zz)| zz)
}

fn parse_value_f64<'a>(value: &'a str) -> CommandResult<'a, f64> {
    value
        .parse()
        .map_err(|_| Err::Failure(Error::new(value, ErrorKind::Float)))
}

pub fn title<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, &'a str> {
    parse_command_tag(command, "TITLE")?;

    Ok(value)
}

pub fn bpm<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, (u16, f64)> {
    let (_, zz) = all_consuming(preceded(
        tag_no_case("BPM"),
        opt(take(2usize)).map_res(|o| o.map_or(Ok(0), |zz| u16::from_str_radix(zz, 36))),
    ))
    .parse(command)?;

    let value = parse_value_f64(value)?;

    Ok((zz, value))
}

pub fn base_bpm<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, f64> {
    parse_command_tag(command, "BASEBPM")?;

    parse_value_f64(value)
}

pub fn wav<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, (u16, &'a str)> {
    let zz = parse_command_tag_zz(command, "WAV")?;

    Ok((zz, value))
}

pub fn object_desc<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, ObjectDesc<'a>> {
    let (_, (measure, channel)) = all_consuming((measure, channel)).parse(command)?;
    let value = ObjectValue(channel, value);

    Ok(ObjectDesc {
        measure,
        channel,
        value,
    })
}

pub struct ObjectDesc<'a> {
    pub measure: u16,
    pub channel: Channel,
    pub value: ObjectValue<'a>,
}

fn measure(input: &str) -> IResult<&str, u16> {
    (
        take(1usize).map_res(|m| u16::from_str_radix(m, 36)),
        take(2usize).map_res(str::parse::<u16>),
    )
        .map(|(m, mm)| m * 100 + mm)
        .parse(input)
}

fn channel(input: &str) -> IResult<&str, Channel> {
    let (rem, channel) = take(2usize)
        .map_res(|cc| u8::from_str_radix(cc, 16))
        .parse(input)?;

    channel
        .try_into()
        .map(|ch| (rem, ch))
        .map_err(|_| Err::Failure(Error::new(input, ErrorKind::MapRes)))
}

pub struct ObjectValue<'a>(Channel, &'a str);

impl<'a> ObjectValue<'a> {
    pub fn into_float(self) -> Result<f64, Err<Error<&'a str>>> {
        self.1
            .parse()
            .map_err(|_| Err::Failure(Error::new(self.1, ErrorKind::Float)))
    }

    pub fn into_iter(
        self,
    ) -> ParserIterator<
        &'a str,
        Error<&'a str>,
        impl Parser<&'a str, Output = u16, Error = Error<&'a str>>,
    > {
        let radix = self.0.value_radix();

        let digit = move |i| anychar.map_opt(|c| c.to_digit(radix)).parse(i);

        let digit_ignoring_underscore =
            move |i| digit.or(preceded(take_while(|c| c == '_'), digit)).parse(i);

        iterator(
            self.1,
            (
                cut_not_eof(digit_ignoring_underscore),
                cut(digit_ignoring_underscore),
            )
                .map(move |(a, b)| (a * radix + b) as u16),
        )
    }
}
