use core::ptr;
use std::{
    ffi::{c_int, c_void},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use ffmpeg_next::{Error, ffi::*, format::context};
use futures_io::AsyncRead;
use futures_lite::{AsyncReadExt, future};

pub struct Input<'a> {
    inner: context::Input,
    _io_ctx: IoContext<'a>,
}

impl<'a> Deref for Input<'a> {
    type Target = context::Input;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> DerefMut for Input<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

struct IoContext<'a> {
    ptr: *mut AVIOContext,
    _phantom: PhantomData<&'a mut &'a mut (dyn AsyncRead + Unpin)>,
}

impl<'a> Drop for IoContext<'a> {
    fn drop(&mut self) {
        unsafe {
            av_freep(&mut (*self.ptr).buffer as *mut _ as *mut c_void);
            avio_context_free(&mut self.ptr);
        }
    }
}

pub fn input_from_reader<'a>(
    reader: &'a mut &'a mut (dyn AsyncRead + Unpin),
) -> Result<Input<'a>, Error> {
    unsafe {
        const BUF_SIZE: usize = 4096;

        let io_ctx = {
            let mut buf = av_malloc(BUF_SIZE);
            if buf.is_null() {
                return Err(Error::Other { errno: ENOMEM });
            }

            let io_ctx = avio_alloc_context(
                buf as *mut u_char,
                BUF_SIZE as c_int,
                0,
                reader as *mut _ as *mut c_void,
                Some(read_packet),
                None,
                None,
            );
            if io_ctx.is_null() {
                av_freep((&mut buf) as *mut _ as *mut c_void);
                return Err(Error::Other { errno: ENOMEM });
            }

            IoContext {
                ptr: io_ctx,
                _phantom: PhantomData,
            }
        };

        let mut fmt_ctx = avformat_alloc_context();
        if fmt_ctx.is_null() {
            return Err(Error::Other { errno: ENOMEM });
        }
        (*fmt_ctx).pb = io_ctx.ptr;

        match avformat_open_input(&mut fmt_ctx, ptr::null(), ptr::null(), ptr::null_mut()) {
            0 => {
                let mut fmt_ctx = context::Input::wrap(fmt_ctx);

                match avformat_find_stream_info(fmt_ctx.as_mut_ptr(), ptr::null_mut()) {
                    e if e < 0 => Err(Error::from(e)),
                    _ => Ok(Input {
                        inner: fmt_ctx,
                        _io_ctx: io_ctx,
                    }),
                }
            }
            e => Err(Error::from(e)),
        }
    }
}

unsafe extern "C" fn read_packet(opaque: *mut c_void, buf: *mut u8, buf_size: c_int) -> c_int {
    let reader = unsafe { &mut *(opaque as *mut &mut (dyn AsyncRead + Unpin)) };
    let buf = unsafe { std::slice::from_raw_parts_mut(buf, buf_size as usize) };

    match future::block_on(reader.read(buf)) {
        Ok(0) => AVERROR_EOF,
        Ok(n) => n as c_int,
        Err(err) => err
            .raw_os_error()
            .map(|e| AVERROR(e.abs()))
            .unwrap_or(AVERROR_UNKNOWN),
    }
}
