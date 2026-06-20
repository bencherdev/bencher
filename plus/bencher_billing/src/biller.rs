use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use bencher_json::{
    Email, Entitlements, LicensedPlanId, MeteredPlanId, OrganizationUuid, PlanLevel, PlanStatus,
    organization::plan::{
        JsonCardDetails, JsonPlan, METRICS_METER_NAME, RUNNER_MINUTES_METER_NAME,
    },
    system::{
        config::JsonBilling,
        payment::{JsonCard, JsonCheckout, JsonCustomer},
    },
};
use stripe::{Client as StripeClient, IdempotencyKey, RequestStrategy, StripeRequest as _};
use stripe_billing::{
    BillingMeterEvent, Subscription, SubscriptionId, SubscriptionItem, SubscriptionStatus,
    billing_credit_grant::{
        CreateBillingCreditGrant, CreateBillingCreditGrantAmount,
        CreateBillingCreditGrantAmountMonetary, CreateBillingCreditGrantAmountType,
        CreateBillingCreditGrantApplicabilityConfig,
        CreateBillingCreditGrantApplicabilityConfigScope,
        CreateBillingCreditGrantApplicabilityConfigScopePrices, ListBillingCreditGrant,
    },
    billing_meter_event::CreateBillingMeterEvent,
    subscription::{
        CancelSubscription, CreateSubscription, CreateSubscriptionItems, DiscountsDataParam,
        RetrieveSubscription, UpdateSubscription,
    },
};
use stripe_checkout::{
    CheckoutSessionId, CheckoutSessionMode, CheckoutSessionUiMode,
    checkout_session::{
        CreateCheckoutSession, CreateCheckoutSessionConsentCollection,
        CreateCheckoutSessionConsentCollectionTermsOfService, CreateCheckoutSessionDiscounts,
        CreateCheckoutSessionLineItems, CreateCheckoutSessionLineItemsAdjustableQuantity,
        CreateCheckoutSessionPaymentMethodTypes, CreateCheckoutSessionSubscriptionData,
        RetrieveCheckoutSession,
    },
};
use stripe_core::customer::{CreateCustomer, ListCustomer};
use stripe_payment::{
    PaymentMethod, PaymentMethodId,
    payment_method::{
        AttachPaymentMethod, CreatePaymentMethod, CreatePaymentMethodCard,
        CreatePaymentMethodCardDetailsParams, CreatePaymentMethodType,
    },
};
use stripe_product::{Price, PriceId};
use stripe_shared::{Customer, CustomerId};
use stripe_types::{Currency, Expandable};

use crate::{BillingError, products::Products};

const METADATA_UUID: &str = "uuid";
const METADATA_ORGANIZATION: &str = "organization";
const STRIPE_MAX_QUANTITY: u32 = 999_999;

/// Stripe meter event payload key for the customer identifier.
const METER_CUSTOMER_KEY: &str = "stripe_customer_id";
/// Stripe meter event payload key for the usage value.
const METER_VALUE_KEY: &str = "value";

/// Credit grant metadata key marking a grant as set by Bencher Cloud.
const CREDIT_BENCHER_CLOUD_KEY: &str = "bencher_cloud";
/// Credit grant metadata key holding the billing-period start, used to
/// idempotently grant exactly one included-usage credit per period.
const CREDIT_PERIOD_KEY: &str = "period_start";
/// Descriptive name shown in the Stripe Dashboard for the included usage credit.
const CREDIT_NAME: &str = "Bencher Cloud included usage credit";

#[derive(Clone)]
pub struct Biller {
    client: StripeClient,
    products: Products,
}

/// Outcome of [`Biller::ensure_period_credit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodCredit {
    /// A new included-usage credit was granted for the current period.
    Granted,
    /// This period's credit already existed; no new grant was created.
    AlreadyGranted,
    /// The subscription has no flat base fee, so no credit applies.
    NotApplicable,
}

/// The configuration price-name key (e.g. `default`) selecting which configured
/// price to use. A newtype so a plan carries a typed selector, not a bare `String`.
#[derive(Debug, Clone)]
struct PriceName(String);

impl PriceName {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PriceName {
    fn from(price_name: String) -> Self {
        Self(price_name)
    }
}

impl fmt::Display for PriceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
enum PlusPlan {
    Free,
    // Bencher Cloud metered tier with a flat monthly base fee and an included
    // usage credit. Carries the metered metrics price name, which resolves
    // against the `metrics` product; the base price and trial coupon come from
    // the `pro` product and are looked up separately. This tier has no licensed
    // (Self-Hosted) form.
    Pro(PriceName),
    Team(PlusUsage),
    Enterprise(PlusUsage),
}

#[derive(Debug, Clone)]
enum PlusUsage {
    Metered(PriceName),
    Licensed(PriceName, Entitlements),
}

impl PlusPlan {
    fn metered(plan_level: PlanLevel, price_name: String) -> Self {
        match plan_level {
            PlanLevel::Free => Self::Free,
            PlanLevel::Pro => Self::Pro(price_name.into()),
            PlanLevel::Team => Self::Team(PlusUsage::Metered(price_name.into())),
            PlanLevel::Enterprise => Self::Enterprise(PlusUsage::Metered(price_name.into())),
        }
    }

    fn licensed(plan_level: PlanLevel, price_name: String, entitlements: Entitlements) -> Self {
        match plan_level {
            PlanLevel::Free => Self::Free,
            // This tier is Bencher Cloud metered only; it has no licensed form.
            // The API layer rejects a licensed request for it before reaching here.
            PlanLevel::Pro => Self::Pro(price_name.into()),
            PlanLevel::Team => Self::Team(PlusUsage::Licensed(price_name.into(), entitlements)),
            PlanLevel::Enterprise => {
                Self::Enterprise(PlusUsage::Licensed(price_name.into(), entitlements))
            },
        }
    }

    fn bare_metal_price<'a>(&self, products: &'a Products) -> Result<&'a Price, BillingError> {
        let (pricing, price_name) = match self {
            Self::Free => return Err(BillingError::ProductLevelFree),
            Self::Pro(price_name) => (&products.bare_metal.metered, price_name),
            Self::Team(usage) | Self::Enterprise(usage) => match usage {
                PlusUsage::Metered(price_name) => (&products.bare_metal.metered, price_name),
                PlusUsage::Licensed(price_name, _) => (&products.bare_metal.licensed, price_name),
            },
        };
        pricing
            .get(price_name.as_str())
            .ok_or_else(|| BillingError::PriceNotFound(price_name.to_string()))
    }

    // The flat recurring base fee for this plan, if any.
    fn base_price<'a>(&self, products: &'a Products) -> Result<Option<&'a Price>, BillingError> {
        match self {
            Self::Pro(price_name) => products
                .pro
                .base
                .get(price_name.as_str())
                .map(Some)
                .ok_or_else(|| BillingError::PriceNotFound(price_name.to_string())),
            Self::Free | Self::Team(_) | Self::Enterprise(_) => Ok(None),
        }
    }

    // Whether this plan carries a flat recurring base fee (and so receives the
    // free-trial coupon and the monthly included-usage credit).
    fn has_base_fee(&self) -> bool {
        matches!(self, Self::Pro(_))
    }

    // The configured free-trial coupon for this plan's product, if any.
    fn trial_coupon<'a>(&self, products: &'a Products) -> Option<&'a str> {
        match self {
            Self::Pro(_) => products.pro.trial_coupon.as_deref(),
            Self::Free | Self::Team(_) | Self::Enterprise(_) => None,
        }
    }

    fn into_plus_price(
        self,
        products: &Products,
    ) -> Result<(&Price, Option<Entitlements>), BillingError> {
        Ok(match self {
            PlusPlan::Free => return Err(BillingError::ProductLevelFree),
            PlusPlan::Pro(price_name) => (
                products
                    .metrics
                    .metered
                    .get(price_name.as_str())
                    .ok_or_else(|| BillingError::PriceNotFound(price_name.to_string()))?,
                None,
            ),
            PlusPlan::Team(plus_usage) => match plus_usage {
                PlusUsage::Metered(price_name) => (
                    products
                        .team
                        .metered
                        .get(price_name.as_str())
                        .ok_or_else(|| BillingError::PriceNotFound(price_name.to_string()))?,
                    None,
                ),
                PlusUsage::Licensed(price_name, entitlements) => (
                    products
                        .team
                        .licensed
                        .get(price_name.as_str())
                        .ok_or_else(|| BillingError::PriceNotFound(price_name.to_string()))?,
                    Some(entitlements),
                ),
            },
            PlusPlan::Enterprise(plus_usage) => match plus_usage {
                PlusUsage::Metered(price_name) => (
                    products
                        .enterprise
                        .metered
                        .get(price_name.as_str())
                        .ok_or_else(|| BillingError::PriceNotFound(price_name.to_string()))?,
                    None,
                ),
                PlusUsage::Licensed(price_name, entitlements) => (
                    products
                        .enterprise
                        .licensed
                        .get(price_name.as_str())
                        .ok_or_else(|| BillingError::PriceNotFound(price_name.to_string()))?,
                    Some(entitlements),
                ),
            },
        })
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

impl From<PlanId> for SubscriptionId {
    fn from(plan_id: PlanId) -> Self {
        match plan_id {
            PlanId::Metered(metered_plan_id) => metered_plan_id.as_ref().into(),
            PlanId::Licensed(licensed_plan_id) => licensed_plan_id.as_ref().into(),
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

    pub async fn new_checkout_session(
        &self,
        organization: OrganizationUuid,
        customer: &JsonCustomer,
        plan_level: PlanLevel,
        price_name: String,
        entitlements: Option<Entitlements>,
        return_url: &str,
    ) -> Result<JsonCheckout, BillingError> {
        let customer = self.get_or_create_customer(customer).await?;

        let plus_plan = if let Some(entitlements) = entitlements {
            PlusPlan::licensed(plan_level, price_name, entitlements)
        } else {
            PlusPlan::metered(plan_level, price_name)
        };
        let bare_metal_price = plus_plan.bare_metal_price(&self.products)?;
        let base_price = plus_plan.base_price(&self.products)?;
        let trial_coupon = self.trial_coupon(&plus_plan);
        let (plus_price, entitlements) = plus_plan.into_plus_price(&self.products)?;

        let mut plus_line_item = CreateCheckoutSessionLineItems {
            price: Some(plus_price.id.to_string()),
            quantity: entitlements.map(|e| std::cmp::min(e.into(), STRIPE_MAX_QUANTITY.into())),
            ..Default::default()
        };
        plus_line_item.adjustable_quantity = entitlements.map(|_| {
            let mut aq = CreateCheckoutSessionLineItemsAdjustableQuantity::new(true);
            aq.minimum = Some(10_000);
            aq.maximum = Some(STRIPE_MAX_QUANTITY.into());
            aq
        });

        let bare_metal_line_item = CreateCheckoutSessionLineItems {
            price: Some(bare_metal_price.id.to_string()),
            ..Default::default()
        };

        // Flat monthly base fee billed alongside the metered usage.
        let mut line_items = Vec::new();
        if let Some(base_price) = base_price {
            line_items.push(CreateCheckoutSessionLineItems {
                price: Some(base_price.id.to_string()),
                quantity: Some(1),
                ..Default::default()
            });
        }
        line_items.push(plus_line_item);
        line_items.push(bare_metal_line_item);

        let mut consent = CreateCheckoutSessionConsentCollection::new();
        // https://bencher.dev/legal/subscription/
        consent.terms_of_service =
            Some(CreateCheckoutSessionConsentCollectionTermsOfService::Required);

        let mut sub_data = CreateCheckoutSessionSubscriptionData::new();
        sub_data.metadata = Some(
            [(METADATA_ORGANIZATION.into(), organization.to_string())]
                .into_iter()
                .collect(),
        );

        let mut create_checkout_session = CreateCheckoutSession::new()
            .ui_mode(CheckoutSessionUiMode::HostedPage)
            .customer(customer.to_string())
            .payment_method_types(vec![CreateCheckoutSessionPaymentMethodTypes::Card])
            .currency(Currency::USD)
            .mode(CheckoutSessionMode::Subscription)
            .line_items(line_items)
            .consent_collection(consent)
            .subscription_data(sub_data)
            .success_url(return_url);
        // Free trial: waive the first month's base fee via a one-time coupon.
        // Metered usage still bills, offset by the monthly included-usage credit.
        if let Some(coupon) = trial_coupon {
            create_checkout_session =
                create_checkout_session.discounts(vec![CreateCheckoutSessionDiscounts {
                    coupon: Some(coupon),
                    promotion_code: None,
                }]);
        }
        let mut checkout_session = create_checkout_session.send(&self.client).await?;

        Ok(JsonCheckout {
            session: checkout_session.id.to_string(),
            url: checkout_session
                .url
                .take()
                .ok_or(BillingError::NoCheckoutUrl(Box::new(checkout_session)))?,
        })
    }

    pub async fn get_checkout_session(
        &self,
        session_id: &str,
    ) -> Result<SubscriptionId, BillingError> {
        let session_id: CheckoutSessionId = session_id.into();
        let mut checkout_session = RetrieveCheckoutSession::new(session_id)
            .expand(vec!["subscription".into()])
            .send(&self.client)
            .await?;
        let subscription = checkout_session
            .subscription
            .take()
            .ok_or(BillingError::NoSubscription(Box::new(checkout_session)))?;
        Ok(subscription.id().clone())
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
        let mut customers = ListCustomer::new()
            .email(email.as_ref())
            .send(&self.client)
            .await?;

        if let Some(customer) = customers.data.pop() {
            if customers.data.is_empty() {
                Ok(Some(customer.id))
            } else {
                Err(BillingError::EmailCollision(
                    Box::new(customer),
                    customers.data,
                ))
            }
        } else {
            Ok(None)
        }
    }

    // WARNING: Use caution when calling this directly as multiple users with the same email can be created
    // Use `get_or_create_customer` instead!
    async fn create_customer(&self, customer: &JsonCustomer) -> Result<CustomerId, BillingError> {
        CreateCustomer::new()
            .name(customer.name.as_ref())
            .email(customer.email.as_ref())
            .metadata(
                [(METADATA_UUID.into(), customer.uuid.to_string())]
                    .into_iter()
                    .collect::<HashMap<String, String>>(),
            )
            .send(&self.client)
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
        let card_params = into_payment_card(json_card);
        let payment_method = CreatePaymentMethod::new()
            .type_(CreatePaymentMethodType::Card)
            .card(CreatePaymentMethodCard::CardDetailsParams(card_params))
            .send(&self.client)
            .await?;

        AttachPaymentMethod::new(payment_method.id.clone())
            .customer(customer_id.to_string())
            .send(&self.client)
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
            PlusPlan::metered(plan_level, price_name),
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
            PlusPlan::licensed(plan_level, price_name, entitlements),
        )
        .await
    }

    // WARNING: Use caution when calling this directly as multiple subscriptions can be created
    async fn create_subscription(
        &self,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        plus_plan: PlusPlan,
    ) -> Result<Subscription, BillingError> {
        let bare_metal_price = plus_plan.bare_metal_price(&self.products)?;
        let base_price = plus_plan.base_price(&self.products)?;
        let trial_coupon = self.trial_coupon(&plus_plan);
        let (plus_price, entitlements) = plus_plan.into_plus_price(&self.products)?;

        // Flat monthly base fee billed alongside the metered usage.
        let mut items = Vec::new();
        if let Some(base_price) = base_price {
            let mut base_item = CreateSubscriptionItems::new();
            base_item.price = Some(base_price.id.to_string());
            base_item.quantity = Some(1);
            items.push(base_item);
        }
        let mut plus_item = CreateSubscriptionItems::new();
        plus_item.price = Some(plus_price.id.to_string());
        plus_item.quantity = entitlements.map(Into::into);
        items.push(plus_item);
        let mut bare_metal_item = CreateSubscriptionItems::new();
        bare_metal_item.price = Some(bare_metal_price.id.to_string());
        items.push(bare_metal_item);

        let mut create_subscription = CreateSubscription::new()
            .customer(customer_id.to_string())
            .items(items)
            .default_payment_method(payment_method_id.to_string())
            .metadata(
                [(METADATA_ORGANIZATION.to_owned(), organization.to_string())]
                    .into_iter()
                    .collect::<HashMap<String, String>>(),
            );
        // Free trial: waive the first month's base fee via a one-time coupon.
        if let Some(coupon) = trial_coupon {
            create_subscription = create_subscription.discounts(vec![DiscountsDataParam {
                coupon: Some(coupon),
                discount: None,
                promotion_code: None,
            }]);
        }
        create_subscription
            .send(&self.client)
            .await
            .map_err(Into::into)
    }

    pub async fn get_metered_plan(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<JsonPlan, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        self.get_plan(&subscription_id).await
    }

    pub async fn get_licensed_plan(
        &self,
        licensed_plan_id: &LicensedPlanId,
    ) -> Result<JsonPlan, BillingError> {
        let subscription_id: SubscriptionId = licensed_plan_id.as_ref().into();
        self.get_plan(&subscription_id).await
    }

    async fn get_plan(&self, subscription_id: &SubscriptionId) -> Result<JsonPlan, BillingError> {
        let subscription = self
            .get_subscription_expand(
                subscription_id,
                vec![
                    "customer".into(),
                    "default_payment_method".into(),
                    "items".into(),
                    "items.data.price.product".into(),
                ],
            )
            .await?;

        let Some(organization) = subscription.metadata.get(METADATA_ORGANIZATION) else {
            return Err(BillingError::NoOrganization(subscription_id.clone()));
        };
        let organization = organization
            .parse()
            .map_err(|e| BillingError::BadOrganizationUuid(organization.clone(), e))?;

        let plan_price_ids = self.products.plan_price_ids(METRICS_METER_NAME);
        let subscription_items = Self::filter_subscription_items(
            subscription_id,
            subscription.items.data,
            &plan_price_ids,
        )?;
        let subscription_item = Self::get_subscription_item(subscription_id, subscription_items)?;

        let current_period_start =
            subscription_item
                .current_period_start
                .try_into()
                .map_err(|e| {
                    BillingError::DateTime(
                        subscription_id.clone(),
                        subscription_item.current_period_start,
                        e,
                    )
                })?;
        let current_period_end = subscription_item
            .current_period_end
            .try_into()
            .map_err(|e| {
                BillingError::DateTime(
                    subscription_id.clone(),
                    subscription_item.current_period_end,
                    e,
                )
            })?;

        let created = subscription.created.try_into().map_err(|e| {
            BillingError::DateTime(subscription_id.clone(), subscription.created, e)
        })?;

        let customer = Self::get_plan_customer(&subscription.customer)?;
        let card = Self::get_plan_card(
            subscription_id,
            subscription.default_payment_method.as_ref(),
        )?;
        let (level, unit_amount) = Self::get_plan_price(&subscription_item)?;

        let status = Self::map_status(&subscription.status);

        Ok(JsonPlan {
            organization,
            customer,
            card,
            level,
            unit_amount: unit_amount.into(),
            created,
            current_period_start,
            current_period_end,
            status,
            cancel_at_period_end: subscription.cancel_at_period_end,
            license: None,
        })
    }

    pub async fn get_subscription(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<Subscription, BillingError> {
        self.get_subscription_expand(subscription_id, vec![]).await
    }

    pub async fn get_subscription_expand(
        &self,
        subscription_id: &SubscriptionId,
        expand: Vec<String>,
    ) -> Result<Subscription, BillingError> {
        let mut req = RetrieveSubscription::new(subscription_id.clone());
        if !expand.is_empty() {
            req = req.expand(expand);
        }
        req.send(&self.client).await.map_err(Into::into)
    }

    fn get_plan_customer(customer: &Expandable<Customer>) -> Result<JsonCustomer, BillingError> {
        let Some(customer) = customer.as_object() else {
            return Err(BillingError::NoCustomerInfo(customer.id().clone()));
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

    fn get_plan_card(
        subscription_id: &SubscriptionId,
        default_payment_method: Option<&Expandable<PaymentMethod>>,
    ) -> Result<JsonCardDetails, BillingError> {
        let Some(default_payment_method) = default_payment_method else {
            return Err(BillingError::NoDefaultPaymentMethod(
                subscription_id.clone(),
            ));
        };
        let Some(default_payment_method_info) = default_payment_method.as_object() else {
            return Err(BillingError::NoDefaultPaymentMethodInfo(
                default_payment_method.id().clone(),
            ));
        };
        let Some(card_details) = &default_payment_method_info.card else {
            return Err(BillingError::NoCardDetails(
                default_payment_method.id().clone(),
            ));
        };
        Ok(JsonCardDetails {
            brand: card_details.brand.parse()?,
            last_four: card_details.last4.parse()?,
            exp_month: card_details.exp_month.try_into()?,
            exp_year: card_details.exp_year.try_into()?,
        })
    }

    fn get_plan_price(
        subscription_item: &SubscriptionItem,
    ) -> Result<(PlanLevel, u64), BillingError> {
        let price = &subscription_item.price;

        let Some(unit_amount) = price.unit_amount else {
            return Err(BillingError::NoUnitAmount(price.id.clone()));
        };
        let unit_amount = u64::try_from(unit_amount)?;

        let Some(product_info) = price.product.as_object() else {
            return Err(BillingError::NoProductInfo(price.product.id().clone()));
        };
        // The metered plan item's product name maps to a `PlanLevel`:
        // `Bencher Team`/`Bencher Enterprise`, or `Bencher Metrics` for the Pro
        // plan (whose metered metrics item lives on the `metrics` product).
        let plan_level = product_info.name.parse()?;

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
                    Box::new(subscription_item),
                    subscription_items,
                ))
            }
        } else {
            Err(BillingError::NoSubscriptionItem(subscription_id.clone()))
        }
    }

    // During the metered billing migration, a Stripe subscription may have
    // multiple subscription items (old + new metered prices). This function
    // filters subscription items to only those whose price ID matches one of
    // the provided known price IDs, so that `get_subscription_item()` can still
    // enforce its exactly-one invariant against the filtered set.
    //
    // Outside of migration, this is a no-op: subscriptions have one item whose
    // price matches a known ID, so the filtered list is identical to the input.
    fn filter_subscription_items(
        subscription_id: &SubscriptionId,
        subscription_items: Vec<SubscriptionItem>,
        price_ids: &HashSet<&PriceId>,
    ) -> Result<Vec<SubscriptionItem>, BillingError> {
        let total = subscription_items.len();
        let filtered: Vec<_> = subscription_items
            .into_iter()
            .filter(|item| price_ids.contains(&item.price.id))
            .collect();
        if filtered.is_empty() {
            Err(BillingError::NoMatchingSubscriptionItem(
                subscription_id.clone(),
                total,
            ))
        } else {
            Ok(filtered)
        }
    }

    pub async fn get_metered_plan_status(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<(PlanStatus, CustomerId), BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        let subscription = self.get_subscription(&subscription_id).await?;
        Ok((
            Self::map_status(&subscription.status),
            subscription.customer.id().clone(),
        ))
    }

    pub async fn get_licensed_plan_status(
        &self,
        licensed_plan_id: &LicensedPlanId,
    ) -> Result<PlanStatus, BillingError> {
        let subscription_id: SubscriptionId = licensed_plan_id.as_ref().into();
        let subscription = self.get_subscription(&subscription_id).await?;
        Ok(Self::map_status(&subscription.status))
    }

    fn map_status(status: &SubscriptionStatus) -> PlanStatus {
        match status {
            SubscriptionStatus::Active => PlanStatus::Active,
            SubscriptionStatus::Canceled => PlanStatus::Canceled,
            SubscriptionStatus::Incomplete => PlanStatus::Incomplete,
            SubscriptionStatus::IncompleteExpired => PlanStatus::IncompleteExpired,
            SubscriptionStatus::PastDue => PlanStatus::PastDue,
            SubscriptionStatus::Paused => PlanStatus::Paused,
            SubscriptionStatus::Trialing => PlanStatus::Trialing,
            SubscriptionStatus::Unpaid | SubscriptionStatus::Unknown(_) | _ => PlanStatus::Unpaid,
        }
    }

    pub async fn record_metrics_usage(
        &self,
        customer_id: &CustomerId,
        quantity: u32,
    ) -> Result<BillingMeterEvent, BillingError> {
        self.record_metered_usage(METRICS_METER_NAME, customer_id, quantity)
            .await
    }

    pub async fn record_runner_usage(
        &self,
        customer_id: &CustomerId,
        minutes: u32,
    ) -> Result<BillingMeterEvent, BillingError> {
        self.record_metered_usage(RUNNER_MINUTES_METER_NAME, customer_id, minutes)
            .await
    }

    async fn record_metered_usage(
        &self,
        meter_name: &str,
        customer_id: &CustomerId,
        quantity: u32,
    ) -> Result<BillingMeterEvent, BillingError> {
        CreateBillingMeterEvent::new(
            meter_name,
            HashMap::from([
                (METER_CUSTOMER_KEY.to_owned(), customer_id.to_string()),
                (METER_VALUE_KEY.to_owned(), quantity.to_string()),
            ]),
        )
        .send(&self.client)
        .await
        .map_err(Into::into)
    }

    // The first-period base-fee waiver coupon for this plan: returned only when
    // the plan has a base fee and its product configures a trial coupon.
    fn trial_coupon(&self, plus_plan: &PlusPlan) -> Option<String> {
        if !plus_plan.has_base_fee() {
            return None;
        }
        plus_plan
            .trial_coupon(&self.products)
            .map(ToOwned::to_owned)
    }

    /// Idempotently grant the current billing period's included usage credit for a
    /// metered subscription that carries a flat base fee. The credit equals that
    /// base fee and forms a single fungible pool scoped to the subscription's
    /// metered usage prices, expiring at period end (use-it-or-lose-it). A no-op if
    /// the subscription has no base fee or this period's grant already exists.
    pub async fn ensure_period_credit(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<PeriodCredit, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        let subscription = self
            .get_subscription_expand(&subscription_id, vec!["items".into()])
            .await?;

        // The included credit equals the subscription's flat base fee. A metered
        // plan without a configured base price (e.g. a grandfathered Team plan) is
        // a safe no-op.
        let base_cents: HashMap<&PriceId, i64> = self
            .products
            .all_base_prices()
            .filter_map(|price| price.unit_amount.map(|cents| (&price.id, cents)))
            .collect();
        let Some(value_cents) = matched_base_cents(
            subscription.items.data.iter().map(|item| &item.price.id),
            &base_cents,
        ) else {
            return Ok(PeriodCredit::NotApplicable);
        };

        let customer_id = subscription.customer.id().clone();
        // All subscription items share the same billing period.
        let item = subscription
            .items
            .data
            .first()
            .ok_or_else(|| BillingError::NoSubscriptionItem(subscription_id.clone()))?;
        let period_start = item.current_period_start;
        let period_end = item.current_period_end;

        // The credit applies to the metered usage prices (every item except the
        // flat base fee), forming a single fungible pool.
        let credit_price_ids: Vec<String> = subscription
            .items
            .data
            .iter()
            .map(|item| &item.price.id)
            .filter(|id| !base_cents.contains_key(*id))
            .map(ToString::to_string)
            .collect();

        // Dedup: skip if a Bencher-managed grant for this period already exists.
        // Stripe lists grants newest-first by `created` and cannot filter by
        // metadata, and a customer may have unrelated grants (manual dashboard
        // credits, refunds), so scan a page of recent grants for our marker plus
        // period rather than trusting the single newest. Our current-period grant
        // is recent, so it sits well within one page. The idempotency key guards
        // concurrent attempts; this guards across the sweep's daily cadence.
        let marker = period_start.to_string();
        let grants = ListBillingCreditGrant::new()
            .customer(customer_id.to_string())
            .limit(100)
            .send(&self.client)
            .await?;
        if period_already_granted(
            grants.data.iter().map(|grant| {
                (
                    grant
                        .metadata
                        .get(CREDIT_BENCHER_CLOUD_KEY)
                        .map(String::as_str),
                    grant.metadata.get(CREDIT_PERIOD_KEY).map(String::as_str),
                )
            }),
            &marker,
        ) {
            return Ok(PeriodCredit::AlreadyGranted);
        }

        self.create_credit_grant(
            &customer_id,
            value_cents,
            credit_price_ids,
            period_start,
            period_end,
        )
        .await?;
        Ok(PeriodCredit::Granted)
    }

    async fn create_credit_grant(
        &self,
        customer_id: &CustomerId,
        value_cents: i64,
        credit_price_ids: Vec<String>,
        period_start: stripe_types::Timestamp,
        period_end: stripe_types::Timestamp,
    ) -> Result<(), BillingError> {
        let prices = credit_price_ids
            .into_iter()
            .map(CreateBillingCreditGrantApplicabilityConfigScopePrices::new)
            .collect();
        let scope = CreateBillingCreditGrantApplicabilityConfigScope {
            price_type: None,
            prices: Some(prices),
        };
        let amount = CreateBillingCreditGrantAmount {
            monetary: Some(CreateBillingCreditGrantAmountMonetary::new(
                Currency::USD,
                value_cents,
            )),
            type_: CreateBillingCreditGrantAmountType::Monetary,
        };
        let metadata = HashMap::from([
            (CREDIT_BENCHER_CLOUD_KEY.to_owned(), "true".to_owned()),
            (CREDIT_PERIOD_KEY.to_owned(), period_start.to_string()),
        ]);
        // Idempotency key from customer + period so concurrent grant attempts
        // (e.g. plan creation overlapping the daily sweep) collapse to one grant.
        // The list-check above handles dedup across periods (beyond the key's TTL).
        let idempotency_key =
            IdempotencyKey::new(format!("bencher-credit:{customer_id}:{period_start}"))?;
        // Do not set `effective_at`. Stripe rejects a timestamp before its server
        // "now", and `period_start` is in the past for an already-active
        // subscription. Omitting it defaults to Stripe's "now" (also sidestepping
        // clock skew between us and Stripe). The credit still covers this period's
        // metered usage, which is invoiced at `expires_at` (period end).
        CreateBillingCreditGrant::new(
            amount,
            CreateBillingCreditGrantApplicabilityConfig { scope },
        )
        .customer(customer_id.to_string())
        .expires_at(period_end)
        .metadata(metadata)
        .name(CREDIT_NAME)
        .customize()
        .request_strategy(RequestStrategy::Idempotent(idempotency_key))
        .send(&self.client)
        .await
        .map(|_grant| ())
        .map_err(Into::into)
    }

    /// Immediately cancel a metered subscription. Used by the admin-only DELETE
    /// path; the organization downgrades to Free right away.
    pub async fn cancel_metered_subscription(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<Subscription, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        self.cancel_subscription(&subscription_id).await
    }

    /// Schedule (`true`) or clear (`false`) a metered subscription's
    /// cancel-at-period-end. With `true` the customer keeps access through the
    /// period they have already paid for; the subscription stays active until it
    /// then lapses (reconciled lazily on read and by the daily sweep). With
    /// `false` a scheduled cancellation is cleared, resuming the subscription.
    /// Used by the self-service PATCH path.
    pub async fn set_metered_cancel_at_period_end(
        &self,
        metered_plan_id: &MeteredPlanId,
        cancel_at_period_end: bool,
    ) -> Result<Subscription, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        UpdateSubscription::new(subscription_id)
            .cancel_at_period_end(cancel_at_period_end)
            .send(&self.client)
            .await
            .map_err(Into::into)
    }

    pub async fn cancel_licensed_subscription(
        &self,
        licensed_plan_id: &LicensedPlanId,
    ) -> Result<Subscription, BillingError> {
        let subscription_id: SubscriptionId = licensed_plan_id.as_ref().into();
        self.cancel_subscription(&subscription_id).await
    }

    async fn cancel_subscription(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<Subscription, BillingError> {
        CancelSubscription::new(subscription_id.clone())
            .send(&self.client)
            .await
            .map_err(Into::into)
    }
}

/// The included-credit amount (the matched flat base price's cents) for a
/// subscription, given a map of configured base price ID to cents. `None` if the
/// subscription carries no configured base fee. Gates the included-usage credit to
/// base-fee subscriptions and sizes it to the base fee.
fn matched_base_cents<'a>(
    mut item_price_ids: impl Iterator<Item = &'a PriceId>,
    base_cents: &HashMap<&'a PriceId, i64>,
) -> Option<i64> {
    item_price_ids.find_map(|id| base_cents.get(id).copied())
}

/// Whether a Bencher-managed credit grant for the given billing-period marker
/// already exists. Each item is `(bencher_cloud_marker, period_start)`; only
/// grants carrying our marker count, so unrelated grants (manual credits,
/// refunds) are ignored even when newer. (Per-period dedup for
/// `ensure_period_credit`.)
fn period_already_granted<'a>(
    mut grants: impl Iterator<Item = (Option<&'a str>, Option<&'a str>)>,
    marker: &'a str,
) -> bool {
    grants.any(|(managed, period)| managed == Some("true") && period == Some(marker))
}

fn into_payment_card(card: JsonCard) -> CreatePaymentMethodCardDetailsParams {
    let JsonCard {
        number,
        exp_month,
        exp_year,
        cvc,
    } = card;
    let exp_month: i64 = i32::from(exp_month).into();
    let exp_year: i64 = i32::from(exp_year).into();
    let number: String = number.into();
    let mut params = CreatePaymentMethodCardDetailsParams::new(exp_month, exp_year, number);
    params.cvc = Some(cvc.into());
    params
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use bencher_json::{
        Entitlements, OrganizationUuid, PlanLevel, PlanStatus, UserUuid,
        organization::plan::{DEFAULT_PRICE_NAME, METRICS_METER_NAME, RUNNER_MINUTES_METER_NAME},
        system::{
            config::{JsonBilling, JsonProduct, JsonProducts},
            payment::{JsonCard, JsonCustomer},
        },
    };
    use chrono::{Datelike as _, Utc};
    use literally::hmap;
    use pretty_assertions::assert_eq;
    use stripe_shared::{CustomerId, PaymentMethodId};

    use rustls::crypto::aws_lc_rs::default_provider;

    use crate::{Biller, PeriodCredit};

    use super::{METER_CUSTOMER_KEY, METER_VALUE_KEY, matched_base_cents, period_already_granted};

    #[test]
    fn matched_base_cents_returns_base_fee() {
        let base: stripe_product::PriceId = "price_base".parse().unwrap();
        let metered: stripe_product::PriceId = "price_metered".parse().unwrap();
        let other: stripe_product::PriceId = "price_other".parse().unwrap();
        let base_cents: HashMap<&stripe_product::PriceId, i64> = HashMap::from([(&base, 2_000)]);
        // A subscription carrying the base price yields that base price's cents.
        assert_eq!(
            matched_base_cents([&metered, &base].into_iter(), &base_cents),
            Some(2_000),
        );
        // A subscription without a configured base price yields nothing.
        assert_eq!(
            matched_base_cents([&metered, &other].into_iter(), &base_cents),
            None,
        );
    }

    #[test]
    fn period_already_granted_matches_marker() {
        // A Bencher-managed grant for the period counts.
        assert!(period_already_granted(
            [(Some("true"), Some("100")), (None, None)].into_iter(),
            "100",
        ));
        // A non-Bencher grant on top does not hide a real Bencher grant deeper down.
        assert!(period_already_granted(
            [(None, Some("100")), (Some("true"), Some("100"))].into_iter(),
            "100",
        ));
        // A non-Bencher grant for the period (no marker) is not counted as ours.
        assert!(!period_already_granted(
            [(None, Some("100"))].into_iter(),
            "100",
        ));
        // A different period does not match.
        assert!(!period_already_granted(
            [(Some("true"), Some("100"))].into_iter(),
            "200",
        ));
        assert!(!period_already_granted(
            std::iter::empty::<(Option<&str>, Option<&str>)>(),
            "100",
        ));
    }

    const TEST_BILLING_KEY: &str = "TEST_BILLING_KEY";

    fn billing_key() -> Option<String> {
        std::env::var(TEST_BILLING_KEY).ok()
    }

    fn products() -> JsonProducts {
        JsonProducts {
            pro: JsonProduct {
                id: "prod_UizgEJP4gIENBi".into(),
                metered: HashMap::new(),
                licensed: HashMap::new(),
                base: hmap! {
                    "default".to_owned() => "price_1TjXgeKal5vzTlmhZzr88bki".to_owned(),
                },
                trial_coupon: Some("m8uHY6oa".to_owned()),
            },
            team: JsonProduct {
                id: "prod_NKz5B9dGhDiSY1".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1T8NRdKal5vzTlmhBfL9IdMi".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1O4XlwKal5vzTlmh0n0wtplQ".to_owned(),
                },
                base: HashMap::new(),
                trial_coupon: None,
            },
            enterprise: JsonProduct {
                id: "prod_NLC7fDet2C8Nmk".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1T8NStKal5vzTlmhPBxy2izR".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1O4Xo1Kal5vzTlmh1KrcEbq0".to_owned(),
                },
                base: HashMap::new(),
                trial_coupon: None,
            },
            metrics: JsonProduct {
                id: "prod_UjlAPYSw7n3RDq".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1TkHeSKal5vzTlmhijDCkWDe".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1TkHeSKal5vzTlmhPKXPOW7c".to_owned(),
                },
                base: HashMap::new(),
                trial_coupon: None,
            },
            bare_metal: JsonProduct {
                id: "prod_U6a28ecwqRZHYz".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1T8Ms4Kal5vzTlmhSAPxSbrT".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1TCWdkKal5vzTlmh9fdahWqY".to_owned(),
                },
                base: HashMap::new(),
                trial_coupon: None,
            },
        }
    }

    async fn setup_customer_payment_method(biller: &Biller) -> (CustomerId, PaymentMethodId) {
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
        assert!(
            biller
                .get_customer(&json_customer.email)
                .await
                .unwrap()
                .is_none()
        );
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
        (customer_id, payment_method_id)
    }

    #[expect(clippy::too_many_arguments, reason = "test helper")]
    async fn metered_subscription(
        biller: &Biller,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        plan_level: PlanLevel,
        price_name: String,
        runner_minutes: usize,
        metrics_quantity: u32,
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

        let subscription_id = &create_subscription.id;
        let get_subscription = biller.get_subscription(subscription_id).await.unwrap();
        assert_eq!(create_subscription.id, get_subscription.id);

        let metered_plan_id = &subscription_id.as_ref().parse().unwrap();
        let json_plan = biller.get_metered_plan(metered_plan_id).await.unwrap();
        // The plan level is derived from the metered plan item's Stripe product
        // name. For Pro that item lives on the `metrics` product, exercising the
        // "Bencher Metrics" -> PlanLevel::Pro mapping.
        assert_eq!(json_plan.level, plan_level);

        let (plan_status, customer_id) = biller
            .get_metered_plan_status(metered_plan_id)
            .await
            .unwrap();
        assert_eq!(plan_status, PlanStatus::Active);

        // Only Pro carries a flat base fee, so only Pro grants an included-usage
        // credit; the call is a no-op for Team/Enterprise. Granting twice must be
        // idempotent (regression: a past `effective_at` was rejected by Stripe).
        let first_credit = biller.ensure_period_credit(metered_plan_id).await.unwrap();
        let second_credit = biller.ensure_period_credit(metered_plan_id).await.unwrap();
        if plan_level == PlanLevel::Pro {
            assert_eq!(first_credit, PeriodCredit::Granted);
            assert_eq!(second_credit, PeriodCredit::AlreadyGranted);
        } else {
            assert_eq!(first_credit, PeriodCredit::NotApplicable);
            assert_eq!(second_credit, PeriodCredit::NotApplicable);
        }

        record_runner_usage(biller, &customer_id, runner_minutes).await;
        record_metrics_usage(biller, &customer_id, metrics_quantity).await;

        // PATCH path: schedule cancel-at-period-end, then clear it (resume). The
        // subscription stays active throughout.
        let scheduled = biller
            .set_metered_cancel_at_period_end(metered_plan_id, true)
            .await
            .unwrap();
        assert!(scheduled.cancel_at_period_end);
        let resumed = biller
            .set_metered_cancel_at_period_end(metered_plan_id, false)
            .await
            .unwrap();
        assert!(!resumed.cancel_at_period_end);

        // DELETE path: immediate cancel.
        biller
            .cancel_metered_subscription(&subscription_id.parse().unwrap())
            .await
            .unwrap();
        let (plan_status, _) = biller
            .get_metered_plan_status(metered_plan_id)
            .await
            .unwrap();
        assert_eq!(plan_status, PlanStatus::Canceled);
    }

    async fn licensed_subscription(
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

        let subscription_id = &create_subscription.id;
        let get_subscription = biller.get_subscription(subscription_id).await.unwrap();
        assert_eq!(create_subscription.id, get_subscription.id);

        let licensed_plan_id = &subscription_id.as_ref().parse().unwrap();
        biller.get_licensed_plan(licensed_plan_id).await.unwrap();

        let plan_status = biller
            .get_licensed_plan_status(licensed_plan_id)
            .await
            .unwrap();
        assert_eq!(plan_status, PlanStatus::Active);

        biller
            .cancel_licensed_subscription(licensed_plan_id)
            .await
            .unwrap();

        let plan_status = biller
            .get_licensed_plan_status(licensed_plan_id)
            .await
            .unwrap();
        assert_eq!(plan_status, PlanStatus::Canceled);
    }

    async fn record_runner_usage(biller: &Biller, customer_id: &CustomerId, runner_minutes: usize) {
        for _ in 0..runner_minutes {
            let event = biller.record_runner_usage(customer_id, 1).await.unwrap();
            assert_eq!(event.event_name, RUNNER_MINUTES_METER_NAME);
            assert_eq!(
                event.payload.get(METER_CUSTOMER_KEY),
                Some(&customer_id.to_string()),
            );
            assert_eq!(event.payload.get(METER_VALUE_KEY), Some(&"1".to_owned()),);
        }
    }

    async fn record_metrics_usage(biller: &Biller, customer_id: &CustomerId, quantity: u32) {
        let event = biller
            .record_metrics_usage(customer_id, quantity)
            .await
            .unwrap();
        assert_eq!(event.event_name, METRICS_METER_NAME);
        assert_eq!(
            event.payload.get(METER_CUSTOMER_KEY),
            Some(&customer_id.to_string()),
        );
        assert_eq!(
            event.payload.get(METER_VALUE_KEY),
            Some(&quantity.to_string()),
        );
    }

    fn make_price(price_id: &str) -> stripe_shared::Price {
        stripe_shared::Price {
            active: false,
            billing_scheme: stripe_shared::PriceBillingScheme::PerUnit,
            created: 0,
            currency: stripe_types::Currency::USD,
            currency_options: None,
            custom_unit_amount: None,
            id: price_id.parse().unwrap(),
            livemode: false,
            lookup_key: None,
            metadata: HashMap::new(),
            nickname: None,
            product: stripe_types::Expandable::Id(
                "prod_test".parse::<stripe_shared::ProductId>().unwrap(),
            ),
            recurring: None,
            tax_behavior: None,
            tiers: None,
            tiers_mode: None,
            transform_quantity: None,
            type_: stripe_shared::PriceType::Recurring,
            unit_amount: None,
            unit_amount_decimal: None,
        }
    }

    fn make_plan() -> stripe_shared::Plan {
        stripe_shared::Plan {
            active: false,
            amount: None,
            amount_decimal: None,
            billing_scheme: stripe_shared::PlanBillingScheme::PerUnit,
            created: 0,
            currency: stripe_types::Currency::USD,
            id: "plan_test".parse().unwrap(),
            interval: stripe_shared::PlanInterval::Month,
            interval_count: 1,
            livemode: false,
            metadata: None,
            meter: None,
            nickname: None,
            product: None,
            tiers: None,
            tiers_mode: None,
            transform_usage: None,
            trial_period_days: None,
            usage_type: stripe_shared::PlanUsageType::Metered,
        }
    }

    fn make_subscription_item(price_id: &str) -> stripe_billing::SubscriptionItem {
        stripe_billing::SubscriptionItem {
            billing_thresholds: None,
            created: 0,
            current_period_end: 0,
            current_period_start: 0,
            discounts: vec![],
            id: format!("si_test_{price_id}").parse().unwrap(),
            metadata: HashMap::new(),
            plan: make_plan(),
            price: make_price(price_id),
            quantity: None,
            subscription: "sub_test".to_owned(),
            tax_rates: None,
        }
    }

    #[test]
    fn filter_subscription_items_single_match() {
        let sub_id: stripe_billing::SubscriptionId = "sub_test".parse().unwrap();
        let known: stripe_product::PriceId = "price_known".parse().unwrap();
        let items = vec![
            make_subscription_item("price_known"),
            make_subscription_item("price_unknown"),
        ];
        let price_ids = HashSet::from([&known]);
        let filtered = Biller::filter_subscription_items(&sub_id, items, &price_ids).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.first().unwrap().price.id, known);
    }

    #[test]
    fn filter_subscription_items_no_match() {
        let sub_id: stripe_billing::SubscriptionId = "sub_test".parse().unwrap();
        let known: stripe_product::PriceId = "price_known".parse().unwrap();
        let items = vec![
            make_subscription_item("price_a"),
            make_subscription_item("price_b"),
        ];
        let price_ids = HashSet::from([&known]);
        let err = Biller::filter_subscription_items(&sub_id, items, &price_ids).unwrap_err();
        assert!(
            matches!(err, crate::BillingError::NoMatchingSubscriptionItem(id, 2) if id == sub_id)
        );
    }

    #[test]
    fn filter_subscription_items_empty_input() {
        let sub_id: stripe_billing::SubscriptionId = "sub_test".parse().unwrap();
        let known: stripe_product::PriceId = "price_known".parse().unwrap();
        let price_ids = HashSet::from([&known]);
        let err = Biller::filter_subscription_items(&sub_id, vec![], &price_ids).unwrap_err();
        assert!(
            matches!(err, crate::BillingError::NoMatchingSubscriptionItem(id, 0) if id == sub_id)
        );
    }

    #[test]
    fn filter_subscription_items_all_match() {
        let sub_id: stripe_billing::SubscriptionId = "sub_test".parse().unwrap();
        let known: stripe_product::PriceId = "price_known".parse().unwrap();
        let items = vec![make_subscription_item("price_known")];
        let price_ids = HashSet::from([&known]);
        let filtered = Biller::filter_subscription_items(&sub_id, items, &price_ids).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.first().unwrap().price.id, known);
    }

    #[test]
    fn filter_subscription_items_multiple_known_ids() {
        let sub_id: stripe_billing::SubscriptionId = "sub_test".parse().unwrap();
        let known_a: stripe_product::PriceId = "price_a".parse().unwrap();
        let known_b: stripe_product::PriceId = "price_b".parse().unwrap();
        let items = vec![
            make_subscription_item("price_a"),
            make_subscription_item("price_b"),
            make_subscription_item("price_c"),
        ];
        let price_ids = HashSet::from([&known_a, &known_b]);
        let filtered = Biller::filter_subscription_items(&sub_id, items, &price_ids).unwrap();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn get_subscription_item_after_filter() {
        let known: stripe_product::PriceId = "price_known".parse().unwrap();
        let sub_id: stripe_billing::SubscriptionId = "sub_test".parse().unwrap();
        let items = vec![
            make_subscription_item("price_known"),
            make_subscription_item("price_old_meter"),
        ];
        let price_ids = HashSet::from([&known]);
        let filtered = Biller::filter_subscription_items(&sub_id, items, &price_ids).unwrap();
        let result = Biller::get_subscription_item(&sub_id, filtered).unwrap();
        assert_eq!(result.price.id, known);
    }

    #[test]
    fn filter_subscription_items_excludes_bare_metal() {
        let sub_id: stripe_billing::SubscriptionId = "sub_test".parse().unwrap();
        let team_price: stripe_product::PriceId = "price_team".parse().unwrap();
        let bare_metal_price: stripe_product::PriceId = "price_bare_metal".parse().unwrap();
        let items = vec![
            make_subscription_item("price_team"),
            make_subscription_item("price_bare_metal"),
        ];
        // Only team/enterprise prices, not bare_metal
        let price_ids = HashSet::from([&team_price]);
        let filtered = Biller::filter_subscription_items(&sub_id, items, &price_ids).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.first().unwrap().price.id, team_price);
        // Verify bare_metal was excluded
        assert!(
            filtered
                .iter()
                .all(|item| item.price.id != bare_metal_price)
        );
    }

    #[test]
    fn get_subscription_item_no_items() {
        let sub_id: stripe_billing::SubscriptionId = "sub_test".parse().unwrap();
        Biller::get_subscription_item(&sub_id, vec![]).unwrap_err();
    }

    #[test]
    fn get_subscription_item_multiple_items() {
        let sub_id: stripe_billing::SubscriptionId = "sub_test".parse().unwrap();
        let items = vec![
            make_subscription_item("price_a"),
            make_subscription_item("price_b"),
        ];
        Biller::get_subscription_item(&sub_id, items).unwrap_err();
    }

    // Note: To run this test locally run:
    // `export TEST_BILLING_KEY=...`
    #[tokio::test]
    async fn biller() {
        let Some(billing_key) = billing_key() else {
            return;
        };
        default_provider()
            .install_default()
            .expect("Failed to install default crypto provider");
        let json_billing = JsonBilling {
            secret_key: billing_key.parse().unwrap(),
            products: products(),
        };
        let biller = Biller::new(json_billing).await.unwrap();

        let (customer_id, payment_method_id) = setup_customer_payment_method(&biller).await;

        // Pro Metered Plan
        let organization = OrganizationUuid::new();
        metered_subscription(
            &biller,
            organization,
            customer_id.clone(),
            payment_method_id.clone(),
            PlanLevel::Pro,
            DEFAULT_PRICE_NAME.into(),
            3,
            7,
        )
        .await;

        // Team Metered Plan
        let organization = OrganizationUuid::new();
        metered_subscription(
            &biller,
            organization,
            customer_id.clone(),
            payment_method_id.clone(),
            PlanLevel::Team,
            DEFAULT_PRICE_NAME.into(),
            5,
            10,
        )
        .await;

        // Team Licensed Plan
        let organization = OrganizationUuid::new();
        licensed_subscription(
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
        metered_subscription(
            &biller,
            organization,
            customer_id.clone(),
            payment_method_id.clone(),
            PlanLevel::Enterprise,
            DEFAULT_PRICE_NAME.into(),
            10,
            25,
        )
        .await;

        // Enterprise Licensed Plan
        let organization = OrganizationUuid::new();
        licensed_subscription(
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
