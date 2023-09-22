use thiserror::Error;

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
    #[error("Failed to validate JWT (JSON Web Token): {0}")]
    Jwt(String),
    #[error("Failed to validate branch name: {0}")]
    BranchName(String),
    #[error("Failed to validate benchmark name: {0}")]
    BenchmarkName(String),
    #[error("Failed to validate non-empty string: {0}")]
    NonEmpty(String),
    #[error("Failed to validate URL: {0}")]
    Url(String),
    #[error("Failed to translate internal URL ({0}): {1}")]
    UrlToUrl(crate::Url, url::ParseError),
    #[error("Failed to validate git hash: {0}")]
    GitHash(String),
    #[error("Failed to validate secret: {0}")]
    Secret(String),
    #[error("Failed to validate plan level: {0}")]
    PlanLevel(String),
    #[error("Failed to validate plan status: {0}")]
    PlanStatus(String),
    #[error("Failed to validate payment card brand: {0}")]
    CardBrand(String),
    #[error("Failed to validate payment card number: {0}")]
    CardNumber(String),
    #[error("Failed to validate payment card last four numbers: {0}")]
    LastFour(String),
    #[error("Failed to validate payment card expiration year: {0}")]
    ExpirationYear(i32),
    #[error("Failed to validate payment card expiration year: {0}")]
    ExpirationYearStr(String),
    #[error("Failed to validate payment card expiration month: {0}")]
    ExpirationMonth(i32),
    #[error("Failed to validate payment card expiration month: {0}")]
    ExpirationMonthStr(String),
    #[error("Failed to cast in: {0}")]
    IntError(#[from] std::num::TryFromIntError),
    #[error("Failed to validate payment card CVC: {0}")]
    CardCvc(String),
    #[error("Invalid statistical boundary: {0}")]
    Boundary(f64),
    #[error("Invalid statistical sample size: {0}")]
    SampleSize(u32),
}
