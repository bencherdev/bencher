use dropshot::{
    HttpResponseAccepted, HttpResponseCreated, HttpResponseDeleted, HttpResponseHeaders,
    HttpResponseOk,
};

mod endpoint;
mod headers;
mod registrar;
mod remote_ip;
mod total_count;

pub use endpoint::{Delete, Endpoint, Get, Patch, Post, Put};
pub use headers::CorsHeaders;
pub use registrar::Registrar;
#[cfg(feature = "plus")]
pub use remote_ip::remote_ip;
pub use total_count::TotalCount;

pub type CorsResponse = HttpResponseHeaders<HttpResponseOk<()>, CorsHeaders>;
pub type ResponseOk<T> = HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>;
pub type ResponseCreated<T> = HttpResponseHeaders<HttpResponseCreated<T>, CorsHeaders>;
pub type ResponseAccepted<T> = HttpResponseHeaders<HttpResponseAccepted<T>, CorsHeaders>;
pub type ResponseDeleted = HttpResponseHeaders<HttpResponseDeleted, CorsHeaders>;
