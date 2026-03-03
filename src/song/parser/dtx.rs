use std::io;

use bevy::{prelude::*, tasks::futures_lite::AsyncBufRead};
use encoding_rs::SHIFT_JIS;

use crate::utils::encoding::AsyncBufReadEncodingExt;

pub async fn parse_dtx_chart(reader: impl AsyncBufRead + Unpin) -> io::Result<String> {
    let mut reader = reader.with_encoding(SHIFT_JIS);

    let mut output = String::new();
    reader.read_to_string(&mut output).await?;

    Ok(output)
}
