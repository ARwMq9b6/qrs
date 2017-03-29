//! Error

use std;
use qrcode;

#[derive(Debug, error_chain)]
pub enum ErrorKind {
    Msg(String),

    #[error_chain(custom)]
    SyncPoisonError(String),

    #[error_chain(foreign)]
    Io(std::io::Error),

    #[error_chain(foreign)]
    ParseInt(std::num::ParseIntError),

    #[error_chain(foreign)]
    StrFromUtf8(std::string::FromUtf8Error),

    #[error_chain(custom)]
    Qr(qrcode::types::QrError),
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        use std::error::Error;

        Self::from_kind(ErrorKind::SyncPoisonError(err.description().to_string()))
    }
}

impl From<qrcode::types::QrError> for Error {
    fn from(err: qrcode::types::QrError) -> Self {
        Self::from_kind(ErrorKind::Qr(err))
    }
}
