#![cfg(feature = "plus")]

use bencher_valid::{CardCvc, CardNumber, Email, ExpirationMonth, ExpirationYear, NonEmpty};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::UserUuid;

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewPayment {
    pub customer: JsonCustomer,
    pub card: JsonCard,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCustomer {
    pub uuid: UserUuid,
    pub name: NonEmpty,
    pub email: Email,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCard {
    pub number: CardNumber,
    pub exp_month: ExpirationMonth,
    pub exp_year: ExpirationYear,
    pub cvc: CardCvc,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPayment {
    pub customer: NonEmpty,
    pub payment_method: NonEmpty,
}
