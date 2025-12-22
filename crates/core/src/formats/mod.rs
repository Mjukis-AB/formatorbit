//! Built-in format implementations.

mod base64;
mod binary;
mod color;
mod datetime;
mod hash;
mod hex;
mod integers;
mod ipaddr;
mod json;
mod jwt;
mod msgpack;
mod plist;
mod url;
mod utf8;
mod uuid;

pub use base64::Base64Format;
pub use binary::BinaryFormat;
pub use color::ColorFormat;
pub use datetime::DateTimeFormat;
pub use hash::HashFormat;
pub use hex::HexFormat;
pub use integers::{BytesToIntFormat, DecimalFormat};
pub use ipaddr::IpAddrFormat;
pub use json::JsonFormat;
pub use jwt::JwtFormat;
pub use msgpack::MsgPackFormat;
pub use plist::PlistFormat;
pub use url::UrlEncodingFormat;
pub use utf8::Utf8Format;
pub use uuid::UuidFormat;
