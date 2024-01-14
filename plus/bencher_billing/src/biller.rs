use std::{fmt, str::FromStr};

use bencher_json::{
    organization::plan::{JsonCardDetails, JsonPlan},
    system::{
        config::JsonBilling,
        payment::{JsonCard, JsonCustomer},
    },
    Email, Entitlements, LicensedPlanId, MeteredPlanId, OrganizationUuid, PlanLevel, PlanStatus,
};
use stripe::{
    AttachPaymentMethod, CancelSubscription, CardDetailsParams as PaymentCard,
    Client as StripeClient, CreateCustomer, CreatePaymentMethod, CreatePaymentMethodCardUnion,
    CreateSubscription, CreateSubscriptionItems, CreateUsageRecord, Customer, CustomerId,
    Expandable, ListCustomers, PaymentMethod, PaymentMethodId, PaymentMethodTypeFilter,
    Subscription, SubscriptionId, SubscriptionItem, SubscriptionStatus, UsageRecord,
};

use crate::{products::Products, BillingError};

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
    Licensed(String, Entitlements),
}

impl ProductPlan {
    fn metered(plan_level: PlanLevel, price_name: String) -> Self {
        match plan_level {
            PlanLevel::Free => Self::Free,
            PlanLevel::Team => Self::Team(ProductUsage::Metered(price_name)),
            PlanLevel::Enterprise => Self::Enterprise(ProductUsage::Metered(price_name)),
        }
    }

    fn licensed(plan_level: PlanLevel, price_name: String, entitlements: Entitlements) -> Self {
        match plan_level {
            PlanLevel::Free => Self::Free,
            PlanLevel::Team => Self::Team(ProductUsage::Licensed(price_name, entitlements)),
            PlanLevel::Enterprise => {
                Self::Enterprise(ProductUsage::Licensed(price_name, entitlements))
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlanId {
    Metered(MeteredPlanId),
    Licensed(LicensedPlanId),
}

impl From<MeteredPlanId> for PlanId {
    fn from(metered_plan_id: MeteredPlanId) -> Self {
        Self::Metered(metered_plan_id)
    }
}

impl From<LicensedPlanId> for PlanId {
    fn from(licensed_plan_id: LicensedPlanId) -> Self {
        Self::Licensed(licensed_plan_id)
    }
}

impl TryFrom<PlanId> for SubscriptionId {
    type Error = BillingError;

    fn try_from(plan_id: PlanId) -> Result<Self, Self::Error> {
        match plan_id {
            PlanId::Metered(metered_plan_id) => metered_plan_id
                .as_ref()
                .parse()
                .map_err(BillingError::MeteredPlanId),
            PlanId::Licensed(licensed_plan_id) => licensed_plan_id
                .as_ref()
                .parse()
                .map_err(BillingError::MeteredPlanId),
        }
    }
}

impl fmt::Display for PlanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metered(metered_plan_id) => write!(f, "{metered_plan_id}"),
            Self::Licensed(licensed_plan_id) => write!(f, "{licensed_plan_id}"),
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
        customer: &JsonCustomer,
    ) -> Result<CustomerId, BillingError> {
        if let Some(customer) = self.get_customer(&customer.email).await? {
            Ok(customer)
        } else {
            self.create_customer(customer).await
        }
    }

    pub async fn get_customer(&self, email: &Email) -> Result<Option<CustomerId>, BillingError> {
        let list_customers = ListCustomers {
            email: Some(email.as_ref()),
            ..Default::default()
        };
        let mut customers = Customer::list(&self.client, &list_customers).await?;

        if let Some(customer) = customers.data.pop() {
            if customers.data.is_empty() {
                Ok(Some(customer.id))
            } else {
                Err(BillingError::EmailCollision(customer, customers.data))
            }
        } else {
            Ok(None)
        }
    }

    // WARNING: Use caution when calling this directly as multiple users with the same email can be created
    // Use `get_or_create_customer` instead!
    async fn create_customer(&self, customer: &JsonCustomer) -> Result<CustomerId, BillingError> {
        let create_customer = CreateCustomer {
            name: Some(customer.name.as_ref()),
            email: Some(customer.email.as_ref()),
            metadata: Some(
                [(METADATA_UUID.into(), customer.uuid.to_string())]
                    .into_iter()
                    .collect(),
            ),
            ..Default::default()
        };
        Customer::create(&self.client, create_customer)
            .await
            .map(|customer| customer.id)
            .map_err(Into::into)
    }

    // WARNING: Use caution when calling this directly as multiple payment methods can be created
    pub async fn create_payment_method(
        &self,
        customer_id: CustomerId,
        json_card: JsonCard,
    ) -> Result<PaymentMethodId, BillingError> {
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
                customer: customer_id,
            },
        )
        .await
        .map(|payment_method| payment_method.id)
        .map_err(Into::into)
    }

    pub async fn create_metered_subscription(
        &self,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        plan_level: PlanLevel,
        price_name: String,
    ) -> Result<Subscription, BillingError> {
        self.create_subscription(
            organization,
            customer_id,
            payment_method_id,
            ProductPlan::metered(plan_level, price_name),
        )
        .await
    }

    pub async fn create_licensed_subscription(
        &self,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        plan_level: PlanLevel,
        price_name: String,
        entitlements: Entitlements,
    ) -> Result<Subscription, BillingError> {
        self.create_subscription(
            organization,
            customer_id,
            payment_method_id,
            ProductPlan::licensed(plan_level, price_name, entitlements),
        )
        .await
    }

    // WARNING: Use caution when calling this directly as multiple subscriptions can be created
    async fn create_subscription(
        &self,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        product_plan: ProductPlan,
    ) -> Result<Subscription, BillingError> {
        let mut create_subscription = CreateSubscription::new(customer_id);
        let (price, entitlements) = match product_plan {
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
                ProductUsage::Licensed(price_name, entitlements) => (
                    self.products
                        .team
                        .licensed
                        .get(&price_name)
                        .ok_or(BillingError::PriceNotFound(price_name))?,
                    Some(entitlements),
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
                ProductUsage::Licensed(price_name, entitlements) => (
                    self.products
                        .enterprise
                        .licensed
                        .get(&price_name)
                        .ok_or(BillingError::PriceNotFound(price_name))?,
                    Some(entitlements),
                ),
            },
        };

        create_subscription.items = Some(vec![CreateSubscriptionItems {
            price: Some(price.id.to_string()),
            quantity: entitlements.map(Into::into),
            ..Default::default()
        }]);
        create_subscription.default_payment_method = Some(&payment_method_id);
        create_subscription.metadata = Some(
            [(METADATA_ORGANIZATION.to_owned(), organization.to_string())]
                .into_iter()
                .collect(),
        );

        Subscription::create(&self.client, create_subscription)
            .await
            .map_err(Into::into)
    }

    pub async fn get_subscription<P>(&self, plan_id: P) -> Result<Subscription, BillingError>
    where
        PlanId: From<P>,
    {
        self.get_subscription_expand(plan_id, &[]).await
    }

    pub async fn get_subscription_expand<P>(
        &self,
        plan_id: P,
        expand: &[&str],
    ) -> Result<Subscription, BillingError>
    where
        PlanId: From<P>,
    {
        let id = PlanId::from(plan_id).try_into()?;
        Subscription::retrieve(&self.client, &id, expand)
            .await
            .map_err(Into::into)
    }

    pub async fn get_plan<P>(&self, plan_id: P) -> Result<JsonPlan, BillingError>
    where
        PlanId: From<P>,
        P: Clone,
    {
        let subscription = self
            .get_subscription_expand(
                plan_id.clone(),
                &[
                    "customer",
                    "default_payment_method",
                    "items",
                    "items.data.price.product",
                ],
            )
            .await?;

        let Some(organization) = subscription.metadata.get(METADATA_ORGANIZATION) else {
            return Err(BillingError::NoOrganization(plan_id.into()));
        };
        let organization = organization
            .parse()
            .map_err(|e| BillingError::BadOrganizationUuid(organization.clone(), e))?;

        let current_period_start = subscription.current_period_start.try_into().map_err(|e| {
            BillingError::DateTime(plan_id.clone().into(), subscription.current_period_start, e)
        })?;
        let current_period_end = subscription.current_period_end.try_into().map_err(|e| {
            BillingError::DateTime(plan_id.clone().into(), subscription.current_period_end, e)
        })?;

        let customer = Self::get_plan_customer(&subscription.customer)?;
        let card = Self::get_plan_card(plan_id.clone(), &subscription.default_payment_method)?;
        let (level, unit_amount) = Self::get_plan_price(plan_id, subscription.items.data)?;

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
            license: None,
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
            uuid: uuid
                .parse()
                .map_err(|e| BillingError::BadUserUuid(uuid.clone(), e))?,
            name: name.parse()?,
            email: email.parse()?,
        })
    }

    fn get_plan_card<P>(
        plan_id: P,
        default_payment_method: &Option<Expandable<PaymentMethod>>,
    ) -> Result<JsonCardDetails, BillingError>
    where
        PlanId: From<P>,
    {
        let Some(default_payment_method) = default_payment_method else {
            return Err(BillingError::NoDefaultPaymentMethod(plan_id.into()));
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

    fn get_plan_price<P>(
        plan_id: P,
        subscription_items: Vec<SubscriptionItem>,
    ) -> Result<(PlanLevel, u64), BillingError>
    where
        PlanId: From<P>,
    {
        let subscription_item = Self::get_subscription_item(plan_id, subscription_items)?;
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

    fn get_subscription_item<P>(
        plan_id: P,
        mut subscription_items: Vec<SubscriptionItem>,
    ) -> Result<SubscriptionItem, BillingError>
    where
        PlanId: From<P>,
    {
        if let Some(subscription_item) = subscription_items.pop() {
            if subscription_items.is_empty() {
                Ok(subscription_item)
            } else {
                Err(BillingError::MultipleSubscriptionItems(
                    plan_id.into(),
                    subscription_item,
                    subscription_items,
                ))
            }
        } else {
            Err(BillingError::NoSubscriptionItem(plan_id.into()))
        }
    }

    pub async fn get_plan_status<P>(&self, plan_id: P) -> Result<PlanStatus, BillingError>
    where
        PlanId: From<P>,
    {
        let subscription = self.get_subscription(plan_id).await?;
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
        metered_plan_id: MeteredPlanId,
        quantity: u32,
    ) -> Result<UsageRecord, BillingError> {
        let subscription = self.get_subscription(metered_plan_id).await?;
        let metered_plan_id = MeteredPlanId::from_str(subscription.id.as_ref())?;
        let subscription_item =
            Self::get_subscription_item(metered_plan_id, subscription.items.data)?;

        let create_usage_record = CreateUsageRecord {
            quantity: quantity.into(),
            ..Default::default()
        };
        UsageRecord::create(&self.client, &subscription_item.id, create_usage_record)
            .await
            .map_err(Into::into)
    }

    pub async fn cancel_metered_subscription(
        &self,
        metered_plan_id: MeteredPlanId,
    ) -> Result<Subscription, BillingError> {
        self.cancel_subscription(metered_plan_id).await
    }

    pub async fn cancel_licensed_subscription(
        &self,
        licensed_plan_id: LicensedPlanId,
    ) -> Result<Subscription, BillingError> {
        self.cancel_subscription(licensed_plan_id).await
    }

    async fn cancel_subscription<P>(&self, plan_id: P) -> Result<Subscription, BillingError>
    where
        PlanId: From<P>,
    {
        let subscription = PlanId::from(plan_id).try_into()?;
        Subscription::cancel(&self.client, &subscription, CancelSubscription::default())
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
    use std::str::FromStr;

    use bencher_json::{
        organization::plan::DEFAULT_PRICE_NAME,
        system::{
            config::{JsonBilling, JsonProduct, JsonProducts},
            payment::{JsonCard, JsonCustomer},
        },
        Entitlements, LicensedPlanId, MeteredPlanId, OrganizationUuid, PlanLevel, PlanStatus,
        UserUuid,
    };
    use chrono::{Datelike, Utc};
    use literally::hmap;
    use pretty_assertions::assert_eq;
    use stripe::{CustomerId, PaymentMethodId};

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
                    "default".to_owned() => "price_1O4XlwKal5vzTlmh0n0wtplQ".to_owned(),
                },
            },
            enterprise: JsonProduct {
                id: "prod_NLC7fDet2C8Nmk".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1McW2eKal5vzTlmhECLIyVQz".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1O4Xo1Kal5vzTlmh1KrcEbq0".to_owned(),
                },
            },
        }
    }

    async fn test_metered_subscription(
        biller: &Biller,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        plan_level: PlanLevel,
        price_name: String,
        usage_count: usize,
    ) {
        let create_subscription = biller
            .create_metered_subscription(
                organization,
                customer_id,
                payment_method_id,
                plan_level,
                price_name,
            )
            .await
            .unwrap();

        let metered_plan_id = MeteredPlanId::from_str(create_subscription.id.as_ref()).unwrap();
        let get_subscription = biller
            .get_subscription(metered_plan_id.clone())
            .await
            .unwrap();
        assert_eq!(create_subscription.id, get_subscription.id);
        biller.get_plan(metered_plan_id.clone()).await.unwrap();

        let plan_status = biller
            .get_plan_status(metered_plan_id.clone())
            .await
            .unwrap();
        assert_eq!(plan_status, PlanStatus::Active);

        test_record_usage(biller, metered_plan_id.clone(), usage_count).await;

        biller
            .cancel_metered_subscription(metered_plan_id.clone())
            .await
            .unwrap();
        let plan_status = biller
            .get_plan_status(metered_plan_id.clone())
            .await
            .unwrap();
        assert_eq!(plan_status, PlanStatus::Canceled);
    }

    async fn test_licensed_subscription(
        biller: &Biller,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        plan_level: PlanLevel,
        price_name: String,
        entitlements: Entitlements,
    ) {
        let create_subscription = biller
            .create_licensed_subscription(
                organization,
                customer_id,
                payment_method_id,
                plan_level,
                price_name,
                entitlements,
            )
            .await
            .unwrap();

        let licensed_plan_id = LicensedPlanId::from_str(create_subscription.id.as_ref()).unwrap();
        let get_subscription = biller
            .get_subscription(licensed_plan_id.clone())
            .await
            .unwrap();
        assert_eq!(create_subscription.id, get_subscription.id);
        biller.get_plan(licensed_plan_id.clone()).await.unwrap();

        let plan_status = biller
            .get_plan_status(licensed_plan_id.clone())
            .await
            .unwrap();
        assert_eq!(plan_status, PlanStatus::Active);

        biller
            .cancel_licensed_subscription(licensed_plan_id.clone())
            .await
            .unwrap();

        let plan_status = biller
            .get_plan_status(licensed_plan_id.clone())
            .await
            .unwrap();
        assert_eq!(plan_status, PlanStatus::Canceled);
    }

    async fn test_record_usage(
        biller: &Biller,
        metered_plan_id: MeteredPlanId,
        usage_count: usize,
    ) {
        for _ in 0..usage_count {
            let quantity = u32::from(rand::random::<u8>());
            biller
                .record_usage(metered_plan_id.clone(), quantity)
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
        let json_customer = JsonCustomer {
            uuid: UserUuid::new(),
            name,
            email,
        };
        assert!(biller
            .get_customer(&json_customer.email)
            .await
            .unwrap()
            .is_none());
        let create_customer_id = biller.create_customer(&json_customer).await.unwrap();
        let get_customer_id = biller
            .get_customer(&json_customer.email)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(create_customer_id, get_customer_id);
        let customer_id = create_customer_id;
        let get_or_create_customer_id =
            biller.get_or_create_customer(&json_customer).await.unwrap();
        assert_eq!(customer_id, get_or_create_customer_id);

        // Payment Method
        let json_card = JsonCard {
            number: "3530111333300000".parse().unwrap(),
            exp_year: (Utc::now().year() + 1).try_into().unwrap(),
            exp_month: 1.try_into().unwrap(),
            cvc: "123".parse().unwrap(),
        };
        let payment_method_id = biller
            .create_payment_method(customer_id.clone(), json_card.clone())
            .await
            .unwrap();

        // Team Metered Plan
        let organization = OrganizationUuid::new();
        test_metered_subscription(
            &biller,
            organization,
            customer_id.clone(),
            payment_method_id.clone(),
            PlanLevel::Team,
            DEFAULT_PRICE_NAME.into(),
            10,
        )
        .await;

        // Team Licensed Plan
        let organization = OrganizationUuid::new();
        test_licensed_subscription(
            &biller,
            organization,
            customer_id.clone(),
            payment_method_id.clone(),
            PlanLevel::Team,
            DEFAULT_PRICE_NAME.into(),
            1_000.try_into().unwrap(),
        )
        .await;

        // Enterprise Metered Plan
        let organization = OrganizationUuid::new();
        test_metered_subscription(
            &biller,
            organization,
            customer_id.clone(),
            payment_method_id.clone(),
            PlanLevel::Enterprise,
            DEFAULT_PRICE_NAME.into(),
            25,
        )
        .await;

        // Enterprise Licensed Plan
        let organization = OrganizationUuid::new();
        test_licensed_subscription(
            &biller,
            organization,
            customer_id.clone(),
            payment_method_id.clone(),
            PlanLevel::Team,
            DEFAULT_PRICE_NAME.into(),
            1_000.try_into().unwrap(),
        )
        .await;
    }
}
