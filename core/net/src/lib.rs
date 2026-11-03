//! A small HTTP layer for the connectors.
//!
//! The connectors build a [`Request`] and hand it to an [`Http`] implementation; they never touch
//! sockets. [`UreqHttp`] is the real client. With the `mock` feature, [`mock::MockServer`] is a
//! local server that records what arrived, so a connector's requests can be checked byte for byte
//! without any real service.

mod client;
mod multipart;
mod request;
mod url;

#[cfg(any(test, feature = "mock"))]
pub mod mock;

pub use client::UreqHttp;
pub use multipart::Multipart;
pub use request::{Body, Download, Http, Method, NetError, Request, Response};
pub use url::{check_url, encode_component, form_encode, query_string, UrlError};
