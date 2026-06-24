use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use bencher_json::{
    DateTime, Email, Entitlements, LicensedPlanId, MeteredPlanId, OrganizationUuid, PlanLevel,
    PlanStatus,
    organization::plan::{
        ACTIVE_SERIES_METER_NAME, JsonCardDetails, JsonPlan, METRICS_METER_NAME,
        RUNNER_MINUTES_METER_NAME,
    },
    system::{
        config::JsonBilling,
        payment::{JsonCard, JsonCheckout, JsonCustomer},
    },
};
use stripe::Client as StripeClient;
use stripe_billing::{
    BillingMeterEvent, Subscription, SubscriptionId, SubscriptionItem, SubscriptionStatus,
    billing_meter_event::CreateBillingMeterEvent,
    subscription::{
        CancelSubscription, CreateSubscription, CreateSubscriptionItems, RetrieveSubscription,
        UpdateSubscription,
    },
};
use stripe_checkout::{
    CheckoutSessionId, CheckoutSessionMode, CheckoutSessionUiMode,
    checkout_session::{
        CreateCheckoutSession, CreateCheckoutSessionConsentCollection,
        CreateCheckoutSessionConsentCollectionTermsOfService, CreateCheckoutSessionLineItems,
        CreateCheckoutSessionLineItemsAdjustableQuantity, CreateCheckoutSessionPaymentMethodTypes,
        CreateCheckoutSessionSubscriptionData, RetrieveCheckoutSession,
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

/// Free-trial length (in days) for a new Pro subscription. Set per subscription and per
/// Checkout Session (a subscription-level trial), not as a per-price default (which is
/// incompatible with Checkout). A native trial generates no invoice for the period, so
/// all usage (active series and bare-metal runner minutes) is free during the trial, not
/// just the base fee.
const PRO_TRIAL_PERIOD_DAYS: u32 = 30;

#[derive(Clone)]
pub struct Biller {
    client: StripeClient,
    products: Products,
}

/// A metered subscription's current billing snapshot, fetched in one call: its
/// [`PlanStatus`] (the active/lapsed gate), the Stripe [`CustomerId`] to post usage
/// for, the [`PlanLevel`], and the current period bounds (the active-series count
/// window).
#[derive(Debug, Clone)]
pub struct MeteredPlanBilling {
    pub status: PlanStatus,
    pub customer_id: CustomerId,
    pub level: PlanLevel,
    pub current_period_start: DateTime,
    pub current_period_end: DateTime,
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
    // Bencher Cloud self-serve tier. Carries the price name for the single tiered
    // active-series price (on the `pro` product) that bills both the flat monthly base
    // fee (tier 1) and the per-series step-ups. Gets a native free trial. This tier has
    // no licensed (Self-Hosted) form.
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

    // The native free-trial length (in days) for this plan, if any. Only the Pro
    // self-serve tier gets a trial.
    fn trial_period_days(&self) -> Option<u32> {
        match self {
            Self::Pro(_) => Some(PRO_TRIAL_PERIOD_DAYS),
            Self::Free | Self::Team(_) | Self::Enterprise(_) => None,
        }
    }

    fn into_plus_price(
        self,
        products: &Products,
    ) -> Result<(&Price, Option<Entitlements>), BillingError> {
        Ok(match self {
            PlusPlan::Free => return Err(BillingError::ProductLevelFree),
            // Pro bills on its own tiered active-series price (base fee + step-ups),
            // which lives on the `pro` product, not the shared `metrics` product.
            PlusPlan::Pro(price_name) => (
                products
                    .pro
                    .metered
                    .get(price_name.as_str())
                    .ok_or_else(|| BillingError::PriceNotFound(price_name.to_string()))?,
                None,
            ),
            PlusPlan::Team(plus_usage) => match plus_usage {
                // Metered metrics bill on the shared `metrics` ("Bencher Metrics")
                // product across paid tiers; only the licensed price stays on the
                // tier's own product.
                PlusUsage::Metered(price_name) => (
                    products
                        .metrics
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
                // Metered metrics bill on the shared `metrics` product (see Team).
                PlusUsage::Metered(price_name) => (
                    products
                        .metrics
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
        let trial_period_days = plus_plan.trial_period_days();
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

        // Pro's tiered active-series price carries the base fee, so there is no
        // separate base line item.
        let line_items = vec![plus_line_item, bare_metal_line_item];

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
        // Native free trial set at the subscription level (Checkout-compatible), not as
        // a per-price default. No charge during the trial; billing starts at trial end.
        sub_data.trial_period_days = trial_period_days;

        let create_checkout_session = CreateCheckoutSession::new()
            .ui_mode(CheckoutSessionUiMode::HostedPage)
            .customer(customer.to_string())
            .payment_method_types(vec![CreateCheckoutSessionPaymentMethodTypes::Card])
            .currency(Currency::USD)
            .mode(CheckoutSessionMode::Subscription)
            .line_items(line_items)
            .consent_collection(consent)
            .subscription_data(sub_data)
            .success_url(return_url);
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
        let trial_period_days = plus_plan.trial_period_days();
        let (plus_price, entitlements) = plus_plan.into_plus_price(&self.products)?;

        // Pro's tiered active-series price carries the base fee, so there is no
        // separate base item.
        let mut plus_item = CreateSubscriptionItems::new();
        plus_item.price = Some(plus_price.id.to_string());
        plus_item.quantity = entitlements.map(Into::into);
        let mut bare_metal_item = CreateSubscriptionItems::new();
        bare_metal_item.price = Some(bare_metal_price.id.to_string());
        let items = vec![plus_item, bare_metal_item];

        let mut create_subscription = CreateSubscription::new()
            .customer(customer_id.to_string())
            .items(items)
            .default_payment_method(payment_method_id.to_string())
            .metadata(
                [(METADATA_ORGANIZATION.to_owned(), organization.to_string())]
                    .into_iter()
                    .collect::<HashMap<String, String>>(),
            );
        // Native free trial (subscription-level): no charge during the trial; billing
        // starts at trial end.
        if let Some(days) = trial_period_days {
            create_subscription = create_subscription.trial_period_days(days);
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

    /// Derive a paid subscription's plan level from its line items. Pro is the only
    /// paid tier that carries the tiered active-series price (on the `pro` product), so
    /// a Pro active-series item identifies the plan level; any other paid metered
    /// subscription is Team.
    fn subscription_plan_level(&self, subscription: &Subscription) -> PlanLevel {
        let pro_price_ids: HashSet<&PriceId> = self
            .products
            .pro
            .metered
            .values()
            .map(|price| &price.id)
            .collect();
        let is_pro = subscription
            .items
            .data
            .iter()
            .any(|item| pro_price_ids.contains(&item.price.id));
        plan_level_from_pro_price(is_pro)
    }

    async fn get_plan(&self, subscription_id: &SubscriptionId) -> Result<JsonPlan, BillingError> {
        let subscription = self
            .get_subscription_expand(
                subscription_id,
                vec!["customer".into(), "default_payment_method".into()],
            )
            .await?;

        let Some(organization) = subscription.metadata.get(METADATA_ORGANIZATION) else {
            return Err(BillingError::NoOrganization(subscription_id.clone()));
        };
        let organization = organization
            .parse()
            .map_err(|e| BillingError::BadOrganizationUuid(organization.clone(), e))?;

        let level = self.subscription_plan_level(&subscription);

        let plan_price_ids = self.products.plan_price_ids();
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
        let unit_amount = Self::get_plan_unit_amount(
            &subscription_item,
            self.products
                .tier_base_fee_cents(&subscription_item.price.id),
        )?;

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

    /// The plan's displayed per-period unit amount (cents). For a flat price this is the
    /// price's `unit_amount`. A tiered price (the Pro active-series price) exposes no
    /// flat `unit_amount`, so the caller passes `fallback_cents` (the base monthly fee,
    /// tier 1 `flat_amount`) to use instead.
    fn get_plan_unit_amount(
        subscription_item: &SubscriptionItem,
        fallback_cents: Option<i64>,
    ) -> Result<u64, BillingError> {
        let price = &subscription_item.price;
        let cents = price
            .unit_amount
            .or(fallback_cents)
            .ok_or_else(|| BillingError::NoUnitAmount(price.id.clone()))?;
        u64::try_from(cents).map_err(Into::into)
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

    /// One-fetch billing snapshot: status, the Stripe customer to post usage for, the
    /// plan level, and the current period bounds. Expands the subscription's items
    /// because the period bounds live on them (all items share one period). Used on the
    /// report path to gate access and to supply the active-series count window.
    pub async fn get_metered_plan_billing(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<MeteredPlanBilling, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        let subscription = self
            .get_subscription_expand(&subscription_id, vec!["items".into()])
            .await?;
        let (status, customer_id, level) = self.subscription_status_snapshot(&subscription);
        // All subscription items share the same billing period.
        let item = subscription
            .items
            .data
            .first()
            .ok_or_else(|| BillingError::NoSubscriptionItem(subscription_id.clone()))?;
        let current_period_start = item.current_period_start.try_into().map_err(|e| {
            BillingError::DateTime(subscription_id.clone(), item.current_period_start, e)
        })?;
        let current_period_end = item.current_period_end.try_into().map_err(|e| {
            BillingError::DateTime(subscription_id.clone(), item.current_period_end, e)
        })?;
        Ok(MeteredPlanBilling {
            status,
            customer_id,
            level,
            current_period_start,
            current_period_end,
        })
    }

    /// Resolve a metered subscription's live status for re-subscription gating:
    /// `Some(status)` when the subscription exists, and `None` when Stripe reports it no
    /// longer exists (a definitive 404, safe to treat as gone). Returns an error only for
    /// indeterminate failures (network, 5xx, rate limit), so a transient outage is never
    /// mistaken for "gone" (which would risk pruning a still-live subscription). The
    /// caller distinguishes terminal from recoverable (dunning) statuses.
    pub async fn metered_plan_status(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<Option<PlanStatus>, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        match self.get_subscription(&subscription_id).await {
            Ok(subscription) => Ok(Some(Self::map_status(&subscription.status))),
            Err(BillingError::Stripe(stripe::StripeError::Stripe(_, 404))) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Derive a subscription's status, customer, and plan level for
    /// [`Self::get_metered_plan_billing`]. Deriving the level does not require the
    /// subscription's items to be expanded (item price ids are present without it).
    fn subscription_status_snapshot(
        &self,
        subscription: &Subscription,
    ) -> (PlanStatus, CustomerId, PlanLevel) {
        (
            Self::map_status(&subscription.status),
            subscription.customer.id().clone(),
            self.subscription_plan_level(subscription),
        )
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
        self.record_metered_usage(METRICS_METER_NAME, customer_id, quantity, None)
            .await
    }

    /// Post an organization's cumulative period-to-date active-series count to the
    /// `active_series` meter (which backs the Pro tiered price), stamped with `when` (the
    /// time the count was taken). Billed with `last` aggregation: the explicit timestamp
    /// orders concurrent posts by `when` (to one-second granularity) rather than by
    /// Stripe's receipt order, so reports more than a second apart bill the later count
    /// deterministically; a same-second tie self-heals on the next report. Posting after
    /// each report makes the final post of the period the period total, and a missed post
    /// self-heals on the next report. Mirrors [`Self::record_metrics_usage`].
    pub async fn record_series_usage(
        &self,
        customer_id: &CustomerId,
        quantity: u32,
        when: DateTime,
    ) -> Result<BillingMeterEvent, BillingError> {
        self.record_metered_usage(
            ACTIVE_SERIES_METER_NAME,
            customer_id,
            quantity,
            Some(when.timestamp()),
        )
        .await
    }

    pub async fn record_runner_usage(
        &self,
        customer_id: &CustomerId,
        minutes: u32,
    ) -> Result<BillingMeterEvent, BillingError> {
        self.record_metered_usage(RUNNER_MINUTES_METER_NAME, customer_id, minutes, None)
            .await
    }

    /// `timestamp` (Unix seconds) is set on the meter event only for `last`-aggregation
    /// meters (the `active_series` meter), where it makes ordering deterministic; the
    /// `sum`-aggregation meters (metrics, runner minutes) pass `None` and use Stripe's
    /// receipt time.
    async fn record_metered_usage(
        &self,
        meter_name: &str,
        customer_id: &CustomerId,
        quantity: u32,
        timestamp: Option<i64>,
    ) -> Result<BillingMeterEvent, BillingError> {
        let mut event = CreateBillingMeterEvent::new(
            meter_name,
            HashMap::from([
                (METER_CUSTOMER_KEY.to_owned(), customer_id.to_string()),
                (METER_VALUE_KEY.to_owned(), quantity.to_string()),
            ]),
        );
        if let Some(timestamp) = timestamp {
            event = event.timestamp(timestamp);
        }
        event.send(&self.client).await.map_err(Into::into)
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
    /// then lapses (reconciled lazily on read). With
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

/// A paid subscription's plan level. Pro is the only paid tier that carries the tiered
/// active-series price (on the `pro` product), so a subscription with that item is Pro;
/// any other paid metered subscription is Team.
fn plan_level_from_pro_price(is_pro: bool) -> PlanLevel {
    if is_pro {
        PlanLevel::Pro
    } else {
        PlanLevel::Team
    }
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
        organization::plan::{
            ACTIVE_SERIES_METER_NAME, DEFAULT_PRICE_NAME, METRICS_METER_NAME,
            RUNNER_MINUTES_METER_NAME,
        },
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

    use crate::Biller;

    use super::{METER_CUSTOMER_KEY, METER_VALUE_KEY, plan_level_from_pro_price};

    #[test]
    fn plan_level_from_pro_price_resolves_tier() {
        // Pro is the only paid tier carrying the tiered active-series price, so it
        // resolves to Pro; any other paid metered subscription resolves to Team.
        assert_eq!(plan_level_from_pro_price(true), PlanLevel::Pro);
        assert_eq!(plan_level_from_pro_price(false), PlanLevel::Team);
    }

    #[test]
    fn get_plan_unit_amount_reads_price() {
        let mut item = make_subscription_item("price_metrics");
        item.price.unit_amount = Some(1);
        assert_eq!(Biller::get_plan_unit_amount(&item, None).unwrap(), 1);
    }

    #[test]
    fn get_plan_unit_amount_missing_errors() {
        // `make_subscription_item` builds a price with `unit_amount: None` and no tier
        // fallback, so the amount is unresolved.
        let item = make_subscription_item("price_metrics");
        let err = Biller::get_plan_unit_amount(&item, None).unwrap_err();
        assert!(matches!(err, crate::BillingError::NoUnitAmount(_)));
    }

    #[test]
    fn get_plan_unit_amount_tiered_uses_fallback() {
        // A tiered price exposes no flat `unit_amount`; the base-tier fee is used.
        let item = make_subscription_item("price_pro_tiered");
        assert_eq!(
            Biller::get_plan_unit_amount(&item, Some(2_000)).unwrap(),
            2_000,
        );
    }

    const TEST_BILLING_KEY: &str = "TEST_BILLING_KEY";

    fn billing_key() -> Option<String> {
        std::env::var(TEST_BILLING_KEY).ok()
    }

    fn products() -> JsonProducts {
        JsonProducts {
            pro: JsonProduct {
                id: "prod_UizgEJP4gIENBi".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1Tmo29Kal5vzTlmh9Qk5vtLi".to_owned(),
                },
                licensed: HashMap::new(),
            },
            team: JsonProduct {
                id: "prod_NKz5B9dGhDiSY1".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1T8NRdKal5vzTlmhBfL9IdMi".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1O4XlwKal5vzTlmh0n0wtplQ".to_owned(),
                },
            },
            enterprise: JsonProduct {
                id: "prod_NLC7fDet2C8Nmk".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1T8NStKal5vzTlmhPBxy2izR".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1O4Xo1Kal5vzTlmh1KrcEbq0".to_owned(),
                },
            },
            metrics: JsonProduct {
                id: "prod_UjlAPYSw7n3RDq".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1TkHeSKal5vzTlmhijDCkWDe".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1TkHeSKal5vzTlmhPKXPOW7c".to_owned(),
                },
            },
            bare_metal: JsonProduct {
                id: "prod_U6a28ecwqRZHYz".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1T8Ms4Kal5vzTlmhSAPxSbrT".to_owned(),
                },
                licensed: hmap! {
                    "default".to_owned() => "price_1TCWdkKal5vzTlmh9fdahWqY".to_owned(),
                },
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

    #[expect(
        clippy::too_many_arguments,
        reason = "test helper exercising the full metered subscription lifecycle"
    )]
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
        // The plan level is derived from the tiered active-series price: Pro carries it
        // and resolves to Pro; tiers without it resolve to Team.
        let expected_level = if plan_level == PlanLevel::Pro {
            PlanLevel::Pro
        } else {
            PlanLevel::Team
        };
        assert_eq!(json_plan.level, expected_level);

        let billing = biller
            .get_metered_plan_billing(metered_plan_id)
            .await
            .unwrap();
        // Pro starts in a native free trial, so it is `Trialing`; the other tiers have no
        // trial and are `Active` immediately.
        let expected_status = if plan_level == PlanLevel::Pro {
            PlanStatus::Trialing
        } else {
            PlanStatus::Active
        };
        assert_eq!(billing.status, expected_status);
        // The billing path derives the same level as the full plan fetch.
        assert_eq!(billing.level, expected_level);
        let customer_id = billing.customer_id;

        record_metrics_usage(biller, &customer_id, metrics_quantity).await;
        record_series_usage(biller, &customer_id, metrics_quantity).await;
        record_runner_usage(biller, &customer_id, runner_minutes).await;

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
        let billing = biller
            .get_metered_plan_billing(metered_plan_id)
            .await
            .unwrap();
        assert_eq!(billing.status, PlanStatus::Canceled);
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

    async fn record_series_usage(biller: &Biller, customer_id: &CustomerId, quantity: u32) {
        // Live Stripe test: a meter event needs a real, recent timestamp (a fixed
        // `DateTime::TEST` would be rejected as outside Stripe's accepted window).
        let event = biller
            .record_series_usage(customer_id, quantity, bencher_json::DateTime::now())
            .await
            .unwrap();
        assert_eq!(event.event_name, ACTIVE_SERIES_METER_NAME);
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
