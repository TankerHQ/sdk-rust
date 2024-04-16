mod client;
pub mod request;
pub mod response;

pub use client::HttpClient;
pub type HttpRequestId = usize;
