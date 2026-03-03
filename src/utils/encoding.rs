use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use bevy::tasks::futures_lite::AsyncBufRead;
use bevy::{prelude::*, tasks::futures_lite::ready};
use encoding_rs::{CoderResult, Decoder, Encoding};

pub trait AsyncBufReadEncodingExt: AsyncBufRead + Sized {
    fn with_encoding(self, encoding: &'static Encoding) -> DecodedBufRead<Self>;
}

impl<R: AsyncBufRead> AsyncBufReadEncodingExt for R {
    fn with_encoding(self, encoding: &'static Encoding) -> DecodedBufRead<Self> {
        DecodedBufRead {
            inner: self,
            decoder: encoding.new_decoder(),
            buf: String::new(),
            pos: 0,
            finalized_decoder: false,
        }
    }
}

pub struct DecodedBufRead<R> {
    inner: R,
    decoder: Decoder,
    buf: String,
    pos: usize,
    finalized_decoder: bool,
}

impl<R: AsyncBufRead + Unpin> DecodedBufRead<R> {
    pub fn buffer(&self) -> &str {
        &self.buf[self.pos..]
    }

    pub fn poll_fill_buf(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<&str>> {
        if self.pos >= self.buf.len() {
            let chunk = ready!(Pin::new(&mut self.inner).poll_fill_buf(cx))?;

            if !chunk.is_empty() || !self.finalized_decoder {
                // Decode if received data, or finalize the decoder (flushing internal states)
                self.pos = 0;
                self.buf.clear();
                self.finalized_decoder = chunk.is_empty();

                let max_len = self.decoder.max_utf8_buffer_length(chunk.len()).unwrap();
                self.buf.reserve(max_len);

                let (res, read, err) =
                    self.decoder
                        .decode_to_string(chunk, &mut self.buf, self.finalized_decoder);

                debug_assert_ne!(
                    res,
                    CoderResult::OutputFull,
                    "Reserving max required length before decoding should have ensured result is never OutputFull."
                );
                debug_assert_eq!(
                    read,
                    chunk.len(),
                    "Decoder should have read everything since output buf is large enough."
                );
                if err {
                    warn!(
                        "Encountered invalid sequence while decoding {}.",
                        self.decoder.encoding().name()
                    );
                }

                let chunk_len = chunk.len();
                Pin::new(&mut self.inner).consume(chunk_len);
            }
        }
        Poll::Ready(Ok(self.buffer()))
    }

    pub fn consume(&mut self, amt: usize) {
        self.pos = std::cmp::min(self.pos + amt, self.buf.len());
    }

    pub fn read_to_string<'a>(
        &'a mut self,
        buf: &'a mut String,
    ) -> impl Future<Output = io::Result<usize>> + 'a {
        ReadToStringFuture {
            reader: self,
            buf,
            total_read: 0,
        }
    }

    pub fn read_line<'a>(
        &'a mut self,
        buf: &'a mut String,
    ) -> impl Future<Output = io::Result<usize>> + 'a {
        ReadLineFuture {
            reader: self,
            buf,
            total_read: 0,
        }
    }
}

struct ReadToStringFuture<'a, R> {
    reader: &'a mut DecodedBufRead<R>,
    buf: &'a mut String,
    total_read: usize,
}

impl<R: AsyncBufRead + Unpin> Future for ReadToStringFuture<'_, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            buf,
            total_read,
        } = &mut *self;

        loop {
            let filled = ready!(reader.poll_fill_buf(cx))?;
            if filled.is_empty() {
                break;
            }

            buf.push_str(filled);

            let len = filled.len();
            reader.consume(len);
            *total_read += len;
        }
        Poll::Ready(Ok(*total_read))
    }
}

struct ReadLineFuture<'a, R> {
    reader: &'a mut DecodedBufRead<R>,
    buf: &'a mut String,
    total_read: usize,
}

impl<R: AsyncBufRead + Unpin> Future for ReadLineFuture<'_, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            buf,
            total_read,
        } = &mut *self;

        loop {
            let filled = ready!(reader.poll_fill_buf(cx))?;
            if filled.is_empty() {
                break;
            }

            let (read, found_new_line) = filled
                .find('\n')
                .map(|pos| (pos + 1, true))
                .unwrap_or_else(|| (filled.len(), false));

            buf.push_str(&filled[..read]);
            reader.consume(read);
            *total_read += read;

            if found_new_line {
                break;
            }
        }
        Poll::Ready(Ok(*total_read))
    }
}

#[cfg(test)]
mod test {
    use std::{char::REPLACEMENT_CHARACTER, task::Waker};

    use bevy::tasks::futures_lite::{FutureExt, io::BufReader};
    use encoding_rs::{UTF_8, UTF_16LE};

    use super::*;

    fn expect_no_io<T>(result: Result<T, io::Error>) -> T {
        result.expect("No IO error possible")
    }

    #[test]
    fn fill_buf_utf8() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let mut reader = b"Hello world".with_encoding(UTF_8);

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready("Hello world"),
            "Should return the full buffer"
        );

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready("Hello world"),
            "Should still return full buffer since nothing is consumed"
        );

        reader.consume("Hello world".len());

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready(""),
            "EOF"
        );
    }

    #[test]
    fn partial_consume() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let mut reader = b"Hello world".with_encoding(UTF_8);

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready("Hello world"),
            "Should return the full buffer"
        );

        reader.consume("Hello ".len());

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready("world"),
            "Buffer consumed partially"
        );

        reader.consume("world".len());

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready(""),
            "EOF"
        );
    }

    #[test]
    fn small_buf() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let mut reader =
            BufReader::with_capacity(4, b"Hello world".as_slice()).with_encoding(UTF_8);

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready("Hell"),
            "Fill up to the inner buffer capacity"
        );

        reader.consume(4);

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready("o wo"),
            "Fill up to the inner buffer capacity"
        );

        reader.consume(4);

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready("rld"),
            "Fill up to the inner buffer capacity"
        );

        reader.consume(4);

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready(""),
            "EOF"
        );
    }

    #[test]
    fn fill_buf_utf16() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let encoded: Vec<u8> = "Hello world UTF16"
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let mut reader = encoded.with_encoding(UTF_16LE);

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready("Hello world UTF16"),
            "Return full decoded buffer"
        );

        reader.consume("Hello world UTF16".len());

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready(""),
            "EOF"
        );
    }

    #[test]
    fn fill_buf_utf16_malformed() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let encoded: Vec<u8> = "Hello world UTF16"
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let (_, malformed_encoded) = encoded.split_last().unwrap();
        let mut reader = malformed_encoded.with_encoding(UTF_16LE);

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready("Hello world UTF1"),
            "Decoded up to before the last malformed UTF16 sequence"
        );

        reader.consume("Hello world UTF1".len());

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready(REPLACEMENT_CHARACTER.to_string().as_str()),
            "The final malformed sequence is replaced with the replacement character"
        );

        reader.consume(REPLACEMENT_CHARACTER.len_utf8());

        assert_eq!(
            reader.poll_fill_buf(&mut cx).map(expect_no_io),
            Poll::Ready(""),
            "EOF"
        );
    }

    #[test]
    fn read_to_string() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let mut reader = b"Hello world".with_encoding(UTF_8);

        let expected = "Hello world".to_string();

        let mut buf = String::new();
        assert_eq!(
            reader
                .read_to_string(&mut buf)
                .poll(&mut cx)
                .map(expect_no_io),
            Poll::Ready(expected.len())
        );
        assert_eq!(buf, expected);
    }

    #[test]
    fn read_to_string_malformed() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let encoded: Vec<u8> = "Hello world UTF16"
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let (_, malformed_encoded) = encoded.split_last().unwrap();
        let mut reader = malformed_encoded.with_encoding(UTF_16LE);

        let expected = format!("Hello world UTF1{}", REPLACEMENT_CHARACTER);

        let mut buf = String::new();
        assert_eq!(
            reader
                .read_to_string(&mut buf)
                .poll(&mut cx)
                .map(expect_no_io),
            Poll::Ready(expected.len())
        );
        assert_eq!(buf, expected);
    }

    #[test]
    fn read_line() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let mut reader = b"Never gonna give you up\n\
            Never gonna let you down\n\
            Never gonna run around and desert you"
            .with_encoding(UTF_8);

        let mut buf = String::new();

        for expected in [
            "Never gonna give you up\n",
            "Never gonna let you down\n",
            "Never gonna run around and desert you",
            "",
        ] {
            buf.clear();

            assert_eq!(
                reader.read_line(&mut buf).poll(&mut cx).map(expect_no_io),
                Poll::Ready(expected.len())
            );
            assert_eq!(buf, expected.to_string());
        }
    }
}
