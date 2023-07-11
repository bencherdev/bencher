mod codegen {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/codegen.rs"));
}

pub use codegen::*;

impl From<bencher_json::BenchmarkName> for types::BenchmarkName {
    fn from(benchmark_name: bencher_json::BenchmarkName) -> Self {
        Self(benchmark_name.into())
    }
}

impl From<bencher_json::Boundary> for types::Boundary {
    fn from(boundary: bencher_json::Boundary) -> Self {
        Self(boundary.into())
    }
}

impl From<bencher_json::BranchName> for types::BranchName {
    fn from(branch_name: bencher_json::BranchName) -> Self {
        Self(branch_name.into())
    }
}

impl From<bencher_json::Email> for types::Email {
    fn from(email: bencher_json::Email) -> Self {
        Self(email.into())
    }
}

impl From<bencher_json::GitHash> for types::GitHash {
    fn from(git_hash: bencher_json::GitHash) -> Self {
        Self(git_hash.into())
    }
}

impl From<bencher_json::Jwt> for types::Jwt {
    fn from(jwt: bencher_json::Jwt) -> Self {
        Self(jwt.into())
    }
}

impl From<bencher_json::NonEmpty> for types::NonEmpty {
    fn from(non_empty: bencher_json::NonEmpty) -> Self {
        Self(non_empty.into())
    }
}

impl From<bencher_json::ResourceId> for types::ResourceId {
    fn from(resource_id: bencher_json::ResourceId) -> Self {
        Self(resource_id.into())
    }
}

impl From<bencher_json::Slug> for types::Slug {
    fn from(slug: bencher_json::Slug) -> Self {
        Self(slug.into())
    }
}

impl From<bencher_json::Url> for types::Url {
    fn from(url: bencher_json::Url) -> Self {
        Self(url.into())
    }
}

impl From<bencher_json::UserName> for types::UserName {
    fn from(user_name: bencher_json::UserName) -> Self {
        Self(user_name.into())
    }
}

#[cfg(feature = "plus")]
mod plus {
    // CardCvc, CardNumber, ExpirationMonth, ExpirationYear, PlanLevel, PlanStatus,
    // impl From<bencher_json::CardBrand> for crate::types::CardBrand {
    //     fn from(card_brand: bencher_json::CardBrand) -> Self {
    //         Self(card_brand.into())
    //     }
    // }

    impl From<bencher_json::CardCvc> for crate::types::CardCvc {
        fn from(card_cvc: bencher_json::CardCvc) -> Self {
            Self(card_cvc.into())
        }
    }

    impl From<bencher_json::CardNumber> for crate::types::CardNumber {
        fn from(card_number: bencher_json::CardNumber) -> Self {
            Self(card_number.into())
        }
    }

    impl From<bencher_json::ExpirationMonth> for crate::types::ExpirationMonth {
        fn from(expiration_month: bencher_json::ExpirationMonth) -> Self {
            Self(expiration_month.into())
        }
    }

    impl From<bencher_json::ExpirationYear> for crate::types::ExpirationYear {
        fn from(expiration_year: bencher_json::ExpirationYear) -> Self {
            Self(expiration_year.into())
        }
    }
}
