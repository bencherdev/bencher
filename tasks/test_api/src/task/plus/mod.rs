use bencher_json::Jwt;

pub mod oci;
pub mod runner;

// `docker login` takes a deprecated user API token as its password, but not a user session.
const TEST_ADMIN_LEGACY_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjQzNjA5LCJpYXQiOjE2OTg2NzYzMTQsImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJldXN0YWNlLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsfQ.xumYID-R4waqhyjhcbSlwartbiRJ2AwngVkevLUBVCA";
const TEST_LEGACY_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjM2MDI0LCJpYXQiOjE2OTg2Njg3MjksImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJtdXJpZWwuYmFnZ2VAbm93aGVyZS5jb20iLCJvcmciOm51bGx9.t3t23mlgKYZmUt7-PbRWLqXlCTt6Ydh8TRE8KiSGQi4";
const DEV_ADMIN_LEGACY_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo2MDYwMDI5NDE0LCJpYXQiOjE3NjUwNjIxMTksImlzcyI6Imh0dHBzOi8vZGV2ZWwtLWJlbmNoZXIubmV0bGlmeS5hcHAvIiwic3ViIjoiZXVzdGFjZS5iYWdnZUBub3doZXJlLmNvbSIsIm9yZyI6bnVsbCwic3RhdGUiOm51bGx9.jY6749lVWe3pJ53LBXoNSl19b59xifOLdwMwQUNMZ5g";
const DEV_LEGACY_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjQyMTU2LCJpYXQiOjE2OTg2NzQ4NjEsImlzcyI6Imh0dHBzOi8vZGV2ZWwtLWJlbmNoZXIubmV0bGlmeS5hcHAvIiwic3ViIjoibXVyaWVsLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsfQ.9z7jmM53TcVzc1inDxTeX9_OR0PQPpZAsKsCE7lWHfo";

fn unwrap_admin_legacy_token(admin_token: Option<Jwt>, is_dev: bool) -> Jwt {
    admin_token.unwrap_or_else(|| {
        if is_dev {
            DEV_ADMIN_LEGACY_API_TOKEN
        } else {
            TEST_ADMIN_LEGACY_API_TOKEN
        }
        .parse()
        .expect("Invalid legacy test JWT")
    })
}

fn unwrap_user_legacy_token(user_token: Option<Jwt>, is_dev: bool) -> Jwt {
    user_token.unwrap_or_else(|| {
        if is_dev {
            DEV_LEGACY_API_TOKEN
        } else {
            TEST_LEGACY_API_TOKEN
        }
        .parse()
        .expect("Invalid legacy test JWT")
    })
}
