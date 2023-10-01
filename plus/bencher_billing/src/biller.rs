use bencher_json::{
    organization::metered::{JsonCard, JsonCardDetails, JsonCustomer, JsonPlan},
    system::config::JsonBilling,
    Email, PlanLevel, PlanStatus, UserName,
};
use chrono::{TimeZone, Utc};
use stripe::{
    AttachPaymentMethod, CardDetailsParams as PaymentCard, Client as StripeClient, CreateCustomer,
    CreatePaymentMethod, CreatePaymentMethodCardUnion, CreateSubscription, CreateSubscriptionItems,
    CreateUsageRecord, Customer, Expandable, ListCustomers, PaymentMethod, PaymentMethodTypeFilter,
    Subscription, SubscriptionId, SubscriptionItem, SubscriptionStatus, UsageRecord,
};
use uuid::Uuid;

use crate::{products::Products, BillingError};

// Metrics are bundled by the thousand
const METRIC_QUANTITY: u64 = 1_000;

const METADATA_UUID: &str = "uuid";
const METADATA_ORGANIZATION: &str = "organization";

pub struct Biller {
    client: StripeClient,
    products: Products,
}

#[derive(Debug, Clone)]
enum ProductPlan {
    Free,
    Team(ProductUsage),
    Enterprise(ProductUsage),
}

#[derive(Debug, Clone)]
enum ProductUsage {
    Metered(String),
    Licensed(String, u64),
}

impl ProductPlan {
    fn metered(plan_level: PlanLevel, price_name: String) -> Self {
        match plan_level {
            PlanLevel::Free => Self::Free,
            PlanLevel::Team => Self::Team(ProductUsage::Metered(price_name)),
            PlanLevel::Enterprise => Self::Enterprise(ProductUsage::Metered(price_name)),
        }
    }

    fn licensed(plan_level: PlanLevel, price_name: String, quantity: u64) -> Self {
        match plan_level {
            PlanLevel::Free => Self::Free,
            PlanLevel::Team => Self::Team(ProductUsage::Licensed(price_name, quantity)),
            PlanLevel::Enterprise => Self::Enterprise(ProductUsage::Licensed(price_name, quantity)),
        }
    }
}

impl Biller {
    pub async fn new(billing: JsonBilling) -> Result<Self, BillingError> {
        let JsonBilling {
            secret_key,
            products,
        } = billing;
        let client = StripeClient::new(secret_key);
        let products = Products::new(&client, products).await?;

        Ok(Self { client, products })
    }

    pub async fn get_or_create_customer(
        &self,
        name: &UserName,
        email: &Email,
        uuid: Uuid,
    ) -> Result<Customer, BillingError> {
        if let Some(customer) = self.get_customer(email).await? {
            Ok(customer)
        } else {
            self.create_customer(name, email, uuid).await
        }
    }

    pub async fn get_customer(&self, email: &Email) -> Result<Option<Customer>, BillingError> {
        let list_customers = ListCustomers {
            email: Some(email.as_ref()),
            ..Default::default()
        };
        let mut customers = Customer::list(&self.client, &list_customers).await?;

        if let Some(customer) = customers.data.pop() {
            if customers.data.is_empty() {
                Ok(Some(customer))
            } else {
                Err(BillingError::EmailCollision(customer, customers.data))
            }
        } else {
            Ok(None)
        }
    }

    // WARNING: Use caution when calling this directly as multiple users with the same email can be created
    // Use `get_or_create_customer` instead!
    async fn create_customer(
        &self,
        name: &UserName,
        email: &Email,
        uuid: Uuid,
    ) -> Result<Customer, BillingError> {
        let create_customer = CreateCustomer {
            name: Some(name.as_ref()),
            email: Some(email.as_ref()),
            metadata: Some(
                [(METADATA_UUID.into(), uuid.to_string())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        };
        Customer::create(&self.client, create_customer)
            .await
            .map_err(Into::into)
    }

    // pub async fn get_or_create_payment_method(
    //     &self,
    //     customer: &Customer,
    //     payment_card: PaymentCard,
    // ) -> Result<PaymentMethod, BillingError> {
    //     if let Some(payment_method) = self.get_payment_method(customer).await? {
    //         Ok(payment_method)
    //     } else {
    //         self.create_payment_method(customer, payment_card).await
    //     }
    // }

    // pub async fn get_payment_method(
    //     &self,
    //     customer: &Customer,
    // ) -> Result<Option<PaymentMethod>, BillingError> {
    //     let list_payment_methods = ListPaymentMethods {
    //         type_: Some(PaymentMethodTypeFilter::Card),
    //         customer: Some(customer.id.clone()),
    //         ..Default::default()
    //     };
    //     let mut payment_methods = PaymentMethod::list(&self.client, &list_payment_methods).await?;

    //     if let Some(payment_method) = payment_methods.data.pop() {
    //         if payment_methods.data.is_empty() {
    //             Ok(Some(payment_method))
    //         } else {
    //             Err(BillingError::MultiplePaymentMethods(
    //                 payment_method,
    //                 payment_methods.data,
    //             ))
    //         }
    //     } else {
    //         Ok(None)
    //     }
    // }

    // WARNING: Use caution when calling this directly as multiple payment methods can be created
    pub async fn create_payment_method(
        &self,
        customer: &Customer,
        json_card: JsonCard,
    ) -> Result<PaymentMethod, BillingError> {
        let create_payment_method = CreatePaymentMethod {
            type_: Some(PaymentMethodTypeFilter::Card),
            card: Some(CreatePaymentMethodCardUnion::CardDetailsParams(
                into_payment_card(json_card),
            )),
            ..Default::default()
        };
        let payment_method = PaymentMethod::create(&self.client, create_payment_method).await?;

        PaymentMethod::attach(
            &self.client,
            &payment_method.id,
            AttachPaymentMethod {
                customer: customer.id.clone(),
            },
        )
        .await
        .map_err(Into::into)
    }

    // pub async fn get_or_create_subscription(
    //     &self,
    //     organization: Uuid,
    //     customer: &Customer,
    //     payment_method: &PaymentMethod,
    //     product_plan: ProductPlan,
    // ) -> Result<Subscription, BillingError> {
    //     if let Some(subscription) = self.get_subscription(organization, customer).await? {
    //         Ok(subscription)
    //     } else {
    //         self.create_subscription(organization, customer, payment_method, product_plan)
    //             .await
    //     }
    // }

    pub async fn create_metered_subscription(
        &self,
        organization: Uuid,
        customer: &Customer,
        payment_method: &PaymentMethod,
        plan_level: PlanLevel,
        price_name: String,
    ) -> Result<Subscription, BillingError> {
        self.create_subscription(
            organization,
            customer,
            payment_method,
            ProductPlan::metered(plan_level, price_name),
        )
        .await
    }

    pub async fn create_licensed_subscription(
        &self,
        organization: Uuid,
        customer: &Customer,
        payment_method: &PaymentMethod,
        plan_level: PlanLevel,
        price_name: String,
        quantity: u64,
    ) -> Result<Subscription, BillingError> {
        self.create_subscription(
            organization,
            customer,
            payment_method,
            ProductPlan::licensed(plan_level, price_name, quantity),
        )
        .await
    }

    // WARNING: Use caution when calling this directly as multiple subscriptions can be created
    async fn create_subscription(
        &self,
        organization: Uuid,
        customer: &Customer,
        payment_method: &PaymentMethod,
        product_plan: ProductPlan,
    ) -> Result<Subscription, BillingError> {
        let mut create_subscription = CreateSubscription::new(customer.id.clone());
        let (price, quantity) = match product_plan {
            ProductPlan::Free => return Err(BillingError::ProductLevelFree),
            ProductPlan::Team(product_usage) => match product_usage {
                ProductUsage::Metered(price_name) => (
                    self.products
                        .team
                        .metered
                        .get(&price_name)
                        .ok_or(BillingError::PriceNotFound(price_name))?,
                    None,
                ),
                ProductUsage::Licensed(price_name, quantity) => (
                    self.products
                        .team
                        .licensed
                        .get(&price_name)
                        .ok_or(BillingError::PriceNotFound(price_name))?,
                    Some(quantity),
                ),
            },
            ProductPlan::Enterprise(product_usage) => match product_usage {
                ProductUsage::Metered(price_name) => (
                    self.products
                        .enterprise
                        .metered
                        .get(&price_name)
                        .ok_or(BillingError::PriceNotFound(price_name))?,
                    None,
                ),
                ProductUsage::Licensed(price_name, quantity) => (
                    self.products
                        .enterprise
                        .licensed
                        .get(&price_name)
                        .ok_or(BillingError::PriceNotFound(price_name))?,
                    Some(quantity),
                ),
            },
        };

        let quantity = if let Some(quantity) = quantity {
            if quantity == 0 {
                return Err(BillingError::QuantityZero(quantity));
            }
            Some(quantity * METRIC_QUANTITY)
        } else {
            None
        };

        create_subscription.items = Some(vec![CreateSubscriptionItems {
            price: Some(price.id.to_string()),
            quantity,
            ..Default::default()
        }]);
        create_subscription.default_payment_method = Some(&payment_method.id);
        create_subscription.metadata = Some(
            [(METADATA_ORGANIZATION.to_owned(), organization.to_string())]
                .into_iter()
                .collect(),
        );

        Subscription::create(&self.client, create_subscription)
            .await
            .map_err(Into::into)
    }

    pub async fn get_subscription(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<Subscription, BillingError> {
        self.get_subscription_expand(subscription_id, &[]).await
    }

    pub async fn get_subscription_expand(
        &self,
        subscription_id: &SubscriptionId,
        expand: &[&str],
    ) -> Result<Subscription, BillingError> {
        Subscription::retrieve(&self.client, subscription_id, expand)
            .await
            .map_err(Into::into)
    }

    pub async fn get_plan(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<JsonPlan, BillingError> {
        let subscription = self
            .get_subscription_expand(
                subscription_id,
                &[
                    "customer",
                    "default_payment_method",
                    "items",
                    "items.data.price.product",
                ],
            )
            .await?;

        let Some(organization) = subscription.metadata.get(METADATA_ORGANIZATION) else {
            return Err(BillingError::NoOrganization(subscription_id.clone()));
        };
        let organization = organization.parse()?;

        let current_period_start = Utc
            .timestamp_opt(subscription.current_period_start, 0)
            .single()
            .ok_or_else(|| {
                BillingError::DateTime(subscription_id.clone(), subscription.current_period_start)
            })?;
        let current_period_end = Utc
            .timestamp_opt(subscription.current_period_end, 0)
            .single()
            .ok_or_else(|| {
                BillingError::DateTime(subscription_id.clone(), subscription.current_period_end)
            })?;

        let customer = Self::get_plan_customer(&subscription.customer)?;
        let card = Self::get_plan_card(subscription_id, &subscription.default_payment_method)?;
        let (level, unit_amount) = Self::get_plan_price(subscription_id, subscription.items.data)?;

        let status = Self::map_status(subscription.status);

        Ok(JsonPlan {
            organization,
            customer,
            card,
            level,
            unit_amount: unit_amount.into(),
            current_period_start,
            current_period_end,
            status,
        })
    }

    fn get_plan_customer(customer: &Expandable<Customer>) -> Result<JsonCustomer, BillingError> {
        let Some(customer) = customer.as_object() else {
            return Err(BillingError::NoCustomerInfo(customer.id()));
        };
        let Some(uuid) = customer
            .metadata
            .as_ref()
            .and_then(|meta| meta.get(METADATA_UUID))
        else {
            return Err(BillingError::NoUuid(customer.id.clone()));
        };
        let Some(name) = &customer.name else {
            return Err(BillingError::NoName(customer.id.clone()));
        };
        let Some(email) = &customer.email else {
            return Err(BillingError::NoEmail(customer.id.clone()));
        };
        Ok(JsonCustomer {
            uuid: uuid.parse()?,
            name: name.parse()?,
            email: email.parse()?,
        })
    }

    fn get_plan_card(
        subscription_id: &SubscriptionId,
        default_payment_method: &Option<Expandable<PaymentMethod>>,
    ) -> Result<JsonCardDetails, BillingError> {
        let Some(default_payment_method) = default_payment_method else {
            return Err(BillingError::NoDefaultPaymentMethod(
                subscription_id.clone(),
            ));
        };
        let Some(default_payment_method_info) = default_payment_method.as_object() else {
            return Err(BillingError::NoDefaultPaymentMethodInfo(
                default_payment_method.id(),
            ));
        };
        let Some(card_details) = &default_payment_method_info.card else {
            return Err(BillingError::NoCardDetails(default_payment_method.id()));
        };
        Ok(JsonCardDetails {
            brand: card_details.brand.parse()?,
            last_four: card_details.last4.parse()?,
            exp_month: card_details.exp_month.try_into()?,
            exp_year: card_details.exp_year.try_into()?,
        })
    }

    fn get_plan_price(
        subscription_id: &SubscriptionId,
        subscription_items: Vec<SubscriptionItem>,
    ) -> Result<(PlanLevel, u64), BillingError> {
        let subscription_item = Self::get_subscription_item(subscription_id, subscription_items)?;
        let Some(price) = subscription_item.price else {
            return Err(BillingError::NoPrice(subscription_item.id));
        };

        let Some(unit_amount) = price.unit_amount else {
            return Err(BillingError::NoUnitAmount(price.id));
        };
        let unit_amount = u64::try_from(unit_amount)?;

        let Some(product) = price.product else {
            return Err(BillingError::NoProduct(price.id));
        };
        let Some(product_info) = product.as_object() else {
            return Err(BillingError::NoProductInfo(product.id()));
        };
        // `Bencher Team` or `Bencher Enterprise`
        let Some(product_name) = &product_info.name else {
            return Err(BillingError::NoProductName(product.id()));
        };
        let plan_level = product_name.parse()?;

        Ok((plan_level, unit_amount))
    }

    fn get_subscription_item(
        subscription_id: &SubscriptionId,
        mut subscription_items: Vec<SubscriptionItem>,
    ) -> Result<SubscriptionItem, BillingError> {
        if let Some(subscription_item) = subscription_items.pop() {
            if subscription_items.is_empty() {
                Ok(subscription_item)
            } else {
                Err(BillingError::MultipleSubscriptionItems(
                    subscription_id.clone(),
                    subscription_item,
                    subscription_items,
                ))
            }
        } else {
            Err(BillingError::NoSubscriptionItem(subscription_id.clone()))
        }
    }

    pub async fn get_plan_status(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<PlanStatus, BillingError> {
        let subscription = self.get_subscription(subscription_id).await?;
        Ok(Self::map_status(subscription.status))
    }

    fn map_status(status: SubscriptionStatus) -> PlanStatus {
        match status {
            SubscriptionStatus::Active => PlanStatus::Active,
            SubscriptionStatus::Canceled => PlanStatus::Canceled,
            SubscriptionStatus::Incomplete => PlanStatus::Incomplete,
            SubscriptionStatus::IncompleteExpired => PlanStatus::IncompleteExpired,
            SubscriptionStatus::PastDue => PlanStatus::PastDue,
            SubscriptionStatus::Paused => PlanStatus::Paused,
            SubscriptionStatus::Trialing => PlanStatus::Trialing,
            SubscriptionStatus::Unpaid => PlanStatus::Unpaid,
        }
    }

    pub async fn record_usage(
        &self,
        subscription_id: &SubscriptionId,
        quantity: u64,
    ) -> Result<UsageRecord, BillingError> {
        let subscription = self.get_subscription(subscription_id).await?;
        let subscription_item =
            Self::get_subscription_item(&subscription.id, subscription.items.data)?;

        let create_usage_record = CreateUsageRecord {
            quantity,
            ..Default::default()
        };
        UsageRecord::create(&self.client, &subscription_item.id, create_usage_record)
            .await
            .map_err(Into::into)
    }
}

fn into_payment_card(card: JsonCard) -> PaymentCard {
    let JsonCard {
        number,
        exp_month,
        exp_year,
        cvc,
    } = card;
    PaymentCard {
        number: number.into(),
        exp_month: exp_month.into(),
        exp_year: exp_year.into(),
        cvc: Some(cvc.into()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use bencher_json::{
        organization::metered::{JsonCard, DEFAULT_PRICE_NAME},
        system::config::{JsonBilling, JsonProduct, JsonProducts},
        PlanLevel,
    };
    use chrono::{Datelike, Utc};
    use literally::hmap;
    use pretty_assertions::assert_eq;
    use stripe::{Customer, PaymentMethod, SubscriptionId};
    use uuid::Uuid;

    use crate::Biller;

    const TEST_BILLING_KEY: &str = "TEST_BILLING_KEY";

    fn test_billing_key() -> Option<String> {
        std::env::var(TEST_BILLING_KEY).ok()
    }

    fn test_products() -> JsonProducts {
        JsonProducts {
            team: JsonProduct {
                id: "prod_NKz5B9dGhDiSY1".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1McW12Kal5vzTlmhoPltpBAW".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1MaJ7kKal5vzTlmh1pbQ5JYR".to_owned(),
                },
            },
            enterprise: JsonProduct {
                id: "prod_NLC7fDet2C8Nmk".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1McW2eKal5vzTlmhECLIyVQz".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1MaViyKal5vzTlmho1MdXIpe".to_owned(),
                },
            },
        }
    }

    async fn test_metered_subscription(
        biller: &Biller,
        organization: Uuid,
        customer: &Customer,
        payment_method: &PaymentMethod,
        plan_level: PlanLevel,
        price_name: String,
        usage_count: usize,
    ) {
        let create_subscription = biller
            .create_metered_subscription(
                organization,
                customer,
                payment_method,
                plan_level,
                price_name,
            )
            .await
            .unwrap();

        let get_subscription = biller
            .get_subscription(&create_subscription.id)
            .await
            .unwrap();
        assert_eq!(create_subscription.id, get_subscription.id);

        test_record_usage(biller, &create_subscription.id, usage_count).await;

        biller.get_plan(&create_subscription.id).await.unwrap();
    }

    async fn test_licensed_subscription(
        biller: &Biller,
        organization: Uuid,
        customer: &Customer,
        payment_method: &PaymentMethod,
        plan_level: PlanLevel,
        price_name: String,
        quantity: u64,
    ) {
        let create_subscription = biller
            .create_licensed_subscription(
                organization,
                customer,
                payment_method,
                plan_level,
                price_name,
                quantity,
            )
            .await
            .unwrap();
        let get_subscription = biller
            .get_subscription(&create_subscription.id)
            .await
            .unwrap();
        assert_eq!(create_subscription.id, get_subscription.id);
    }

    async fn test_record_usage(
        biller: &Biller,
        subscription_id: &SubscriptionId,
        usage_count: usize,
    ) {
        for _ in 0..usage_count {
            let quantity = rand::random::<u8>();
            biller
                .record_usage(subscription_id, u64::from(quantity))
                .await
                .unwrap();
        }
    }

    // Note: To run this test locally run:
    // `export TEST_BILLING_KEY=...`
    #[tokio::test]
    async fn test_biller() {
        let Some(billing_key) = test_billing_key() else {
            return;
        };
        let json_billing = JsonBilling {
            secret_key: billing_key.parse().unwrap(),
            products: test_products(),
        };
        let biller = Biller::new(json_billing).await.unwrap();

        // Customer
        let name = "Muriel Bagge".parse().unwrap();
        let email = format!("muriel.bagge.{}@nowhere.com", rand::random::<u64>())
            .parse()
            .unwrap();
        let user_uuid = Uuid::new_v4();
        assert!(biller.get_customer(&email).await.unwrap().is_none());
        let create_customer = biller
            .create_customer(&name, &email, user_uuid)
            .await
            .unwrap();
        let get_customer = biller.get_customer(&email).await.unwrap().unwrap();
        assert_eq!(create_customer.id, get_customer.id);
        let customer = create_customer;
        let get_or_create_customer = biller
            .get_or_create_customer(&name, &email, user_uuid)
            .await
            .unwrap();
        assert_eq!(customer.id, get_or_create_customer.id);

        // Payment Method
        let json_card = JsonCard {
            number: "3530111333300000".parse().unwrap(),
            exp_year: (Utc::now().year() + 1).try_into().unwrap(),
            exp_month: 1.try_into().unwrap(),
            cvc: "123".parse().unwrap(),
        };
        // assert!(biller
        //     .get_payment_method(&customer)
        //     .await
        //     .unwrap()
        //     .is_none());
        let create_payment_method = biller
            .create_payment_method(&customer, json_card.clone())
            .await
            .unwrap();
        // let get_payment_method = biller.get_payment_method(&customer).await.unwrap().unwrap();
        // assert_eq!(create_payment_method.id, get_payment_method.id);
        let payment_method = create_payment_method;
        // let get_or_create_payment_method = biller
        //     .get_or_create_payment_method(&customer, payment_card)
        //     .await
        //     .unwrap();
        // assert_eq!(payment_method.id, get_or_create_payment_method.id);

        // Team Metered Plan
        let organization = Uuid::new_v4();
        test_metered_subscription(
            &biller,
            organization,
            &customer,
            &payment_method,
            PlanLevel::Team,
            DEFAULT_PRICE_NAME.into(),
            10,
        )
        .await;

        // Team Licensed Plan
        let organization = Uuid::new_v4();
        test_licensed_subscription(
            &biller,
            organization,
            &customer,
            &payment_method,
            PlanLevel::Team,
            DEFAULT_PRICE_NAME.into(),
            10,
        )
        .await;

        // Enterprise Metered Plan
        let organization = Uuid::new_v4();
        test_metered_subscription(
            &biller,
            organization,
            &customer,
            &payment_method,
            PlanLevel::Enterprise,
            DEFAULT_PRICE_NAME.into(),
            25,
        )
        .await;

        // Enterprise Licensed Plan
        let organization = Uuid::new_v4();
        test_licensed_subscription(
            &biller,
            organization,
            &customer,
            &payment_method,
            PlanLevel::Team,
            DEFAULT_PRICE_NAME.into(),
            25,
        )
        .await;
    }
}
