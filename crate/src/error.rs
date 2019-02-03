use std::convert::From;
use std::fmt;
use std::string::FromUtf8Error;
use wasm_bindgen::JsValue;
use zip::result::ZipError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    LeedorError(&'static str),
    MinidomError(minidom::Error),
    StringError(FromUtf8Error),
    ZipError(ZipError),
}

impl From<&'static str> for Error {
    fn from(err: &'static str) -> Error {
        Error::LeedorError(err)
    }
}

impl From<minidom::Error> for Error {
    fn from(err: minidom::Error) -> Error {
        Error::MinidomError(err)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(err: FromUtf8Error) -> Error {
        Error::StringError(err)
    }
}

impl From<ZipError> for Error {
    fn from(err: ZipError) -> Error {
        Error::ZipError(err)
    }
}

// TODO: move to web.rs.
impl From<Error> for JsValue {
    fn from(err: Error) -> JsValue {
        JsValue::from_str(&format!("{}", err))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let error_str = match self {
            Error::LeedorError(s) => format!("LeedorError: {}", s),
            Error::MinidomError(e) => format!("MinidomError: {}", e),
            Error::StringError(e) => format!("StringError: {}", e),
            Error::ZipError(e) => format!("ZipError: {}", e),
        };
        write!(fmt, "{}", error_str)
    }
}

impl std::error::Error for Error {}
