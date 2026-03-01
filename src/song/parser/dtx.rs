use std::io;

use bevy::{
    prelude::*,
    tasks::futures_lite::{AsyncBufRead, StreamExt},
};
use encoding_rs::SHIFT_JIS;

use crate::utils::AsyncBufReadEncodingExt;

pub async fn parse_dtx_chart(reader: impl AsyncBufRead + Unpin) -> io::Result<String> {
    let mut lines = reader.lines_decoded(SHIFT_JIS);
    let mut output = String::new();

    while let Some(line) = lines.try_next().await? {
        output.push_str(&line);
    }

    Ok(output)
}
