use thiserror::Error;

use crate::{Boundary, SampleSize, Window};

pub(crate) const REGEX_ERROR: &str = "Failed to compile regex.";

#[derive(Debug, Error)]
pub enum ValidError {
    #[error("Failed to validate resource ID: {0}")]
    ResourceId(String),
    #[error("Failed to validate user name: {0}")]
    UserName(String),
    #[error("Failed to validate slug: {0}")]
    Slug(String),
    #[error("Failed to validate email: {0}")]
    Email(String),
    #[error("Failed to validate date time: {0}")]
    DateTime(i64),
    #[error("Failed to parse date time: {0}")]
    DateTimeStr(std::num::ParseIntError),
    #[error("Failed to validate date time millis: {0}")]
    DateTimeMillis(i64),
    #[error("Failed to validate JWT (JSON Web Token): {0}")]
    Jwt(String),
    #[error("Failed to validate branch name: {0}")]
    BranchName(String),
    #[error("Failed to validate benchmark name: {0}")]
    BenchmarkName(String),
    #[error("Failed to validate name ID: {0}")]
    NameId(String),
    #[error("Failed to validate non-empty ID: {0}")]
    NonEmpty(String),
    #[error("Failed to validate resource name string: {0}")]
    ResourceName(String),
    #[error("Failed to validate URL: {0}")]
    Url(String),
    #[error("Failed to translate internal URL ({0}): {1}")]
    UrlToUrl(crate::Url, url::ParseError),
    #[error("Failed to validate git hash: {0}")]
    GitHash(String),
    #[error("Failed to validate secret: {0}")]
    Secret(String),
    #[error("Invalid model boundary: {0}")]
    Boundary(f64),
    #[error("Failed to parse model boundary: {0}")]
    BoundaryStr(std::num::ParseFloatError),
    #[error("Invalid model sample size: {0}")]
    SampleSize(u32),
    #[error("Failed to parse model sample size: {0}")]
    SampleSizeStr(std::num::ParseIntError),
    #[error("Invalid model window: {0}")]
    Window(u32),
    #[error("Failed to parse model window: {0}")]
    WindowStr(std::num::ParseIntError),
    #[error("Invalid plot index: {0}")]
    Index(u8),
    #[error("Failed to parse plot index: {0}")]
    IndexStr(std::num::ParseIntError),

    #[cfg(feature = "plus")]
    #[error("Failed to validate plan level: {0}")]
    PlanLevel(String),
    #[cfg(feature = "plus")]
    #[error("Failed to validate plan status: {0}")]
    PlanStatus(String),
    #[cfg(feature = "plus")]
    #[error("Failed to validate payment card brand: {0}")]
    CardBrand(String),
    #[cfg(feature = "plus")]
    #[error("Failed to validate payment card number: {0}")]
    CardNumber(String),
    #[cfg(feature = "plus")]
    #[error("Failed to validate payment card last four numbers: {0}")]
    LastFour(String),
    #[cfg(feature = "plus")]
    #[error("Failed to validate payment card expiration year: {0}")]
    ExpirationYear(i32),
    #[cfg(feature = "plus")]
    #[error("Failed to convert payment card expiration year: {0}")]
    ExpirationYear64(std::num::TryFromIntError),
    #[cfg(feature = "plus")]
    #[error("Failed to validate payment card expiration year: {0}")]
    ExpirationYearStr(String),
    #[cfg(feature = "plus")]
    #[error("Failed to validate payment card expiration month: {0}")]
    ExpirationMonth(i32),
    #[cfg(feature = "plus")]
    #[error("Failed to convert payment card expiration month: {0}")]
    ExpirationMonth64(std::num::TryFromIntError),
    #[cfg(feature = "plus")]
    #[error("Failed to validate payment card expiration month: {0}")]
    ExpirationMonthStr(String),
    #[cfg(feature = "plus")]
    #[error("Failed to validate payment card CVC: {0}")]
    CardCvc(String),
    #[cfg(feature = "plus")]
    #[error("Failed to validate entitlements: {0}")]
    Entitlements(u32),
    #[cfg(feature = "plus")]
    #[error("Failed to parse entitlements: {0}")]
    EntitlementsStr(std::num::ParseIntError),

    #[error(
        "Invalid model, minimum sample size ({min}) is greater than maximum sample size ({max})"
    )]
    SampleSizes { min: SampleSize, max: SampleSize },
    #[error("Invalid model, lower boundary ({lower}) is greater than upper boundary ({upper})")]
    Boundaries { lower: Boundary, upper: Boundary },
    #[error("Invalid model, no boundary provided")]
    NoBoundary,
    #[error("Invalid static model, includes a minimum sample size: {0}")]
    StaticMinSampleSize(SampleSize),
    #[error("Invalid static model, includes a maximum sample size: {0}")]
    StaticMaxSampleSize(SampleSize),
    #[error("Invalid static model, includes a sampling window: {0}")]
    StaticWindow(Window),
    #[error("Invalid percentage boundary: {0}")]
    PercentageBoundary(f64),
    #[error("Invalid statistical boundary: {0}")]
    CdfBoundary(f64),
    #[error("Invalid inter-quartile range boundary: {0}")]
    IqrBoundary(f64),
}
