use std::io;

use bevy::{prelude::*, tasks::futures_lite::AsyncBufRead};
use encoding_rs::SHIFT_JIS;
use nom::{
    Err, IResult, Parser,
    bytes::complete::{is_not, tag},
    character::complete::{not_line_ending, space0},
    combinator::{all_consuming, cut, eof, opt, recognize},
    error::{Error, ErrorKind},
    sequence::{preceded, separated_pair, terminated},
};

use crate::utils::{encoding::AsyncBufReadEncodingExt, parser::*};

pub async fn parse_dtx_chart(reader: impl AsyncBufRead + Unpin) -> io::Result<DtxChart> {
    let mut reader = reader.with_encoding(SHIFT_JIS);

    let mut parser = DtxChartParser::default();
    let mut line = String::new();

    loop {
        line.clear();

        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            break;
        }

        if let Err(err) = parser.parse_line(line.trim_end()) {
            warn!("{:?}. Ignoring line: '{}'", err, line.trim_end());
        }
    }

    Ok(parser.finalize())
}

#[derive(Debug)]
pub struct DtxChart {
    pub title: String,
}

#[derive(Debug, Default)]
struct DtxChartParser {
    title: String,
}

impl DtxChartParser {
    fn parse_line<'a>(&mut self, input: &'a str) -> Result<(), ParseError<'a>> {
        let (input, _) = space0(input)?;

        if let (_, Some(_)) = opt(all_consuming(comment.or(eof))).parse(input)? {
            return Ok(());
        };

        let (_, (command, value)) = all_consuming(terminated(command, opt(comment)))
            .parse(input)
            .map_err(|_| ParseError::NotCommand)?;

        self.parse_command(command, value).map_err(|e| match e {
            ParseError::Nom(_) => ParseError::InvalidCommandValue(value),
            err => err,
        })
    }

    fn parse_command<'a>(
        &mut self,
        command: &'a str,
        value: &'a str,
    ) -> Result<(), ParseError<'a>> {
        if let Some(title) = opt_err(title(command, value))? {
            self.title = title.to_string();
        } else {
            return Err(ParseError::UnknownCommand(command));
        }

        Ok(())
    }

    fn finalize(self) -> DtxChart {
        let Self { title } = self;

        DtxChart { title }
    }
}

#[derive(Debug)]
#[allow(dead_code)] // We use the invariants for logging
enum ParseError<'a> {
    NotCommand,
    UnknownCommand(&'a str),
    InvalidCommandValue(&'a str),
    Nom(Err<Error<&'a str>>),
}

impl<'a> From<Err<Error<&'a str>>> for ParseError<'a> {
    fn from(value: Err<Error<&'a str>>) -> Self {
        Self::Nom(value)
    }
}

fn comment(input: &str) -> IResult<&str, &str> {
    recognize((tag(";"), not_line_ending)).parse(input)
}

fn command(input: &str) -> IResult<&str, (&str, &str)> {
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

fn title<'a>(command: &'a str, value: &'a str) -> CommandResult<'a, &'a str> {
    if command != "TITLE" {
        return Err(Err::Error(Error::new(command, ErrorKind::Tag)));
    }

    Ok(value)
}
