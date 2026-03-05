use nom::{
    Err, Parser,
    error::{Error, ErrorKind},
};

/// Similar to [nom::combinator::opt] but for `Result<O, Err<E>>` instead of [Parser] with `IResult`
pub fn opt_err<O, E>(res: Result<O, Err<E>>) -> Result<Option<O>, Err<E>> {
    match res {
        Ok(ok) => Ok(Some(ok)),
        Err(Err::Error(_)) => Ok(None),
        Err(err) => Err(err),
    }
}

/// Convert all [Err::Error] into [Err::Failure], except [ErrorKind::Eof]. Useful for [nom::combinator::ParserIterator].
pub fn cut_not_eof<I, O>(
    mut parser: impl Parser<I, Output = O, Error = Error<I>>,
) -> impl Parser<I, Output = O, Error = Error<I>> {
    move |input| {
        parser.parse(input).map_err(|err| match err {
            Err::Error(e) => {
                if e.code == ErrorKind::Eof {
                    Err::Error(e)
                } else {
                    Err::Failure(e)
                }
            }
            err => err,
        })
    }
}
