#![cfg(feature = "plus")]

use bencher_json::{
    CardCvc, CardNumber, Email, ExpirationMonth, ExpirationYear, NonEmpty, UserUuid,
};
use clap::{Args, Parser};

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
pub struct CliPayment {
    #[clap(flatten)]
    pub customer: CliPaymentCustomer,

    #[clap(flatten)]
    pub card: CliPaymentCard,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
pub struct CliPaymentCustomer {
    /// User UUID
    #[clap(long)]
    pub uuid: UserUuid,

    /// Name on card
    #[clap(long)]
    pub name: NonEmpty,

    /// User email
    #[clap(long)]
    pub email: Email,
}

#[derive(Args, Debug)]
pub struct CliPaymentCard {
    /// Card number
    #[clap(long)]
    pub number: CardNumber,

    /// Card expiration month
    #[clap(long)]
    pub exp_month: ExpirationMonth,

    /// Card expiration year
    #[clap(long)]
    pub exp_year: ExpirationYear,

    /// Card CVC
    #[clap(long)]
    pub cvc: CardCvc,
}
