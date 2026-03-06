mod commands;

use std::{cmp::Ordering, io};

use bevy::{prelude::warn, tasks::futures_lite::AsyncBufRead};
use encoding_rs::SHIFT_JIS;
use nom::{
    Err, Parser,
    character::complete::space0,
    combinator::{all_consuming, eof, opt},
    error::Error,
    sequence::terminated,
};

use crate::utils::{encoding::AsyncBufReadEncodingExt, parser::*};

use self::commands::*;

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
    pub objects: Vec<Object>,
}

#[derive(Debug, Default)]
struct DtxChartParser {
    title: String,
    objects: Vec<Object>,
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
        } else if let Some(ObjectList {
            measure,
            channel,
            mut iter,
        }) = opt_err(object_list(command, value))?
        {
            // The strategy is to push new objects to the list as we parse through the iterator.
            // But if it fails later, we'll roll back the list.

            let old_len = self.objects.len();
            let mut total_items = 0;

            for (i, obj) in iter.by_ref().enumerate() {
                total_items += 1;

                if obj == 0 {
                    // Only store non-spacing objects
                    continue;
                }

                self.objects.push(Object {
                    measure,
                    fraction: i as f64,
                    channel,
                    value: obj,
                });
            }

            iter.finish().inspect_err(|_| {
                // Parsing failed. Roll back
                self.objects.truncate(old_len);
            })?;

            // All good. Post-process the new objects
            for new_obj in &mut self.objects[old_len..] {
                new_obj.fraction /= total_items as f64;
            }
        } else {
            return Err(ParseError::UnknownCommand(command));
        }

        Ok(())
    }

    fn finalize(self) -> DtxChart {
        let Self { title, mut objects } = self;

        objects.sort_by(|a, b| match a.measure.cmp(&b.measure) {
            Ordering::Equal => a.fraction.total_cmp(&b.fraction),
            other => other,
        });

        DtxChart { title, objects }
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

#[derive(Debug)]
pub struct Object {
    measure: u16,
    fraction: f64,
    channel: u8,
    value: u16,
}
