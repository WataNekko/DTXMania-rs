use nom::Err;

/// Similar to [nom::combinator::opt] but for `Result<O, Err<E>>` instead of [Parser] with `IResult`
pub fn opt_err<O, E>(res: Result<O, Err<E>>) -> Result<Option<O>, Err<E>> {
    match res {
        Ok(ok) => Ok(Some(ok)),
        Err(Err::Error(_)) => Ok(None),
        Err(err) => Err(err),
    }
}
