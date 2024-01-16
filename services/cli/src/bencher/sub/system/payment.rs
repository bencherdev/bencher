#![cfg(feature = "plus")]

use std::convert::TryFrom;

use bencher_client::types::{JsonCard, JsonCustomer, JsonNewPayment};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::payment::{CliPayment, CliPaymentCard, CliPaymentCustomer},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Payment {
    pub customer: JsonCustomer,
    pub card: JsonCard,
    pub backend: AuthBackend,
}

impl TryFrom<CliPayment> for Payment {
    type Error = CliError;

    fn try_from(create: CliPayment) -> Result<Self, Self::Error> {
        let CliPayment {
            customer,
            card,
            backend,
        } = create;
        Ok(Self {
            customer: customer.into(),
            card: card.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPaymentCustomer> for JsonCustomer {
    fn from(customer: CliPaymentCustomer) -> Self {
        let CliPaymentCustomer { uuid, name, email } = customer;
        Self {
            uuid: uuid.into(),
            name: name.into(),
            email: email.into(),
        }
    }
}

impl From<CliPaymentCard> for JsonCard {
    fn from(card: CliPaymentCard) -> Self {
        let CliPaymentCard {
            number,
            exp_month,
            exp_year,
            cvc,
        } = card;
        Self {
            number: number.into(),
            exp_month: exp_month.into(),
            exp_year: exp_year.into(),
            cvc: cvc.into(),
        }
    }
}

impl From<Payment> for JsonNewPayment {
    fn from(create: Payment) -> Self {
        let Payment { customer, card, .. } = create;
        #[allow(clippy::inconsistent_struct_constructor)]
        Self { customer, card }
    }
}

impl SubCmd for Payment {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.payments_post().body(self.clone()).send().await })
            .await?;
        Ok(())
    }
}
