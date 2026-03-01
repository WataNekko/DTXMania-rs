use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use bevy::tasks::futures_lite::{AsyncBufRead, Stream};
use bevy::{prelude::*, tasks::futures_lite::ready};
use encoding_rs::{CoderResult, Decoder, Encoding};

pub trait AsyncBufReadEncodingExt: AsyncBufRead + Sized {
    fn lines_decoded(self, encoding: &'static Encoding) -> impl Stream<Item = io::Result<String>>;
}

impl<R: AsyncBufRead + Unpin> AsyncBufReadEncodingExt for R {
    fn lines_decoded(self, encoding: &'static Encoding) -> impl Stream<Item = io::Result<String>> {
        LinesDecoded {
            reader: self,
            decoder: encoding.new_decoder(),
            decoded_buf: String::new(),
            state: State::PendingRead,
        }
    }
}

struct LinesDecoded<R> {
    reader: R,
    decoder: Decoder,
    decoded_buf: String,
    state: State,
}

enum State {
    PendingRead,
    Available,
    Eof,
}

impl<R: AsyncBufRead + Unpin> Stream for LinesDecoded<R> {
    type Item = io::Result<String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Self {
            reader,
            decoder,
            decoded_buf,
            state,
        } = &mut *self;

        loop {
            match state {
                State::PendingRead => {
                    let chunk = ready!(Pin::new(&mut *reader).poll_fill_buf(cx))?;

                    if chunk.is_empty() {
                        // Flush the decoder's internal state
                        let max_len = decoder.max_utf8_buffer_length(0).unwrap();
                        decoded_buf.reserve(max_len);
                        let (res, read, err) = decoder.decode_to_string(&[], decoded_buf, true);

                        if err {
                            warn!(
                                "Encountered invalid sequence at the end while decoding {}.",
                                decoder.encoding().name()
                            );
                        }
                        debug_assert_eq!(res, CoderResult::InputEmpty);
                        debug_assert_eq!(read, 0);

                        *state = State::Eof;
                        if decoded_buf.is_empty() {
                            continue;
                        } else {
                            return Poll::Ready(Some(Ok(std::mem::take(decoded_buf))));
                        };
                    }

                    let max_len = decoder.max_utf8_buffer_length(chunk.len()).unwrap();
                    decoded_buf.reserve(max_len);

                    let (res, read, err) = decoder.decode_to_string(chunk, decoded_buf, false);
                    if err {
                        warn!(
                            "Encountered invalid sequence while decoding {}.",
                            decoder.encoding().name()
                        );
                    }

                    debug_assert_ne!(
                        res,
                        CoderResult::OutputFull,
                        "Reserving max required length before decoding should have ensured result is never OutputFull."
                    );

                    let chunk_len = chunk.len();
                    Pin::new(&mut *reader).consume(chunk_len);
                    debug_assert_eq!(
                        read, chunk_len,
                        "We assume InputEmpty result would have gobbled all of `chunk` so we consume them all.\
                        If this is not the case for whatever reason then we've dropped some data."
                    );
                    // We don't want to consume only `read` though, because that can be an infinite
                    // loop where fill_buf doesn't return any new data until we consume all old data,
                    // while decode doesn't gobble the rest for whatever reason.

                    *state = State::Available;
                }
                State::Available => {
                    if let Some(pos) = decoded_buf.find('\n') {
                        let remaining = decoded_buf.split_off(pos + 1);
                        let line = std::mem::replace(decoded_buf, remaining);
                        return Poll::Ready(Some(Ok(line)));
                    }
                    *state = State::PendingRead;
                }
                State::Eof => {
                    return Poll::Ready(None);
                }
            }
        }
    }
}
