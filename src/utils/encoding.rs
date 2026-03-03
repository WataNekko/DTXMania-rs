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
        // SAFETY: This type ensures self.pos is never incremented further than self.buf.len().
        unsafe { self.buf.get_unchecked(self.pos..) }
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
    ) -> impl Future<Output = io::Result<()>> + 'a {
        ReadToStringFuture { reader: self, buf }
    }
}

struct ReadToStringFuture<'a, R> {
    reader: &'a mut DecodedBufRead<R>,
    buf: &'a mut String,
}

impl<R: AsyncBufRead + Unpin> Future for ReadToStringFuture<'_, R> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, buf } = &mut *self;

        loop {
            let filled = ready!(reader.poll_fill_buf(cx))?;
            if filled.is_empty() {
                break;
            }

            buf.push_str(filled);

            let len = filled.len();
            reader.consume(len);
        }
        Poll::Ready(Ok(()))
    }
}
