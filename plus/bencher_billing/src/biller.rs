use bencher_json::{
    organization::metered::{JsonCard, JsonCardDetails, JsonCustomer, JsonLevel, JsonPlan},
    system::config::JsonBilling,
    Email, UserName,
};
use chrono::{TimeZone, Utc};
use stripe::{
    AttachPaymentMethod, CardDetailsParams as PaymentCard, Client as StripeClient, CreateCustomer,
    CreatePaymentMethod, CreatePaymentMethodCardUnion, CreateSubscription, CreateSubscriptionItems,
    CreateUsageRecord, Customer, ListCustomers, PaymentMethod, PaymentMethodTypeFilter,
    Subscription, SubscriptionId, SubscriptionItem, UsageRecord,
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
}

#[derive(Debug, Clone)]
enum ProductPlan {
    Team(ProductUsage),
    Enterprise(ProductUsage),
}

#[derive(Debug, Clone)]
enum ProductUsage {
    Metered(String),
    Licensed(String, u64),
}

impl ProductPlan {
    fn metered(json_level: JsonLevel, price_name: String) -> Self {
        match json_level {
            JsonLevel::Team => Self::Team(ProductUsage::Metered(price_name)),
            JsonLevel::Enterprise => Self::Enterprise(ProductUsage::Metered(price_name)),
        }
    }

    fn licensed(json_level: JsonLevel, price_name: String, quantity: u64) -> Self {
        match json_level {
            JsonLevel::Team => Self::Team(ProductUsage::Licensed(price_name, quantity)),
            JsonLevel::Enterprise => Self::Enterprise(ProductUsage::Licensed(price_name, quantity)),
        }
    }
}

impl Biller {
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
        json_level: JsonLevel,
        price_name: String,
    ) -> Result<Subscription, BillingError> {
        self.create_subscription(
            organization,
            customer,
            payment_method,
            ProductPlan::metered(json_level, price_name),
        )
        .await
    }

    pub async fn create_licensed_subscription(
        &self,
        organization: Uuid,
        customer: &Customer,
        payment_method: &PaymentMethod,
        json_level: JsonLevel,
        price_name: String,
        quantity: u64,
    ) -> Result<Subscription, BillingError> {
        self.create_subscription(
            organization,
            customer,
            payment_method,
            ProductPlan::licensed(json_level, price_name, quantity),
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
            } else {
                Some(quantity * METRIC_QUANTITY)
            }
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
            [(METADATA_ORGANIZATION.to_string(), organization.to_string())]
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

    pub async fn get_subscription_item(
        subscription: Subscription,
    ) -> Result<SubscriptionItem, BillingError> {
        let mut subscription_items = subscription.items.data;

        if let Some(subscription_item) = subscription_items.pop() {
            if subscription_items.is_empty() {
                Ok(subscription_item)
            } else {
                Err(BillingError::MultipleSubscriptionItems(
                    subscription.id,
                    subscription_item,
                    subscription_items,
                ))
            }
        } else {
            Err(BillingError::NoSubscriptionItem(subscription.id))
        }
    }

    pub async fn get_plan(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<Option<Customer>, BillingError> {
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
        let current_period_start = subscription.current_period_start;
        let current_period_end = subscription.current_period_end;

        let Some(organization) = subscription.metadata.get(METADATA_ORGANIZATION) else {
            return Err(BillingError::NoOrganization(subscription_id.clone()));
        };

        let Some(customer) = subscription.customer.as_object() else {
            return Err(BillingError::NoCustomerInfo(subscription.customer.id()));
        };
        let Some(uuid) = customer.metadata.get(METADATA_UUID) else {
            return Err(BillingError::NoUuid(customer.id.clone()));
        };
        let Some(name) = &customer.name else {
            return Err(BillingError::NoName(customer.id.clone()));
        };
        let Some(email) = &customer.email else {
            return Err(BillingError::NoEmail(customer.id.clone()));
        };

        let json_customer = JsonCustomer {
            uuid: uuid.parse()?,
            name: name.parse()?,
            email: email.parse()?,
        };

        let Some(default_payment_method) = &subscription.default_payment_method else {
            return Err(BillingError::NoDefaultPaymentMethod(subscription_id.clone()));
        };

        let Some(default_payment_method_info) = default_payment_method.as_object() else {
            return Err(BillingError::NoDefaultPaymentMethodInfo(default_payment_method.id()));
        };

        let Some(card) = &default_payment_method_info.card else {
            return Err(BillingError::NoCardDetails(default_payment_method.id()));
        };

        let json_card_details = JsonCardDetails {
            brand: card.brand.parse()?,
            last_four: card.last4.parse()?,
            exp_month: card.exp_month.try_into()?,
            exp_year: card.exp_year.try_into()?,
        };

        // panic!("{subscription:#?}");

        let subscription_item = Self::get_subscription_item(subscription).await?;

        let Some(price) = subscription_item.price else {
            return Err(BillingError::NoPrice(subscription_item.id))
        };

        let Some(product) = price.product else {
            return Err(BillingError::NoProduct(price.id))
        };

        let Some(product_info) = product.as_object() else {
            return Err(BillingError::NoProductInfo(product.id()))
        };

        // Bencher Team
        // Bencher Enterprise
        let Some(product_name) = &product_info.name else {
            return Err(BillingError::NoProductName(product.id()));
        };

        let json_plan = JsonPlan {
            organization: organization.parse()?,
            customer: json_customer,
            card: json_card_details,
            level: product_name.parse()?,
            current_period_start: Utc
                .timestamp_opt(current_period_start, 0)
                .single()
                .ok_or_else(|| {
                    BillingError::DateTime(subscription_id.clone(), current_period_start)
                })?,
            current_period_end: Utc
                .timestamp_opt(current_period_end, 0)
                .single()
                .ok_or_else(|| {
                    BillingError::DateTime(subscription_id.clone(), current_period_end)
                })?,
        };

        todo!()

        /*
            price: Some(
                        Price {
                            id: PriceId(
                                "price_1McW12Kal5vzTlmhoPltpBAW",
                            ),
                            active: Some(
                                true,
                            ),
                            billing_scheme: Some(
                                PerUnit,
                            ),
                            created: Some(
                                1676648308,
                            ),
                            currency: Some(
                                USD,
                            ),
                            currency_options: None,
                            custom_unit_amount: None,
                            deleted: false,
                            livemode: Some(
                                false,
                            ),
                            lookup_key: None,
                            metadata: {},
                            nickname: None,
                            product: Some(
                                Object(
                                    Product {
                                        id: ProductId(
                                            "prod_NKz5B9dGhDiSY1",
                                        ),
                                        active: Some(
                                            true,
                                        ),
                                        attributes: Some(
                                            [],
                                        ),
                                        caption: None,
                                        created: Some(
                                            1676122095,
                                        ),
                                        deactivate_on: None,
                                        default_price: Some(
                                            Id(
                                                PriceId(
                                                    "price_1McW12Kal5vzTlmhoPltpBAW",
                                                ),
                                            ),
                                        ),
                                        deleted: false,
                                        description: None,
                                        images: Some(
                                            [
                                                "https://files.stripe.com/links/MDB8YWNjdF8xSFZqd3ZLYWw1dnpUbG1ofGZsX3Rlc3RfOXZsWGhJcE85aG5yeXIxTFRoYkxQTEdr00g5SW86QL",
                                            ],
                                        ),
                                        livemode: Some(
                                            false,
                                        ),
                                        metadata: {},
                                        name: Some(
                                            "Bencher Team",
                                        ),
                                        package_dimensions: None,
                                        shippable: None,
                                        statement_descriptor: Some(
                                            "Bencher - POMPEII LLC",
                                        ),
                                        tax_code: None,
                                        type_: Some(
                                            Service,
                                        ),
                                        unit_label: Some(
                                            "metric",
                                        ),
                                        updated: Some(
                                            1676648334,
                                        ),
                                        url: None,
                                    },
                                ),
                            ),
                            recurring: Some(
                                Recurring {
                                    aggregate_usage: Some(
                                        Sum,
                                    ),
                                    interval: Month,
                                    interval_count: 1,
                                    trial_period_days: None,
                                    usage_type: Metered,
                                },
                            ),
                            tax_behavior: Some(
                                Unspecified,
                            ),
                            tiers: None,
                            tiers_mode: None,
                            transform_quantity: None,
                            type_: Some(
                                Recurring,
                            ),
                            unit_amount: Some(
                                1,
                            ),
                            unit_amount_decimal: Some(
                                "1",
                            ),
                        },
                    ),
                    quantity: None,
                    subscription: Some(
                        "sub_1Mdk0tKal5vzTlmhf3O8RftA",
                    ),
                    tax_rates: Some(
                        [],
                    ),
                },
            ],
            has_more: false,
            total_count: Some(
                1,
            ),
            url: "/v1/subscription_items?subscription=sub_1Mdk0tKal5vzTlmhf3O8RftA",
        },
                        customer: Id(
                        CustomerId(
                            "cus_NOMrHVtLltfnqL",
                        ),
                    ),


                     default_payment_method: Some(
                    Id(
                        PaymentMethodId(
                            "pm_1Mda7wKal5vzTlmhjq71XB9w",
                        ),
                    ),


                    items: List {
                data: [
                    SubscriptionItem {
                        id: SubscriptionItemId(
                            "si_NOMrYIRvGxKC6g",
                        ),
                ),


                 plan: Some(
                            Plan {
                                id: PlanId(
                                    "price_1McW12Kal5vzTlmhoPltpBAW",
                                ),

                                 product: Some(
                                    Id(
                                        ProductId(
                                            "prod_NKz5B9dGhDiSY1",
                                        ),
                                    ),
                                ),

                                 price: Some(
                            Price {
                                id: PriceId(
                                    "price_1McW12Kal5vzTlmhoPltpBAW",
                                ),

                                metadata: {
                "organization": "2caa5bab-a42b-4ef8-8b59-e97822ef248d",
            },
                             */
    }

    pub async fn record_usage(
        &self,
        subscription_id: &SubscriptionId,
        quantity: u64,
    ) -> Result<UsageRecord, BillingError> {
        let subscription = self.get_subscription(subscription_id).await?;
        let subscription_item = Self::get_subscription_item(subscription).await?;

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
mod test {
    use bencher_json::{
        organization::metered::{JsonCard, JsonLevel, DEFAULT_PRICE_NAME},
        system::config::{JsonBilling, JsonProduct, JsonProducts},
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
                    "default".to_string() => "price_1McW12Kal5vzTlmhoPltpBAW".to_string(),
                },
                licensed: hmap! {
                    "default".to_string() => "price_1MaJ7kKal5vzTlmh1pbQ5JYR".to_string(),
                },
            },
            enterprise: JsonProduct {
                id: "prod_NLC7fDet2C8Nmk".into(),
                metered: hmap! {
                    "default".to_string() => "price_1McW2eKal5vzTlmhECLIyVQz".to_string(),
                },
                licensed: hmap! {
                    "default".to_string() => "price_1MaViyKal5vzTlmho1MdXIpe".to_string(),
                },
            },
        }
    }

    async fn test_metered_subscription(
        biller: &Biller,
        organization: Uuid,
        customer: &Customer,
        payment_method: &PaymentMethod,
        json_level: JsonLevel,
        price_name: String,
        usage_count: usize,
    ) {
        let create_subscription = biller
            .create_metered_subscription(
                organization,
                customer,
                payment_method,
                json_level,
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

        biller.get_plan(&create_subscription.id).await;
    }

    async fn test_licensed_subscription(
        biller: &Biller,
        organization: Uuid,
        customer: &Customer,
        payment_method: &PaymentMethod,
        json_level: JsonLevel,
        price_name: String,
        quantity: u64,
    ) {
        let create_subscription = biller
            .create_licensed_subscription(
                organization,
                customer,
                payment_method,
                json_level,
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
                .record_usage(subscription_id, quantity as u64)
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
            JsonLevel::Team,
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
            JsonLevel::Team,
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
            JsonLevel::Enterprise,
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
            JsonLevel::Team,
            DEFAULT_PRICE_NAME.into(),
            25,
        )
        .await;
    }
}
