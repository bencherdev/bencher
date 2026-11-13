use std::{collections::HashMap, fmt};

use bencher_json::{
    DateTime, Email, Entitlements, JsonPriceTier, Jwt, LicensedPlanId, MeteredPlanId,
    OrganizationUuid, PlanLevel, PlanStatus,
    organization::plan::{
        ACTIVE_SERIES_METER_NAME, CollectionMethod, JsonCardDetails, JsonNewPlan, JsonPlan,
        JsonPlanCheckout, JsonPlanSubscription, RUNNER_MINUTES_METER_NAME, TiersMode,
    },
    system::{
        config::JsonBilling,
        payment::{JsonCard, JsonCheckout, JsonCustomer},
    },
};
use bencher_license::{LicenseClaims, LicenseError, Licensor};
use stripe::Client as StripeClient;
use stripe_billing::{
    BillingMeterEvent, Subscription, SubscriptionId, SubscriptionItem, SubscriptionStatus,
    billing_meter_event::CreateBillingMeterEvent,
    billing_portal_session::CreateBillingPortalSession,
    subscription::{
        CancelSubscription, CreateSubscription, CreateSubscriptionItems, ListSubscription,
        ListSubscriptionStatus, RetrieveSubscription, UpdateSubscription,
    },
};
use stripe_checkout::{
    CheckoutSessionId, CheckoutSessionMode, CheckoutSessionUiMode,
    checkout_session::{
        CreateCheckoutSession, CreateCheckoutSessionConsentCollection,
        CreateCheckoutSessionConsentCollectionTermsOfService, CreateCheckoutSessionLineItems,
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
use stripe_product::Price;
use stripe_shared::{Customer, CustomerId, PriceTiersMode, SubscriptionCollectionMethod};
use stripe_types::{Currency, Expandable};

use crate::{
    AttachError, AttachRefusal, BillingError, LicenseTermError, PlanItemError, RenewalError,
    products::{Products, to_json_tier},
};

const METADATA_UUID: &str = "uuid";
const METADATA_ORGANIZATION: &str = "organization";
/// The Benchmark Series that a subscription for a Self-Hosted deal pays for.
const METADATA_SERIES: &str = "series";

/// What a plan read expands: the payer and its payment method for invoices, the
/// subscription's own payment method, and the tiers of each item's price.
const PLAN_EXPAND: [&str; 4] = [
    "customer",
    "customer.invoice_settings.default_payment_method",
    "default_payment_method",
    "items.data.price.tiers",
];

/// Stripe meter event payload key for the customer identifier.
const METER_CUSTOMER_KEY: &str = "stripe_customer_id";
/// Stripe meter event payload key for the usage value.
const METER_VALUE_KEY: &str = "value";

/// Free-trial length (in days) for a new Pro subscription. Set per subscription and per
/// Checkout Session (a subscription-level trial), not as a per-price default (which is
/// incompatible with Checkout). A native trial generates no invoice for the period, so
/// all usage (active series and bare-metal runner minutes) is free during the trial, not
/// just the base fee. Granted once per customer: a customer with any prior subscription
/// (any status, including canceled) gets no trial (see [`Biller::trial_period_days`]),
/// so canceling and re-subscribing cannot restart the trial.
const PRO_TRIAL_PERIOD_DAYS: u32 = 30;

#[derive(Clone)]
pub struct Biller {
    client: StripeClient,
    products: Products,
}

/// A metered subscription's current billing snapshot, fetched in one call.
///
/// It holds the subscription's [`PlanStatus`] (the active/lapsed gate), the Stripe
/// [`CustomerId`] to post usage for, and the current period bounds (the active-series
/// count window).
#[derive(Debug, Clone)]
pub struct MeteredPlanBilling {
    pub status: PlanStatus,
    pub customer_id: CustomerId,
    pub current_period_start: DateTime,
    pub current_period_end: DateTime,
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

/// A subscription attached to an organization, and its plan as the attach read it.
#[derive(Debug)]
pub enum Attachment {
    /// A Bencher Cloud plan for the organization.
    Metered(MeteredPlanId, JsonPlan),
    /// A Self-Hosted deal, with the first license that Bencher Cloud signed for it.
    Licensed(LicensedPlanId, Jwt, JsonPlan),
}

/// A licensed plan as Stripe has it now, and the term of the license that its subscription pays
/// for now.
#[derive(Debug)]
pub struct LicensedPlan {
    pub plan: JsonPlan,
    pub term: Result<LicenseTerm, LicenseTermError>,
}

impl LicensedPlan {
    /// The term to sign a new license for, in place of the license `held`: none unless the
    /// subscription is active and a license for its current term replaces `held`.
    pub fn renewal(&self, held: HeldLicense<'_>) -> Result<Option<LicenseTerm>, RenewalError> {
        renewal(self.plan.status, self.plan.level, &self.term, held)
    }
}

/// The license that a licensed plan holds, by whether Bencher Cloud's key verifies it.
#[derive(Debug, Clone, Copy)]
pub enum HeldLicense<'a> {
    Verified(&'a LicenseClaims),
    /// A license whose key no longer verifies, for example after the signing key changed, which
    /// a license for any term but an earlier one replaces.
    Unverified(&'a LicenseClaims),
}

/// The Benchmark Series and the paid term of the Self-Hosted license that a subscription pays
/// for: its `series` metadata, over its current period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LicenseTerm {
    pub series: Entitlements,
    /// The start of the period, so a server whose clock runs behind still takes the license.
    pub issued_at: DateTime,
    pub term_end: DateTime,
}

impl LicenseTerm {
    /// The term for `series` Benchmark Series over a period from `issued_at` to `term_end`.
    /// A period that ends when or before it starts has already ended.
    fn new(
        series: Entitlements,
        issued_at: DateTime,
        term_end: DateTime,
    ) -> Result<Self, LicenseTermError> {
        if term_end.timestamp() <= issued_at.timestamp() {
            return Err(LicenseTermError::Ended {
                issued_at,
                term_end,
            });
        }
        Ok(Self {
            series,
            issued_at,
            term_end,
        })
    }

    /// Sign the license for this term, for the Self-Hosted `organization`.
    pub fn sign(
        self,
        licensor: &Licensor,
        organization: OrganizationUuid,
    ) -> Result<Jwt, LicenseError> {
        licensor.new_license(organization, self.series, self.issued_at, self.term_end)
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

    /// A biller that sends every Stripe request to `stub`, for the stub's products. A rustls
    /// crypto provider must be installed first.
    #[cfg(any(test, feature = "test-stub"))]
    pub fn for_stub(stub: &crate::stripe_stub::StripeStub) -> Result<Self, BillingError> {
        Ok(Self {
            client: stripe::ClientBuilder::new("sk_test_stub")
                .url(stub.url())
                .build()?,
            products: crate::stripe_stub::products(),
        })
    }

    /// A checkout session for a Pro subscription, the only plan a self-serve checkout
    /// sells.
    pub async fn new_checkout_session(
        &self,
        organization: OrganizationUuid,
        customer: &JsonCustomer,
        price_name: &str,
        return_url: &str,
    ) -> Result<JsonCheckout, BillingError> {
        let customer = self.get_or_create_customer(customer).await?;

        let pro_price = self.products.pro.price(price_name)?;
        let bare_metal_price = self.products.bare_metal.price(price_name)?;
        let trial_period_days = self.trial_period_days(&customer).await?;

        // Pro's tiered active-series price carries the base fee, so there is no
        // separate base line item.
        let line_items = [pro_price, bare_metal_price]
            .into_iter()
            .map(|price| CreateCheckoutSessionLineItems {
                price: Some(price.id.to_string()),
                ..Default::default()
            })
            .collect::<Vec<_>>();

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

    /// Attach the subscription that `plan` names to `organization`: a checkout return must
    /// pass `check_checkout`, a subscription that staff made must pass `check_attach`, and one
    /// for a Self-Hosted deal must pass `check_self_hosted_attach`, and gets its first license.
    /// The plan is read and the license signed before anything is written, so a subscription
    /// that does not read as a plan is never named for the organization. When staff left out
    /// the subscription's `organization` metadata, it is written to name `organization`.
    pub async fn attach(
        &self,
        plan: &JsonNewPlan,
        organization: OrganizationUuid,
        licensor: &Licensor,
    ) -> Result<Attachment, AttachError> {
        match plan {
            JsonNewPlan::Checkout(JsonPlanCheckout { checkout }) => {
                let subscription = self
                    .get_checkout_subscription(checkout.as_ref())
                    .await
                    .map_err(read_error(checkout.as_ref()))?;
                let Attachable { named, .. } = check_checkout(
                    &self.products,
                    Self::map_status(&subscription.status),
                    &subscription.items.data,
                    &subscription.metadata,
                    organization,
                )?;
                self.attach_metered(&subscription, named, organization)
                    .await
            },
            JsonNewPlan::Subscription(JsonPlanSubscription {
                subscription,
                self_hosted,
            }) => {
                let id = subscription.as_ref();
                let subscription = self
                    .get_plan_subscription(&id.into())
                    .await
                    .map_err(read_error(id))?;
                if let Some(self_hosted) = self_hosted {
                    return self
                        .attach_licensed(&subscription, organization, *self_hosted, licensor)
                        .await;
                }
                let Attachable { named, .. } = check_attach(
                    &self.products,
                    Self::map_status(&subscription.status),
                    &subscription.items.data,
                    &subscription.metadata,
                    organization,
                )?;
                self.attach_metered(&subscription, named, organization)
                    .await
            },
        }
    }

    /// Attach `subscription`, for a Self-Hosted deal, to `organization`, the Bencher Cloud
    /// organization that buys it, and sign its first license for the Self-Hosted organization
    /// `self_hosted`.
    async fn attach_licensed(
        &self,
        subscription: &Subscription,
        organization: OrganizationUuid,
        self_hosted: OrganizationUuid,
        licensor: &Licensor,
    ) -> Result<Attachment, AttachError> {
        let SelfHostedAttachable { named, series } = check_self_hosted_attach(
            &self.products,
            Self::map_status(&subscription.status),
            &subscription.items.data,
            &subscription.metadata,
            organization,
        )?;
        let mut json_plan = json_plan(&self.products, subscription, organization)?;
        let term = LicenseTerm::new(
            series,
            json_plan.current_period_start,
            json_plan.current_period_end,
        )
        .map_err(AttachRefusal::Term)?;
        let license = term.sign(licensor, self_hosted)?;
        json_plan.license = Some(licensor.into_json(license.clone())?);
        self.name(subscription, named, organization).await?;
        let licensed_plan_id = subscription
            .id
            .as_ref()
            .parse()
            .map_err(BillingError::from)?;
        Ok(Attachment::Licensed(licensed_plan_id, license, json_plan))
    }

    /// Attach `subscription` as a Bencher Cloud plan for `organization`.
    async fn attach_metered(
        &self,
        subscription: &Subscription,
        named: bool,
        organization: OrganizationUuid,
    ) -> Result<Attachment, AttachError> {
        let json_plan = json_plan(&self.products, subscription, organization)?;
        self.name(subscription, named, organization).await?;
        let metered_plan_id = subscription
            .id
            .as_ref()
            .parse()
            .map_err(BillingError::from)?;
        Ok(Attachment::Metered(metered_plan_id, json_plan))
    }

    /// Write the `organization` metadata that staff left out of `subscription`, to name
    /// `organization`. Nothing is written when it already names it.
    async fn name(
        &self,
        subscription: &Subscription,
        named: bool,
        organization: OrganizationUuid,
    ) -> Result<(), BillingError> {
        if !named {
            UpdateSubscription::new(subscription.id.clone())
                .metadata(HashMap::from([(
                    METADATA_ORGANIZATION.to_owned(),
                    organization.to_string(),
                )]))
                .send(&self.client)
                .await?;
        }
        Ok(())
    }

    /// The subscription that a completed checkout session created, read as a plan.
    async fn get_checkout_subscription(
        &self,
        session_id: &str,
    ) -> Result<Subscription, BillingError> {
        let session_id: CheckoutSessionId = session_id.into();
        let checkout_session = RetrieveCheckoutSession::new(session_id)
            .send(&self.client)
            .await?;
        let Some(subscription_id) = checkout_session
            .subscription
            .as_ref()
            .map(|subscription| subscription.id().clone())
        else {
            return Err(BillingError::NoSubscription(Box::new(checkout_session)));
        };
        self.get_plan_subscription(&subscription_id).await
    }

    /// A billing portal session for the customer who pays for `plan_id`, which leads back to `return_url`.
    pub async fn new_portal_session(
        &self,
        plan_id: PlanId,
        return_url: &str,
    ) -> Result<String, BillingError> {
        let subscription = self.get_subscription(&plan_id.into()).await?;
        let session = CreateBillingPortalSession::new()
            .customer(subscription.customer.id().to_string())
            .return_url(return_url)
            .send(&self.client)
            .await?;
        Ok(session.url)
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

    /// Create a Pro subscription.
    // WARNING: Use caution when calling this directly as multiple subscriptions can be created
    pub async fn create_metered_subscription(
        &self,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        price_name: String,
    ) -> Result<Subscription, BillingError> {
        let pro_price = self.products.pro.price(&price_name)?;
        let bare_metal_price = self.products.bare_metal.price(&price_name)?;
        let trial_period_days = self.trial_period_days(&customer_id).await?;

        // Pro's tiered active-series price carries the base fee, so there is no
        // separate base item.
        let items = [pro_price, bare_metal_price]
            .into_iter()
            .map(|price| {
                let mut item = CreateSubscriptionItems::new();
                item.price = Some(price.id.to_string());
                item
            })
            .collect::<Vec<_>>();

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

    /// The native free-trial length for a new Pro subscription, granted only if the
    /// customer has no prior subscription. This makes the trial once-per-customer:
    /// canceling and re-subscribing (the plan POST prunes a terminally lapsed plan row)
    /// cannot restart it. A Stripe error propagates and fails plan creation rather than
    /// silently granting or denying the trial.
    async fn trial_period_days(
        &self,
        customer_id: &CustomerId,
    ) -> Result<Option<u32>, BillingError> {
        Ok((!self.customer_has_prior_subscription(customer_id).await?)
            .then_some(PRO_TRIAL_PERIOD_DAYS))
    }

    /// Whether the customer has ever had a subscription, in any status (including
    /// canceled and incomplete-expired). One existence check is enough, so a single
    /// result is requested.
    async fn customer_has_prior_subscription(
        &self,
        customer_id: &CustomerId,
    ) -> Result<bool, BillingError> {
        let subscriptions = ListSubscription::new()
            .customer(customer_id.to_string())
            .status(ListSubscriptionStatus::All)
            .limit(1)
            .send(&self.client)
            .await?;
        Ok(!subscriptions.data.is_empty())
    }

    pub async fn get_metered_plan(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<JsonPlan, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        self.get_plan(&subscription_id).await
    }

    /// A licensed plan, and the term of the license its subscription pays for now.
    pub async fn get_licensed_plan(
        &self,
        licensed_plan_id: &LicensedPlanId,
    ) -> Result<LicensedPlan, BillingError> {
        let subscription_id: SubscriptionId = licensed_plan_id.as_ref().into();
        let subscription = self.get_plan_subscription(&subscription_id).await?;
        let plan = self.plan(&subscription)?;
        let term = series(&subscription.metadata).and_then(|series| {
            LicenseTerm::new(series, plan.current_period_start, plan.current_period_end)
        });
        Ok(LicensedPlan { plan, term })
    }

    async fn get_plan(&self, subscription_id: &SubscriptionId) -> Result<JsonPlan, BillingError> {
        let subscription = self.get_plan_subscription(subscription_id).await?;
        self.plan(&subscription)
    }

    /// The plan that `subscription`, read with [`PLAN_EXPAND`], bills the organization its
    /// `organization` metadata names for.
    fn plan(&self, subscription: &Subscription) -> Result<JsonPlan, BillingError> {
        let Some(organization) = subscription.metadata.get(METADATA_ORGANIZATION) else {
            return Err(BillingError::NoOrganization(subscription.id.clone()));
        };
        let organization = organization
            .parse()
            .map_err(|e| BillingError::BadOrganizationUuid(organization.clone(), e))?;
        json_plan(&self.products, subscription, organization)
    }

    /// The subscription with everything a plan read needs, in one request.
    async fn get_plan_subscription(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<Subscription, BillingError> {
        self.get_subscription_expand(subscription_id, PLAN_EXPAND.map(str::to_owned).into())
            .await
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

    /// One-fetch billing snapshot: status, the Stripe customer to post usage for, and the
    /// current period bounds. Expands the subscription's items because the period bounds
    /// live on them (all items share one period). Used on the report path to gate access
    /// and to supply the active-series count window.
    pub async fn get_metered_plan_billing(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<MeteredPlanBilling, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        let subscription = self
            .get_subscription_expand(&subscription_id, vec!["items".into()])
            .await?;
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
            status: Self::map_status(&subscription.status),
            customer_id: subscription.customer.id().clone(),
            current_period_start,
            current_period_end,
        })
    }

    /// Resolve a metered subscription's live status for the two decisions keyed on whether
    /// it is still live in Stripe (re-subscription gating on plan creation, and
    /// cancel-vs-skip on plan deletion): `Some(status)` when the subscription exists, and
    /// `None` when Stripe reports it no longer exists (a definitive 404, safe to treat as
    /// gone). Returns an error only for indeterminate failures (network, 5xx, rate limit),
    /// so a transient outage is never mistaken for "gone" (which would risk orphaning a
    /// still-live subscription: pruned on creation, or left uncanceled on deletion). The
    /// caller distinguishes terminal from recoverable (dunning) statuses.
    pub async fn metered_plan_status(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<Option<PlanStatus>, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        self.subscription_status(&subscription_id).await
    }

    /// The live status of a licensed subscription, or `None` when Stripe reports it gone
    /// (404). Mirrors [`Self::metered_plan_status`] so callers can tell a still-live
    /// subscription from one that is already gone or terminal.
    pub async fn licensed_plan_status(
        &self,
        licensed_plan_id: &LicensedPlanId,
    ) -> Result<Option<PlanStatus>, BillingError> {
        let subscription_id: SubscriptionId = licensed_plan_id.as_ref().into();
        self.subscription_status(&subscription_id).await
    }

    /// The live status of a subscription, or `None` when Stripe reports it gone (404). A
    /// transient Stripe error is returned as-is so callers can distinguish it from "gone".
    async fn subscription_status(
        &self,
        subscription_id: &SubscriptionId,
    ) -> Result<Option<PlanStatus>, BillingError> {
        match self.get_subscription(subscription_id).await {
            Ok(subscription) => Ok(Some(Self::map_status(&subscription.status))),
            Err(BillingError::Stripe(stripe::StripeError::Stripe(_, 404))) => Ok(None),
            Err(e) => Err(e),
        }
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

    /// Post an organization's cumulative period-to-date active-series count to the
    /// `active_series` meter (which backs every metered plan's price), stamped with
    /// `when` (the time the count was taken). Billed with `last` aggregation: the
    /// explicit timestamp orders concurrent posts by `when` (to one-second granularity)
    /// rather than by Stripe's receipt order, so reports more than a second apart bill the
    /// later count deterministically; a same-second tie self-heals on the next report.
    /// Posting after each report makes the final post of the period the period total, and
    /// a missed post self-heals on the next report.
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
    /// `sum`-aggregation runner minutes meter passes `None` and uses Stripe's receipt
    /// time.
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
    /// path; the organization downgrades to Free right away. `invoice_now`
    /// generates a final invoice for any un-invoiced metered usage; a bare
    /// cancel would silently discard all usage accrued since the last invoice.
    pub async fn cancel_metered_subscription(
        &self,
        metered_plan_id: &MeteredPlanId,
    ) -> Result<Subscription, BillingError> {
        let subscription_id: SubscriptionId = metered_plan_id.as_ref().into();
        CancelSubscription::new(subscription_id)
            .invoice_now(true)
            .send(&self.client)
            .await
            .map_err(Into::into)
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

/// A subscription's plan item: its one item on the Pro or Enterprise product, with the
/// plan level that product names. No other item, the bare metal item among them, is the
/// plan item.
fn plan_item<'a>(
    products: &Products,
    items: &'a [SubscriptionItem],
) -> Result<(PlanLevel, &'a SubscriptionItem), PlanItemError> {
    let mut plan_items = items.iter().filter_map(|item| {
        products
            .plan_level(item.price.product.id())
            .map(|level| (level, item))
    });
    match (plan_items.next(), plan_items.next()) {
        (Some(plan_item), None) => Ok(plan_item),
        (Some(_), Some(_)) => Err(PlanItemError::Multiple),
        (None, _) => Err(PlanItemError::Missing),
    }
}

/// The plan that `subscription`, read with [`PLAN_EXPAND`], bills `organization` for.
fn json_plan(
    products: &Products,
    subscription: &Subscription,
    organization: OrganizationUuid,
) -> Result<JsonPlan, BillingError> {
    let Subscription {
        id,
        items,
        created,
        customer,
        default_payment_method,
        collection_method,
        status,
        cancel_at_period_end,
        ..
    } = subscription;
    let (level, item) =
        plan_item(products, &items.data).map_err(|e| BillingError::PlanItem(id.clone(), e))?;
    let PlanPrice {
        unit_amount,
        tiers,
        tiers_mode,
    } = plan_price(&item.price)?;
    let runner_minute = items
        .data
        .iter()
        .find(|item| products.is_bare_metal(item.price.product.id()))
        .and_then(|item| runner_minute(&item.price));
    let collection_method = plan_collection_method(collection_method);
    // Stripe never charges a payment method for a plan that it bills by invoice.
    let card = match collection_method {
        CollectionMethod::ChargeAutomatically => {
            plan_card(default_payment_method.as_ref(), customer)?
        },
        CollectionMethod::SendInvoice => None,
    };
    let date = |timestamp: i64| {
        DateTime::try_from(timestamp).map_err(|e| BillingError::DateTime(id.clone(), timestamp, e))
    };
    Ok(JsonPlan {
        organization,
        customer: plan_customer(customer)?,
        card,
        collection_method,
        level,
        unit_amount: unit_amount.into(),
        tiers,
        tiers_mode,
        runner_minute,
        created: date(*created)?,
        current_period_start: date(item.current_period_start)?,
        current_period_end: date(item.current_period_end)?,
        status: Biller::map_status(status),
        cancel_at_period_end: *cancel_at_period_end,
        license: None,
    })
}

/// How Stripe collects each payment. A method that this Bencher does not know reads as a
/// charge, the default that Stripe gives.
fn plan_collection_method(collection_method: &SubscriptionCollectionMethod) -> CollectionMethod {
    match collection_method {
        SubscriptionCollectionMethod::SendInvoice => CollectionMethod::SendInvoice,
        SubscriptionCollectionMethod::ChargeAutomatically
        | SubscriptionCollectionMethod::Unknown(_)
        | _ => CollectionMethod::ChargeAutomatically,
    }
}

/// The Bencher user who pays: `None` for a customer that staff made in Stripe, which has
/// no `uuid` metadata. A customer with that metadata but data Bencher cannot read is an
/// error, since Bencher made it.
fn plan_customer(customer: &Expandable<Customer>) -> Result<Option<JsonCustomer>, BillingError> {
    let Some(customer) = customer.as_object() else {
        return Err(BillingError::NoCustomerInfo(customer.id().clone()));
    };
    let Some(uuid) = customer
        .metadata
        .as_ref()
        .and_then(|meta| meta.get(METADATA_UUID))
    else {
        return Ok(None);
    };
    let Some(name) = &customer.name else {
        return Err(BillingError::NoName(customer.id.clone()));
    };
    let Some(email) = &customer.email else {
        return Err(BillingError::NoEmail(customer.id.clone()));
    };
    Ok(Some(JsonCustomer {
        uuid: uuid
            .parse()
            .map_err(|e| BillingError::BadUserUuid(uuid.clone(), e))?,
        name: name.parse()?,
        email: email.parse()?,
    }))
}

/// The card that Stripe charges for a plan it charges automatically: the subscription's
/// own default payment method, or else the customer's default for invoices. `None` when
/// neither is set, or the method that pays is not a card.
fn plan_card(
    subscription_method: Option<&Expandable<PaymentMethod>>,
    customer: &Expandable<Customer>,
) -> Result<Option<JsonCardDetails>, BillingError> {
    let customer_method = customer
        .as_object()
        .and_then(|customer| customer.invoice_settings.as_ref())
        .and_then(|settings| settings.default_payment_method.as_ref());
    let Some(method) = subscription_method.or(customer_method) else {
        return Ok(None);
    };
    let Some(method_info) = method.as_object() else {
        return Err(BillingError::NoDefaultPaymentMethodInfo(
            method.id().clone(),
        ));
    };
    let Some(card) = &method_info.card else {
        return Ok(None);
    };
    Ok(Some(JsonCardDetails {
        brand: card.brand.parse()?,
        last_four: card.last4.parse()?,
        exp_month: card.exp_month.try_into()?,
        exp_year: card.exp_year.try_into()?,
    }))
}

/// The plan's price as its own Stripe price sets it: the price's flat `unit_amount`, or
/// for a tiered price the first tier's flat fee, and its tier ladder and mode. A per-deal
/// price is in no configuration, so nothing here reads one.
fn plan_price(price: &Price) -> Result<PlanPrice, BillingError> {
    let tiers = price.tiers.as_deref();
    let cents = price
        .unit_amount
        .or_else(|| tiers?.first()?.flat_amount)
        .ok_or_else(|| BillingError::NoUnitAmount(price.id.clone()))?;
    Ok(PlanPrice {
        unit_amount: u64::try_from(cents)?,
        tiers: tiers.map(|tiers| tiers.iter().map(to_json_tier).collect()),
        tiers_mode: price.tiers_mode.as_ref().and_then(|mode| match mode {
            PriceTiersMode::Graduated => Some(TiersMode::Graduated),
            PriceTiersMode::Volume => Some(TiersMode::Volume),
            PriceTiersMode::Unknown(_) | _ => None,
        }),
    })
}

/// What a plan's own price shows.
#[derive(Debug)]
struct PlanPrice {
    unit_amount: u64,
    tiers: Option<Vec<JsonPriceTier>>,
    tiers_mode: Option<TiersMode>,
}

/// The bare metal price per runner minute, in cents: the price's per unit amount, over the
/// minutes that its quantity transform divides by. `None` for a price without a per unit
/// amount.
fn runner_minute(price: &Price) -> Option<f64> {
    let cents = match price.unit_amount_decimal.as_deref() {
        Some(decimal) => decimal.parse::<f64>().ok()?,
        None => f64::from(i32::try_from(price.unit_amount?).ok()?),
    };
    let minutes = match &price.transform_quantity {
        Some(transform) => u32::try_from(transform.divide_by)
            .ok()
            .filter(|minutes| *minutes > 0)?,
        None => 1,
    };
    Some(cents / f64::from(minutes))
}

/// An attach error for a Stripe read of `id`: not found when Stripe has no such object.
fn read_error(id: &str) -> impl FnOnce(BillingError) -> AttachError + '_ {
    move |error| {
        if let BillingError::Stripe(stripe::StripeError::Stripe(_, 404)) = &error {
            AttachError::NotFound(id.to_owned())
        } else {
            AttachError::Billing(error)
        }
    }
}

/// A subscription that may be attached to an organization.
#[derive(Debug, PartialEq, Eq)]
struct Attachable {
    level: PlanLevel,
    /// Whether the subscription's `organization` metadata already names the organization.
    named: bool,
}

/// Whether a subscription may be attached to `organization`: it still exists in Stripe
/// (not canceled or expired), it has one plan item with a price the plan can show, it has
/// the bare metal item that every Bencher Cloud plan bills runner minutes on, and no
/// `organization` metadata names another organization. Missing metadata is accepted, for
/// the attach to write.
fn check_attach(
    products: &Products,
    status: PlanStatus,
    items: &[SubscriptionItem],
    metadata: &HashMap<String, String>,
    organization: OrganizationUuid,
) -> Result<Attachable, AttachRefusal> {
    let level = live_plan_level(products, status, items)?;
    if !items
        .iter()
        .any(|item| products.is_bare_metal(item.price.product.id()))
    {
        return Err(AttachRefusal::NoBareMetal);
    }
    let named = named(metadata, organization)?;
    Ok(Attachable { level, named })
}

/// The plan level of a subscription that still exists in Stripe (not canceled or expired), from
/// its one plan item, whose price the plan can show.
fn live_plan_level(
    products: &Products,
    status: PlanStatus,
    items: &[SubscriptionItem],
) -> Result<PlanLevel, AttachRefusal> {
    if matches!(status, PlanStatus::Canceled | PlanStatus::IncompleteExpired) {
        return Err(AttachRefusal::Terminal(status));
    }
    let (level, item) = plan_item(products, items).map_err(AttachRefusal::PlanItem)?;
    if plan_price(&item.price).is_err() {
        return Err(AttachRefusal::Price(item.price.id.to_string()));
    }
    Ok(level)
}

/// Whether a subscription's `organization` metadata already names `organization`: refused when
/// it names another, and `false` when it names none, for the attach to write.
fn named(
    metadata: &HashMap<String, String>,
    organization: OrganizationUuid,
) -> Result<bool, AttachRefusal> {
    let Some(named) = metadata.get(METADATA_ORGANIZATION) else {
        return Ok(false);
    };
    if named.parse::<OrganizationUuid>().ok() == Some(organization) {
        Ok(true)
    } else {
        Err(AttachRefusal::OtherOrganization(named.clone()))
    }
}

/// A subscription for a Self-Hosted deal that may be attached to an organization.
#[derive(Debug, PartialEq, Eq)]
struct SelfHostedAttachable {
    /// Whether the subscription's `organization` metadata already names the organization.
    named: bool,
    /// The Benchmark Series in the subscription's `series` metadata.
    series: Entitlements,
}

/// Whether a subscription for a Self-Hosted deal may be attached to `organization`, the Bencher
/// Cloud organization that buys it: it is active, since nothing can take back a license once it
/// is signed, its one plan item is on the Enterprise product with a price the plan can show, its
/// `series` metadata is a count of Benchmark Series, and no `organization` metadata names another
/// organization. It needs no bare metal item, since the Self-Hosted server runs its own jobs.
fn check_self_hosted_attach(
    products: &Products,
    status: PlanStatus,
    items: &[SubscriptionItem],
    metadata: &HashMap<String, String>,
    organization: OrganizationUuid,
) -> Result<SelfHostedAttachable, AttachRefusal> {
    let level = live_plan_level(products, status, items)?;
    if !status.is_active() {
        return Err(AttachRefusal::NotActive(status));
    }
    if level != PlanLevel::Enterprise {
        return Err(AttachRefusal::NotEnterprise(level));
    }
    let series = series(metadata).map_err(AttachRefusal::Term)?;
    let named = named(metadata, organization)?;
    Ok(SelfHostedAttachable { named, series })
}

/// The Benchmark Series in a subscription's `series` metadata.
fn series(metadata: &HashMap<String, String>) -> Result<Entitlements, LicenseTermError> {
    let series = metadata
        .get(METADATA_SERIES)
        .ok_or(LicenseTermError::NoSeries)?;
    series
        .parse()
        .ok()
        .ok_or_else(|| LicenseTermError::BadSeries(series.clone()))
}

/// The term to sign a new license for, in place of the license `held`, for a subscription in
/// `status`, whose plan item is on the product of `level`, that pays for `term` now: none unless
/// the subscription is active and a license for `term` replaces `held`, and an error when the
/// subscription is active but no longer pays for an Enterprise license that Bencher Cloud can
/// sign. A license never moves back a term.
fn renewal(
    status: PlanStatus,
    level: PlanLevel,
    term: &Result<LicenseTerm, LicenseTermError>,
    held: HeldLicense<'_>,
) -> Result<Option<LicenseTerm>, RenewalError> {
    if !status.is_active() {
        return Ok(None);
    }
    if level != PlanLevel::Enterprise {
        return Err(RenewalError::NotEnterprise(level));
    }
    let term = term.clone().map_err(RenewalError::Term)?;
    let replaces = match held {
        HeldLicense::Verified(held) => held.replaced_by(term.series, term.term_end),
        HeldLicense::Unverified(held) => held.trm <= term.term_end.timestamp(),
    };
    Ok(replaces.then_some(term))
}

/// Whether the subscription that a self-serve checkout created may be attached to
/// `organization`: it passes [`check_attach`], its plan item is on the Pro product, and
/// its `organization` metadata, which the checkout writes, names `organization`.
fn check_checkout(
    products: &Products,
    status: PlanStatus,
    items: &[SubscriptionItem],
    metadata: &HashMap<String, String>,
    organization: OrganizationUuid,
) -> Result<Attachable, AttachRefusal> {
    let attachable = check_attach(products, status, items, metadata, organization)?;
    if attachable.level != PlanLevel::Pro {
        return Err(AttachRefusal::NotPro(attachable.level));
    }
    if !attachable.named {
        return Err(AttachRefusal::NoOrganization);
    }
    Ok(attachable)
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
    use std::collections::HashMap;

    use bencher_json::{
        DateTime, Entitlements, MeteredPlanId, OrganizationUuid, PlanLevel, PlanStatus, Secret,
        UserUuid,
        organization::plan::{
            ACTIVE_SERIES_METER_NAME, CollectionMethod, DEFAULT_PRICE_NAME, JsonNewPlan, JsonPlan,
            JsonPlanCheckout, JsonPlanSubscription, RUNNER_MINUTES_METER_NAME, TiersMode,
        },
        system::{
            config::{JsonBilling, JsonEnterpriseProduct, JsonProduct, JsonProducts},
            payment::{JsonCard, JsonCustomer},
        },
    };
    use chrono::{Datelike as _, Utc};
    use literally::hmap;
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};
    use stripe_billing::subscription::{CreateSubscription, CreateSubscriptionItems};
    use stripe_core::customer::CreateCustomer;
    use stripe_product::price::{
        CreatePrice, CreatePriceRecurring, CreatePriceRecurringInterval,
        CreatePriceRecurringUsageType, CreatePriceTiers, CreatePriceTiersUpTo, ListPrice,
    };
    use stripe_shared::{CustomerId, PaymentMethodId, PriceTier};
    use stripe_types::{Currency, Expandable};

    use rustls::crypto::aws_lc_rs::default_provider;

    use bencher_license::{Audience, GRACE_PERIOD, LicenseClaims, Licensor};

    use crate::{
        AttachError, AttachRefusal, Attachment, Biller, HeldLicense, LicenseTerm, LicenseTermError,
        PlanItemError, RenewalError,
        stripe_stub::{
            self as stub, BARE_METAL_PRODUCT, DAY, ENTERPRISE_PRODUCT, NOW, PRO_PRODUCT,
            StripeStub, StubObjects,
        },
    };

    use super::{
        Attachable, METADATA_ORGANIZATION, METADATA_SERIES, METER_CUSTOMER_KEY, METER_VALUE_KEY,
        PlanPrice, SelfHostedAttachable, check_attach, check_checkout, check_self_hosted_attach,
        plan_item, plan_price, renewal, runner_minute, series,
    };

    /// A licensor that signs as Bencher Cloud does, with the key that debug builds trust.
    fn licensor() -> Licensor {
        let key: Secret = include_str!("../../bencher_license/src/test/private.pem")
            .parse()
            .unwrap();
        Licensor::bencher_cloud(&key).unwrap()
    }

    fn named(organization: OrganizationUuid) -> HashMap<String, String> {
        HashMap::from([(METADATA_ORGANIZATION.to_owned(), organization.to_string())])
    }

    #[test]
    fn map_status_maps_subscription_status() {
        use stripe_billing::SubscriptionStatus;

        // Each Stripe subscription status maps to the matching plan status. This mapping
        // feeds `metered_plan_status`/`licensed_plan_status`, which drive the
        // cancel-vs-skip decision in `subscription_is_live`, so a wrong mapping (e.g. a
        // terminal status reading as `Active`) would cancel or skip incorrectly.
        assert_eq!(
            Biller::map_status(&SubscriptionStatus::Active),
            PlanStatus::Active,
        );
        assert_eq!(
            Biller::map_status(&SubscriptionStatus::Canceled),
            PlanStatus::Canceled,
        );
        assert_eq!(
            Biller::map_status(&SubscriptionStatus::Incomplete),
            PlanStatus::Incomplete,
        );
        assert_eq!(
            Biller::map_status(&SubscriptionStatus::IncompleteExpired),
            PlanStatus::IncompleteExpired,
        );
        assert_eq!(
            Biller::map_status(&SubscriptionStatus::PastDue),
            PlanStatus::PastDue,
        );
        assert_eq!(
            Biller::map_status(&SubscriptionStatus::Paused),
            PlanStatus::Paused,
        );
        assert_eq!(
            Biller::map_status(&SubscriptionStatus::Trialing),
            PlanStatus::Trialing,
        );
        assert_eq!(
            Biller::map_status(&SubscriptionStatus::Unpaid),
            PlanStatus::Unpaid,
        );
        // An unrecognized/future Stripe status falls through the wildcard arm to `Unpaid`.
        // That is the safe side for deletion: `Unpaid` reads as live, so a cancel is
        // attempted rather than silently skipped and left to orphan.
        assert_eq!(
            Biller::map_status(&SubscriptionStatus::Unknown("future_status".to_owned())),
            PlanStatus::Unpaid,
        );
    }

    // Fails if the plan item is picked by anything but its product: a Pro or Enterprise
    // price outside the configuration, the bare metal item, or an item on a retired
    // product taken for it.
    #[test]
    fn plan_item_follows_the_product() {
        let products = stub::products();
        let bare_metal = make_subscription_item("price_bare_metal", BARE_METAL_PRODUCT);

        let pro = [
            bare_metal.clone(),
            make_subscription_item("price_pro_unlisted", PRO_PRODUCT),
        ];
        let (level, item) = plan_item(&products, &pro).unwrap();
        assert_eq!(level, PlanLevel::Pro);
        assert_eq!(item.price.id.as_str(), "price_pro_unlisted");

        let enterprise = [
            make_subscription_item("price_deal", ENTERPRISE_PRODUCT),
            bare_metal.clone(),
        ];
        let (level, item) = plan_item(&products, &enterprise).unwrap();
        assert_eq!(level, PlanLevel::Enterprise);
        assert_eq!(item.price.id.as_str(), "price_deal");

        assert_eq!(
            plan_item(&products, std::slice::from_ref(&bare_metal)).unwrap_err(),
            PlanItemError::Missing
        );
        let team = [
            make_subscription_item("price_team", "prod_team"),
            bare_metal.clone(),
        ];
        assert_eq!(
            plan_item(&products, &team).unwrap_err(),
            PlanItemError::Missing
        );
        let both = [
            make_subscription_item("price_pro", PRO_PRODUCT),
            make_subscription_item("price_deal", ENTERPRISE_PRODUCT),
            bare_metal,
        ];
        assert_eq!(
            plan_item(&products, &both).unwrap_err(),
            PlanItemError::Multiple
        );
    }

    // Fails if a plan's amount, tiers, or tiers mode come from anywhere but the plan item's
    // own price, which for a per-deal Enterprise price is in no configuration.
    #[test]
    fn plan_price_comes_from_the_subscription() {
        let products = stub::products();
        let mut deal = make_subscription_item("price_deal", ENTERPRISE_PRODUCT);
        deal.price.unit_amount = None;
        deal.price.billing_scheme = stripe_shared::PriceBillingScheme::Tiered;
        deal.price.tiers_mode = Some(stripe_shared::PriceTiersMode::Graduated);
        deal.price.tiers = Some(vec![
            price_tier(Some(1_000), None, Some(250_000)),
            price_tier(None, Some(40), None),
        ]);
        let items = [
            make_subscription_item("price_bare_metal", BARE_METAL_PRODUCT),
            deal,
        ];
        let (level, item) = plan_item(&products, &items).unwrap();
        assert_eq!(level, PlanLevel::Enterprise);

        let PlanPrice {
            unit_amount,
            tiers,
            tiers_mode,
        } = plan_price(&item.price).unwrap();
        assert_eq!(unit_amount, 250_000);
        assert_eq!(tiers_mode, Some(TiersMode::Graduated));
        let tiers: Vec<_> = tiers
            .unwrap()
            .into_iter()
            .map(|tier| {
                (
                    tier.up_to,
                    tier.unit_amount.map(u64::from),
                    tier.flat_amount.map(u64::from),
                )
            })
            .collect();
        assert_eq!(
            tiers,
            [(Some(1_000), None, Some(250_000)), (None, Some(40), None)]
        );

        let mut volume = make_price("price_pro", PRO_PRODUCT);
        volume.unit_amount = None;
        volume.tiers_mode = Some(stripe_shared::PriceTiersMode::Volume);
        volume.tiers = Some(vec![price_tier(Some(250), None, Some(10_000))]);
        assert_eq!(
            plan_price(&volume).unwrap().tiers_mode,
            Some(TiersMode::Volume)
        );

        let mut flat = make_price("price_flat_deal", ENTERPRISE_PRODUCT);
        flat.unit_amount = Some(1_234);
        let PlanPrice {
            unit_amount,
            tiers,
            tiers_mode,
        } = plan_price(&flat).unwrap();
        assert_eq!(unit_amount, 1_234);
        assert!(tiers.is_none());
        assert!(tiers_mode.is_none());

        let mut unpriced = make_price("price_unpriced", ENTERPRISE_PRODUCT);
        unpriced.unit_amount = None;
        assert!(matches!(
            plan_price(&unpriced).unwrap_err(),
            crate::BillingError::NoUnitAmount(_)
        ));
    }

    // Fails if the bare metal rate is not read from the price's own decimal amount and
    // quantity transform, or is made up for a price without a per unit amount.
    #[test]
    fn runner_minute_reads_the_bare_metal_price() {
        let mut per_minute = make_price("price_bare_metal", BARE_METAL_PRODUCT);
        per_minute.unit_amount = None;
        per_minute.unit_amount_decimal = Some("1.6666666667".to_owned());
        let rate = runner_minute(&per_minute).unwrap();
        assert!((rate - 1.666_666_666_7).abs() < 1e-9, "{rate}");

        let mut per_hour = make_price("price_bare_metal_hour", BARE_METAL_PRODUCT);
        per_hour.unit_amount = Some(200);
        per_hour.unit_amount_decimal = Some("200".to_owned());
        per_hour.transform_quantity = Some(stripe_shared::TransformQuantity {
            divide_by: 60,
            round: stripe_shared::TransformQuantityRound::Up,
        });
        let rate = runner_minute(&per_hour).unwrap();
        assert!((rate - 200.0 / 60.0).abs() < 1e-9, "{rate}");

        let mut tiered = make_price("price_bare_metal_tiered", BARE_METAL_PRODUCT);
        tiered.unit_amount = None;
        tiered.billing_scheme = stripe_shared::PriceBillingScheme::Tiered;
        assert!(runner_minute(&tiered).is_none());
    }

    // Fails if an attach refuses a live subscription with one priced plan item and bare
    // metal, named for the organization or not named at all.
    #[test]
    fn check_attach_accepts() {
        let products = stub::products();
        let organization = OrganizationUuid::new();
        let active = PlanStatus::Active;
        let bare_metal = make_subscription_item("price_bare_metal", BARE_METAL_PRODUCT);
        let pro = [
            make_subscription_item("price_pro", PRO_PRODUCT),
            bare_metal.clone(),
        ];
        let enterprise = [
            make_subscription_item("price_deal", ENTERPRISE_PRODUCT),
            bare_metal,
        ];

        assert_eq!(
            check_attach(
                &products,
                active,
                &enterprise,
                &named(organization),
                organization
            ),
            Ok(Attachable {
                level: PlanLevel::Enterprise,
                named: true,
            })
        );
        assert_eq!(
            check_attach(&products, active, &pro, &named(organization), organization),
            Ok(Attachable {
                level: PlanLevel::Pro,
                named: true,
            })
        );
        assert_eq!(
            check_attach(
                &products,
                active,
                &enterprise,
                &HashMap::new(),
                organization
            ),
            Ok(Attachable {
                level: PlanLevel::Enterprise,
                named: false,
            })
        );
        // A dunning subscription still exists in Stripe, so it may be attached.
        assert_eq!(
            check_attach(
                &products,
                PlanStatus::PastDue,
                &enterprise,
                &named(organization),
                organization
            ),
            Ok(Attachable {
                level: PlanLevel::Enterprise,
                named: true,
            })
        );
    }

    // Fails if an attach takes a subscription in a terminal status, with no plan item, with
    // a price the plan cannot show, without bare metal, or one that names another
    // organization.
    #[test]
    fn check_attach_refuses() {
        let products = stub::products();
        let organization = OrganizationUuid::new();
        let active = PlanStatus::Active;
        let bare_metal = make_subscription_item("price_bare_metal", BARE_METAL_PRODUCT);
        let deal = make_subscription_item("price_deal", ENTERPRISE_PRODUCT);
        let pro = [
            make_subscription_item("price_pro", PRO_PRODUCT),
            bare_metal.clone(),
        ];
        let enterprise = [deal.clone(), bare_metal.clone()];

        for terminal in [PlanStatus::Canceled, PlanStatus::IncompleteExpired] {
            assert_eq!(
                check_attach(
                    &products,
                    terminal,
                    &enterprise,
                    &named(organization),
                    organization
                ),
                Err(AttachRefusal::Terminal(terminal))
            );
        }
        assert_eq!(
            check_attach(
                &products,
                active,
                std::slice::from_ref(&bare_metal),
                &named(organization),
                organization
            ),
            Err(AttachRefusal::PlanItem(PlanItemError::Missing))
        );
        let mut unpriced = deal.clone();
        unpriced.price.unit_amount = None;
        assert_eq!(
            check_attach(
                &products,
                active,
                &[unpriced, bare_metal.clone()],
                &HashMap::new(),
                organization
            ),
            Err(AttachRefusal::Price("price_deal".to_owned()))
        );
        let team = [
            make_subscription_item("price_team", "prod_team"),
            bare_metal,
        ];
        assert_eq!(
            check_attach(&products, active, &team, &named(organization), organization),
            Err(AttachRefusal::PlanItem(PlanItemError::Missing))
        );
        assert_eq!(
            check_attach(
                &products,
                active,
                std::slice::from_ref(&deal),
                &named(organization),
                organization
            ),
            Err(AttachRefusal::NoBareMetal)
        );
        assert_eq!(
            check_attach(
                &products,
                active,
                &pro[..1],
                &named(organization),
                organization
            ),
            Err(AttachRefusal::NoBareMetal)
        );
        let other = OrganizationUuid::new();
        assert_eq!(
            check_attach(&products, active, &enterprise, &named(other), organization),
            Err(AttachRefusal::OtherOrganization(other.to_string()))
        );
        let garbled = HashMap::from([(METADATA_ORGANIZATION.to_owned(), "acme".to_owned())]);
        assert_eq!(
            check_attach(&products, active, &enterprise, &garbled, organization),
            Err(AttachRefusal::OtherOrganization("acme".to_owned()))
        );
    }

    // Fails if a self-serve return records anything but a live Pro plan that names its
    // own organization.
    #[test]
    fn check_checkout_decides() {
        let products = stub::products();
        let organization = OrganizationUuid::new();
        let active = PlanStatus::Active;
        let bare_metal = make_subscription_item("price_bare_metal", BARE_METAL_PRODUCT);
        let pro = [
            make_subscription_item("price_pro", PRO_PRODUCT),
            bare_metal.clone(),
        ];
        let enterprise = [
            make_subscription_item("price_deal", ENTERPRISE_PRODUCT),
            bare_metal,
        ];

        assert_eq!(
            check_checkout(&products, active, &pro, &named(organization), organization),
            Ok(Attachable {
                level: PlanLevel::Pro,
                named: true,
            })
        );
        assert_eq!(
            check_checkout(
                &products,
                active,
                &enterprise,
                &named(organization),
                organization
            ),
            Err(AttachRefusal::NotPro(PlanLevel::Enterprise))
        );
        assert_eq!(
            check_checkout(&products, active, &pro, &HashMap::new(), organization),
            Err(AttachRefusal::NoOrganization)
        );
        let other = OrganizationUuid::new();
        assert_eq!(
            check_checkout(&products, active, &pro, &named(other), organization),
            Err(AttachRefusal::OtherOrganization(other.to_string()))
        );
        assert_eq!(
            check_checkout(
                &products,
                PlanStatus::Canceled,
                &pro,
                &named(organization),
                organization
            ),
            Err(AttachRefusal::Terminal(PlanStatus::Canceled))
        );
    }

    fn series_named(
        organization: Option<OrganizationUuid>,
        series: &str,
    ) -> HashMap<String, String> {
        let mut metadata = HashMap::from([(METADATA_SERIES.to_owned(), series.to_owned())]);
        if let Some(organization) = organization {
            metadata.insert(METADATA_ORGANIZATION.to_owned(), organization.to_string());
        }
        metadata
    }

    fn entitlements(series: u32) -> Entitlements {
        Entitlements::try_from(series).unwrap()
    }

    // Fails if an active or trialing subscription for a Self-Hosted deal is refused when its
    // plan item is on the Enterprise product with a price the plan can show and it carries a
    // count of Benchmark Series, named for the organization or not, with or without a bare metal
    // item.
    #[test]
    fn check_self_hosted_attach_accepts() {
        let products = stub::products();
        let organization = OrganizationUuid::new();
        let deal = make_subscription_item("price_self_hosted", ENTERPRISE_PRODUCT);
        let bare_metal = make_subscription_item("price_bare_metal", BARE_METAL_PRODUCT);
        let accepted = |status, items: &[_], metadata: &HashMap<_, _>| {
            check_self_hosted_attach(&products, status, items, metadata, organization)
        };

        assert_eq!(
            accepted(
                PlanStatus::Active,
                std::slice::from_ref(&deal),
                &series_named(Some(organization), "500")
            ),
            Ok(SelfHostedAttachable {
                named: true,
                series: entitlements(500),
            })
        );
        assert_eq!(
            accepted(
                PlanStatus::Active,
                &[deal.clone(), bare_metal],
                &series_named(None, "1")
            ),
            Ok(SelfHostedAttachable {
                named: false,
                series: entitlements(1),
            })
        );
        assert_eq!(
            accepted(
                PlanStatus::Trialing,
                std::slice::from_ref(&deal),
                &series_named(None, "4294967295")
            ),
            Ok(SelfHostedAttachable {
                named: false,
                series: Entitlements::MAX,
            })
        );
    }

    // Fails if a subscription for a Self-Hosted deal is attached in a terminal status or any
    // other status that is not active, without its plan item on the Enterprise product, with a
    // price the plan cannot show, without a count of Benchmark Series in its `series` metadata,
    // or named for another organization.
    #[test]
    fn check_self_hosted_attach_refuses() {
        let products = stub::products();
        let organization = OrganizationUuid::new();
        let active = PlanStatus::Active;
        let deal = make_subscription_item("price_self_hosted", ENTERPRISE_PRODUCT);
        let deal = std::slice::from_ref(&deal);
        let metadata = series_named(None, "500");
        let refused = |status, items: &[_], metadata: &HashMap<_, _>| {
            check_self_hosted_attach(&products, status, items, metadata, organization).unwrap_err()
        };

        for terminal in [PlanStatus::Canceled, PlanStatus::IncompleteExpired] {
            assert_eq!(
                refused(terminal, deal, &metadata),
                AttachRefusal::Terminal(terminal)
            );
        }
        // A license cannot be taken back, so it is never signed for a subscription that may
        // never be paid.
        for inactive in [
            PlanStatus::PastDue,
            PlanStatus::Unpaid,
            PlanStatus::Incomplete,
            PlanStatus::Paused,
        ] {
            assert_eq!(
                refused(inactive, deal, &metadata),
                AttachRefusal::NotActive(inactive)
            );
        }
        let pro = make_subscription_item("price_pro", PRO_PRODUCT);
        assert_eq!(
            refused(active, std::slice::from_ref(&pro), &metadata),
            AttachRefusal::NotEnterprise(PlanLevel::Pro)
        );
        let bare_metal = make_subscription_item("price_bare_metal", BARE_METAL_PRODUCT);
        assert_eq!(
            refused(active, std::slice::from_ref(&bare_metal), &metadata),
            AttachRefusal::PlanItem(PlanItemError::Missing)
        );
        let mut unpriced = deal[0].clone();
        unpriced.price.unit_amount = None;
        assert_eq!(
            refused(active, &[unpriced], &metadata),
            AttachRefusal::Price("price_self_hosted".to_owned())
        );
        assert_eq!(
            refused(active, deal, &HashMap::new()),
            AttachRefusal::Term(LicenseTermError::NoSeries)
        );
        for bad in ["", "0", "-5", "1.5", "500 series", "4294967296"] {
            assert_eq!(
                refused(active, deal, &series_named(None, bad)),
                AttachRefusal::Term(LicenseTermError::BadSeries(bad.to_owned())),
                "{bad}"
            );
        }
        let other = OrganizationUuid::new();
        assert_eq!(
            refused(active, deal, &series_named(Some(other), "500")),
            AttachRefusal::OtherOrganization(other.to_string())
        );
    }

    // Fails if a Bencher Cloud attach, which a Self-Hosted deal's check leaves alone, starts to
    // need `series` metadata or stops needing the bare metal item.
    #[test]
    fn check_attach_ignores_series() {
        let products = stub::products();
        let organization = OrganizationUuid::new();
        let deal = make_subscription_item("price_deal", ENTERPRISE_PRODUCT);
        let bare_metal = make_subscription_item("price_bare_metal", BARE_METAL_PRODUCT);
        assert_eq!(
            check_attach(
                &products,
                PlanStatus::Active,
                &[deal.clone(), bare_metal],
                &series_named(None, "not a count"),
                organization
            ),
            Ok(Attachable {
                level: PlanLevel::Enterprise,
                named: false,
            })
        );
        assert_eq!(
            check_attach(
                &products,
                PlanStatus::Active,
                std::slice::from_ref(&deal),
                &series_named(None, "500"),
                organization
            ),
            Err(AttachRefusal::NoBareMetal)
        );
    }

    // Fails if a period that ends when or before it starts makes a term, or one that ends a
    // second after it starts does not.
    #[test]
    fn license_term_ends_after_it_starts() {
        let start = DateTime::try_from(NOW).unwrap();
        for end in [start, DateTime::try_from(start.timestamp() - 1).unwrap()] {
            assert_eq!(
                LicenseTerm::new(entitlements(500), start, end),
                Err(LicenseTermError::Ended {
                    issued_at: start,
                    term_end: end,
                })
            );
        }
        let end = DateTime::try_from(start.timestamp() + 1).unwrap();
        assert_eq!(
            LicenseTerm::new(entitlements(500), start, end),
            Ok(LicenseTerm {
                series: entitlements(500),
                issued_at: start,
                term_end: end,
            })
        );
    }

    // Fails if the `series` metadata reads as anything but a whole count above zero.
    #[test]
    fn series_reads_a_count() {
        assert_eq!(series(&series_named(None, "1000")), Ok(entitlements(1_000)));
        assert_eq!(series(&HashMap::new()), Err(LicenseTermError::NoSeries));
        assert_eq!(
            series(&series_named(None, "1,000")),
            Err(LicenseTermError::BadSeries("1,000".to_owned()))
        );
    }

    /// The claims of a license held for 1,000 Benchmark Series, whose term ends at `term_end`.
    fn held(term_end: i64) -> LicenseClaims {
        LicenseClaims {
            aud: Audience::Bencher,
            exp: term_end + GRACE_PERIOD.num_seconds(),
            iat: term_end - 30 * DAY,
            iss: bencher_json::BENCHER_URL_STR.to_owned(),
            sub: OrganizationUuid::new(),
            ser: entitlements(1_000),
            trm: term_end,
        }
    }

    fn term(series: u32, term_end: i64) -> LicenseTerm {
        LicenseTerm {
            series: entitlements(series),
            issued_at: DateTime::try_from(term_end - 30 * DAY).unwrap(),
            term_end: DateTime::try_from(term_end).unwrap(),
        }
    }

    // Fails if a read of a licensed plan signs a new license for anything but an active
    // subscription on the Enterprise product whose term ends later than the license held, or
    // ends with it and covers another count, or ever signs one that moves the license back a
    // term.
    #[test]
    fn renewal_signs_only_a_later_term_or_another_count() {
        let held = held(NOW);
        let verified = HeldLicense::Verified(&held);
        let active = PlanStatus::Active;
        let enterprise = PlanLevel::Enterprise;

        // Renewed: the next period ends later.
        let renewed = term(1_000, NOW + 30 * DAY);
        assert_eq!(
            renewal(active, enterprise, &Ok(renewed), verified),
            Ok(Some(renewed))
        );
        assert_eq!(
            renewal(PlanStatus::Trialing, enterprise, &Ok(renewed), verified),
            Ok(Some(renewed))
        );
        // Upsold in the term: the same end, another count.
        let upsold = term(1_500, NOW);
        assert_eq!(
            renewal(active, enterprise, &Ok(upsold), verified),
            Ok(Some(upsold))
        );
        // Nothing changed.
        assert_eq!(
            renewal(active, enterprise, &Ok(term(1_000, NOW)), verified),
            Ok(None)
        );
        // An earlier term, whatever its count.
        for series in [1_000, 1_500] {
            assert_eq!(
                renewal(active, enterprise, &Ok(term(series, NOW - DAY)), verified),
                Ok(None)
            );
        }
        // Not active: canceled, or in any other status that is not active.
        for status in [
            PlanStatus::Canceled,
            PlanStatus::IncompleteExpired,
            PlanStatus::PastDue,
            PlanStatus::Unpaid,
            PlanStatus::Incomplete,
            PlanStatus::Paused,
        ] {
            assert_eq!(
                renewal(status, enterprise, &Ok(renewed), verified),
                Ok(None),
                "{status}"
            );
            assert_eq!(
                renewal(
                    status,
                    PlanLevel::Pro,
                    &Err(LicenseTermError::NoSeries),
                    verified
                ),
                Ok(None),
                "{status}"
            );
        }
        // An active subscription whose term cannot be signed says why.
        let ended = LicenseTermError::Ended {
            issued_at: DateTime::try_from(NOW + DAY).unwrap(),
            term_end: DateTime::try_from(NOW).unwrap(),
        };
        assert_eq!(
            renewal(active, enterprise, &Err(ended.clone()), verified),
            Err(RenewalError::Term(ended))
        );
        assert_eq!(
            renewal(
                active,
                enterprise,
                &Err(LicenseTermError::NoSeries),
                verified
            ),
            Err(RenewalError::Term(LicenseTermError::NoSeries))
        );
        // A plan item moved off the Enterprise product signs nothing, whatever its term.
        assert_eq!(
            renewal(active, PlanLevel::Pro, &Ok(renewed), verified),
            Err(RenewalError::NotEnterprise(PlanLevel::Pro))
        );
    }

    // Fails if a license whose key no longer verifies is not replaced by a license for its own
    // term or a later one, or is replaced by one that moves it back a term, or if the rest of
    // the rule does not hold for it.
    #[test]
    fn renewal_replaces_an_unverified_license_with_any_term_but_an_earlier_one() {
        let held = held(NOW);
        let unverified = HeldLicense::Unverified(&held);
        let active = PlanStatus::Active;
        let enterprise = PlanLevel::Enterprise;

        for kept in [
            term(1_000, NOW),
            term(1_000, NOW + 30 * DAY),
            term(1_500, NOW),
        ] {
            assert_eq!(
                renewal(active, enterprise, &Ok(kept), unverified),
                Ok(Some(kept))
            );
        }
        assert_eq!(
            renewal(active, enterprise, &Ok(term(1_000, NOW - DAY)), unverified),
            Ok(None)
        );
        assert_eq!(
            renewal(
                PlanStatus::Canceled,
                enterprise,
                &Ok(term(1_000, NOW)),
                unverified
            ),
            Ok(None)
        );
        assert_eq!(
            renewal(active, PlanLevel::Pro, &Ok(term(1_000, NOW)), unverified),
            Err(RenewalError::NotEnterprise(PlanLevel::Pro))
        );
    }

    /// A biller that sends every Stripe request to `stub`.
    fn stub_biller(stub: &StripeStub) -> Biller {
        default_provider().install_default().unwrap_or_default();
        Biller::for_stub(stub).unwrap()
    }

    /// The bare metal item at two dollars an hour, in cents per minute.
    fn bare_metal_item() -> Value {
        stub::item(&stub::price(
            "price_bare_metal",
            BARE_METAL_PRODUCT,
            "3.3333333333",
        ))
    }

    /// A deal of 1,000 series for $2,500 a month, then $0.40 a series.
    fn deal_item() -> Value {
        stub::item(&stub::tiered_price(
            "price_deal",
            ENTERPRISE_PRODUCT,
            "graduated",
            &json!([
                { "up_to": 1_000, "flat_amount": 250_000 },
                { "up_to": null, "unit_amount": 40 },
            ]),
        ))
    }

    fn pro_item() -> Value {
        stub::item(&stub::tiered_price(
            "price_pro",
            PRO_PRODUCT,
            "volume",
            &json!([
                { "up_to": 250, "flat_amount": 10_000 },
                { "up_to": null, "flat_amount": 0 },
            ]),
        ))
    }

    /// A customer that Bencher made, for a Bencher user.
    fn bencher_customer(payment_method: Option<&Value>) -> Value {
        stub::customer(
            &json!({ "uuid": "00000000-0000-4000-8000-000000000001" }),
            payment_method,
        )
    }

    /// A customer that staff made in Stripe, with no Bencher user behind it.
    fn stripe_customer() -> Value {
        stub::customer(&json!({}), None)
    }

    fn objects(subscriptions: Vec<Value>, sessions: Vec<Value>) -> StubObjects {
        let by_id = |objects: Vec<Value>| {
            objects
                .into_iter()
                .map(|object| (object["id"].as_str().unwrap().to_owned(), object))
                .collect()
        };
        StubObjects {
            subscriptions: by_id(subscriptions),
            sessions: by_id(sessions),
        }
    }

    /// The Bencher Cloud plan that an attach made.
    fn metered(attachment: Attachment) -> (MeteredPlanId, JsonPlan) {
        let Attachment::Metered(metered_plan_id, json_plan) = attachment else {
            panic!("not a Bencher Cloud plan: {attachment:?}");
        };
        (metered_plan_id, json_plan)
    }

    fn metadata_writes(stub: &StripeStub) -> Vec<(String, Vec<(String, String)>)> {
        stub.requests()
            .into_iter()
            .filter(|request| request.method == "POST")
            .map(|request| (request.path, request.form))
            .collect()
    }

    // Fails if a plan billed by invoice, for a customer that staff made in Stripe, does
    // not read, or reads with a card or a payer it does not have, or if the plan read takes
    // more than the one subscription request.
    #[tokio::test]
    async fn get_plan_reads_an_invoice_billed_deal() {
        let organization = OrganizationUuid::new();
        let stub = StripeStub::new(objects(
            vec![stub::subscription(
                "sub_invoice",
                "active",
                "send_invoice",
                vec![deal_item(), bare_metal_item()],
                json!({ "organization": organization.to_string() }),
                stripe_customer(),
                None,
            )],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);

        let plan = biller
            .get_metered_plan(&"sub_invoice".parse().unwrap())
            .await
            .unwrap();
        assert!(plan.customer.is_none());
        assert!(plan.card.is_none());
        assert_eq!(plan.organization, organization);
        assert_eq!(plan.level, PlanLevel::Enterprise);
        assert_eq!(u64::from(plan.unit_amount), 250_000);
        assert_eq!(plan.tiers.map(|tiers| tiers.len()), Some(2));
        assert_eq!(plan.tiers_mode, Some(TiersMode::Graduated));
        let rate = plan.runner_minute.unwrap();
        assert!((rate - 3.333_333_333_3).abs() < 1e-9, "{rate}");

        let paths: Vec<String> = stub.requests().into_iter().map(|r| r.path).collect();
        assert_eq!(paths, ["/v1/subscriptions/sub_invoice"]);
    }

    // Fails if a card that staff set on the customer, not on the subscription, is not the
    // plan's card, or a payment method that is not a card is read as one.
    #[tokio::test]
    async fn get_plan_reads_the_card_that_pays() {
        let organization = OrganizationUuid::new();
        let named = json!({ "organization": organization.to_string() });
        let stub = StripeStub::new(objects(
            vec![
                stub::subscription(
                    "sub_customer_card",
                    "active",
                    "charge_automatically",
                    vec![deal_item(), bare_metal_item()],
                    named.clone(),
                    bencher_customer(Some(&stub::card("pm_customer"))),
                    None,
                ),
                stub::subscription(
                    "sub_bank",
                    "active",
                    "charge_automatically",
                    vec![deal_item(), bare_metal_item()],
                    named,
                    bencher_customer(Some(&stub::card("pm_customer"))),
                    Some(stub::bank_account("pm_bank")),
                ),
            ],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);

        let plan = biller
            .get_metered_plan(&"sub_customer_card".parse().unwrap())
            .await
            .unwrap();
        let card = plan.card.unwrap();
        assert_eq!(card.last_four.as_ref(), "4242");
        assert!(plan.customer.is_some());
        assert_eq!(
            plan.collection_method,
            CollectionMethod::ChargeAutomatically
        );

        // Stripe debits the bank account on its own, so the plan is charged, with no card.
        let plan = biller
            .get_metered_plan(&"sub_bank".parse().unwrap())
            .await
            .unwrap();
        assert!(plan.card.is_none());
        assert_eq!(
            plan.collection_method,
            CollectionMethod::ChargeAutomatically
        );
    }

    // Fails if a plan that Stripe bills by invoice reads with a card, which Stripe never
    // charges for it, even when one is on file for the subscription and the customer.
    #[tokio::test]
    async fn get_plan_reads_no_card_for_a_plan_billed_by_invoice() {
        let organization = OrganizationUuid::new();
        let stub = StripeStub::new(objects(
            vec![stub::subscription(
                "sub_invoice_card",
                "active",
                "send_invoice",
                vec![deal_item(), bare_metal_item()],
                json!({ "organization": organization.to_string() }),
                bencher_customer(Some(&stub::card("pm_customer"))),
                Some(stub::card("pm_subscription")),
            )],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);

        let plan = biller
            .get_metered_plan(&"sub_invoice_card".parse().unwrap())
            .await
            .unwrap();
        assert!(plan.card.is_none());
        assert_eq!(plan.collection_method, CollectionMethod::SendInvoice);
        assert!(plan.customer.is_some());
    }

    // Fails if a customer that Bencher made, with data Bencher cannot read, reads as no
    // customer rather than as an error.
    #[tokio::test]
    async fn get_plan_refuses_a_bencher_customer_with_bad_data() {
        let organization = OrganizationUuid::new();
        let stub = StripeStub::new(objects(
            vec![stub::subscription(
                "sub_bad_customer",
                "active",
                "charge_automatically",
                vec![deal_item(), bare_metal_item()],
                json!({ "organization": organization.to_string() }),
                stub::customer(&json!({ "uuid": "not-a-uuid" }), None),
                Some(stub::card("pm_card")),
            )],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);

        assert!(matches!(
            biller
                .get_metered_plan(&"sub_bad_customer".parse().unwrap())
                .await
                .unwrap_err(),
            crate::BillingError::BadUserUuid(..)
        ));
    }

    // Fails if a checkout body reaches Stripe as a subscription id, so a raw subscription
    // passed as a checkout session is never attached.
    #[tokio::test]
    async fn attach_checkout_reads_only_the_session() {
        let organization = OrganizationUuid::new();
        let stub = StripeStub::new(objects(
            vec![stub::subscription(
                "sub_raw",
                "active",
                "charge_automatically",
                vec![pro_item(), bare_metal_item()],
                json!({ "organization": organization.to_string() }),
                bencher_customer(Some(&stub::card("pm_card"))),
                Some(stub::card("pm_card")),
            )],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);

        let plan = JsonNewPlan::Checkout(JsonPlanCheckout {
            checkout: "sub_raw".parse().unwrap(),
        });
        assert!(matches!(
            biller.attach(&plan, organization, &licensor()).await.unwrap_err(),
            AttachError::NotFound(id) if id == "sub_raw"
        ));
        let paths: Vec<String> = stub.requests().into_iter().map(|r| r.path).collect();
        assert_eq!(paths, ["/v1/checkout/sessions/sub_raw"]);
    }

    // Fails if a checkout return takes an Enterprise plan or one that does not name its
    // organization, writes to Stripe, or does not read the Pro plan it takes.
    #[tokio::test]
    async fn attach_checkout_takes_only_its_own_pro_plan() {
        let organization = OrganizationUuid::new();
        let named = json!({ "organization": organization.to_string() });
        let customer = bencher_customer(Some(&stub::card("pm_card")));
        let card = Some(stub::card("pm_card"));
        let stub = StripeStub::new(objects(
            vec![
                stub::subscription(
                    "sub_enterprise",
                    "active",
                    "charge_automatically",
                    vec![deal_item(), bare_metal_item()],
                    named.clone(),
                    customer.clone(),
                    card.clone(),
                ),
                stub::subscription(
                    "sub_unnamed",
                    "trialing",
                    "charge_automatically",
                    vec![pro_item(), bare_metal_item()],
                    json!({}),
                    customer.clone(),
                    card.clone(),
                ),
                stub::subscription(
                    "sub_pro",
                    "trialing",
                    "charge_automatically",
                    vec![pro_item(), bare_metal_item()],
                    named,
                    customer,
                    card,
                ),
            ],
            vec![
                stub::session("cs_enterprise", "sub_enterprise"),
                stub::session("cs_unnamed", "sub_unnamed"),
                stub::session("cs_pro", "sub_pro"),
            ],
        ))
        .await;
        let biller = stub_biller(&stub);
        let checkout = |session: &str| {
            JsonNewPlan::Checkout(JsonPlanCheckout {
                checkout: session.parse().unwrap(),
            })
        };

        assert!(matches!(
            biller
                .attach(&checkout("cs_enterprise"), organization, &licensor())
                .await
                .unwrap_err(),
            AttachError::Refused(AttachRefusal::NotPro(PlanLevel::Enterprise))
        ));
        assert!(matches!(
            biller
                .attach(&checkout("cs_unnamed"), organization, &licensor())
                .await
                .unwrap_err(),
            AttachError::Refused(AttachRefusal::NoOrganization)
        ));
        let (metered_plan_id, plan) = metered(
            biller
                .attach(&checkout("cs_pro"), organization, &licensor())
                .await
                .unwrap(),
        );
        assert_eq!(metered_plan_id.as_ref(), "sub_pro");
        assert_eq!(plan.level, PlanLevel::Pro);
        assert_eq!(plan.tiers_mode, Some(TiersMode::Volume));
        assert_eq!(metadata_writes(&stub).len(), 0);
    }

    // Fails if an attach of a subscription that staff left unnamed does not name it for
    // the organization exactly once, names one that is already named, or does not read the
    // plan, card and payer included, that it attaches.
    #[tokio::test]
    async fn attach_names_an_unnamed_subscription_once() {
        let organization = OrganizationUuid::new();
        let stub = StripeStub::new(objects(
            vec![stub::subscription(
                "sub_deal",
                "active",
                "send_invoice",
                vec![deal_item(), bare_metal_item()],
                json!({ "series": "1000" }),
                stripe_customer(),
                None,
            )],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);
        let plan = JsonNewPlan::Subscription(JsonPlanSubscription {
            subscription: "sub_deal".parse().unwrap(),
            self_hosted: None,
        });

        let (metered_plan_id, json_plan) = metered(
            biller
                .attach(&plan, organization, &licensor())
                .await
                .unwrap(),
        );
        assert_eq!(metered_plan_id.as_ref(), "sub_deal");
        assert_eq!(json_plan.organization, organization);
        assert_eq!(json_plan.level, PlanLevel::Enterprise);
        assert!(json_plan.card.is_none());
        assert!(json_plan.customer.is_none());
        assert_eq!(
            metadata_writes(&stub),
            [(
                "/v1/subscriptions/sub_deal".to_owned(),
                vec![(
                    "metadata[organization]".to_owned(),
                    organization.to_string()
                )]
            )]
        );
        assert_eq!(
            stub.subscription("sub_deal")["metadata"],
            json!({ "series": "1000", "organization": organization.to_string() })
        );

        biller
            .attach(&plan, organization, &licensor())
            .await
            .unwrap();
        assert_eq!(metadata_writes(&stub).len(), 1);
    }

    // Fails if an attach writes to Stripe before it refuses a terminal subscription or a
    // price the plan cannot show.
    #[tokio::test]
    async fn attach_refuses_before_any_write() {
        let organization = OrganizationUuid::new();
        let mut unpriced = deal_item();
        unpriced["price"]["tiers"] = json!([{ "up_to": null, "unit_amount": 40 }]);
        let stub = StripeStub::new(objects(
            vec![
                stub::subscription(
                    "sub_canceled",
                    "canceled",
                    "send_invoice",
                    vec![deal_item(), bare_metal_item()],
                    json!({}),
                    stripe_customer(),
                    None,
                ),
                stub::subscription(
                    "sub_unpriced",
                    "active",
                    "send_invoice",
                    vec![unpriced, bare_metal_item()],
                    json!({}),
                    stripe_customer(),
                    None,
                ),
            ],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);
        let attach = |subscription: &str| {
            JsonNewPlan::Subscription(JsonPlanSubscription {
                subscription: subscription.parse().unwrap(),
                self_hosted: None,
            })
        };

        assert!(matches!(
            biller
                .attach(&attach("sub_canceled"), organization, &licensor())
                .await
                .unwrap_err(),
            AttachError::Refused(AttachRefusal::Terminal(PlanStatus::Canceled))
        ));
        assert!(matches!(
            biller
                .attach(&attach("sub_unpriced"), organization, &licensor())
                .await
                .unwrap_err(),
            AttachError::Refused(AttachRefusal::Price(price)) if price == "price_deal"
        ));
        assert_eq!(metadata_writes(&stub).len(), 0);
    }

    // Fails if an attach writes the `organization` metadata before it reads the plan, so a
    // subscription that does not read as a plan is never named for the organization.
    #[tokio::test]
    async fn attach_reads_the_plan_before_any_write() {
        let stub = StripeStub::new(objects(
            vec![stub::subscription(
                "sub_bad_customer",
                "active",
                "charge_automatically",
                vec![deal_item(), bare_metal_item()],
                json!({}),
                stub::customer(&json!({ "uuid": "not-a-uuid" }), None),
                Some(stub::card("pm_card")),
            )],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);
        let plan = JsonNewPlan::Subscription(JsonPlanSubscription {
            subscription: "sub_bad_customer".parse().unwrap(),
            self_hosted: None,
        });

        assert!(matches!(
            biller
                .attach(&plan, OrganizationUuid::new(), &licensor())
                .await
                .unwrap_err(),
            AttachError::Billing(crate::BillingError::BadUserUuid(..))
        ));
        assert_eq!(metadata_writes(&stub).len(), 0);
        assert_eq!(stub.subscription("sub_bad_customer")["metadata"], json!({}));
    }

    // Fails if a subscription id that Stripe does not have is anything but not found.
    #[tokio::test]
    async fn attach_unknown_subscription_is_not_found() {
        let stub = StripeStub::new(StubObjects::default()).await;
        let biller = stub_biller(&stub);
        let plan = JsonNewPlan::Subscription(JsonPlanSubscription {
            subscription: "sub_missing".parse().unwrap(),
            self_hosted: None,
        });

        assert!(matches!(
            biller
                .attach(&plan, OrganizationUuid::new(), &licensor())
                .await
                .unwrap_err(),
            AttachError::NotFound(id) if id == "sub_missing"
        ));
    }

    /// A Self-Hosted deal's flat price on the Enterprise product.
    fn self_hosted_item() -> Value {
        stub::item(&stub::price(
            "price_self_hosted",
            ENTERPRISE_PRODUCT,
            "1200000",
        ))
    }

    fn self_hosted_attach(subscription: &str, self_hosted: OrganizationUuid) -> JsonNewPlan {
        JsonNewPlan::Subscription(JsonPlanSubscription {
            subscription: subscription.parse().unwrap(),
            self_hosted: Some(self_hosted),
        })
    }

    // Fails if an attach for a Self-Hosted deal does not sign its first license for the
    // Self-Hosted organization, for the Benchmark Series in its `series` metadata, issued at the
    // start of its current period for a term that ends with the period, or does not name the
    // subscription for the organization that buys it, once, after the license is signed.
    #[tokio::test]
    async fn attach_self_hosted_signs_the_first_license() {
        let organization = OrganizationUuid::new();
        let self_hosted = OrganizationUuid::new();
        let stub = StripeStub::new(objects(
            vec![stub::subscription(
                "sub_self_hosted",
                "active",
                "send_invoice",
                vec![self_hosted_item()],
                json!({ "series": "500" }),
                stripe_customer(),
                None,
            )],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);
        let licensor = licensor();

        let attachment = biller
            .attach(
                &self_hosted_attach("sub_self_hosted", self_hosted),
                organization,
                &licensor,
            )
            .await
            .unwrap();
        let Attachment::Licensed(licensed_plan_id, license, json_plan) = attachment else {
            panic!("not a Self-Hosted license: {attachment:?}");
        };
        assert_eq!(licensed_plan_id.as_ref(), "sub_self_hosted");
        let claims = licensor.read(&license).unwrap();
        assert_eq!(claims.sub, self_hosted);
        assert_eq!(claims.ser, entitlements(500));
        assert_eq!(claims.iat, NOW - DAY);
        assert_eq!(claims.trm, NOW + DAY);
        assert_eq!(claims.exp, NOW + DAY + GRACE_PERIOD.num_seconds());

        assert_eq!(json_plan.organization, organization);
        assert_eq!(json_plan.level, PlanLevel::Enterprise);
        assert!(json_plan.runner_minute.is_none());
        let json_license = json_plan.license.unwrap();
        assert_eq!(json_license.key, license);
        assert_eq!(json_license.organization, self_hosted);
        assert_eq!(u32::from(json_license.series), 500);
        assert_eq!(
            stub.subscription("sub_self_hosted")["metadata"],
            json!({ "series": "500", "organization": organization.to_string() })
        );
        assert_eq!(metadata_writes(&stub).len(), 1);
    }

    // Fails if an attach for a Self-Hosted deal writes to Stripe before it refuses a Pro
    // subscription, one without a count in its `series` metadata, a canceled one, an incomplete
    // one, or one whose current period has already ended, or if any of these refusals is not
    // the one it is.
    #[tokio::test]
    async fn attach_self_hosted_refuses_before_any_write() {
        let subscription = |id: &str, status: &str, items: Vec<Value>, metadata: Value| {
            stub::subscription(
                id,
                status,
                "send_invoice",
                items,
                metadata,
                stripe_customer(),
                None,
            )
        };
        let series = json!({ "series": "500" });
        let ended = stub::item_in(
            &stub::price("price_self_hosted", ENTERPRISE_PRODUCT, "1200000"),
            NOW,
            NOW,
        );
        let stub = StripeStub::new(objects(
            vec![
                subscription(
                    "sub_pro",
                    "active",
                    vec![pro_item(), bare_metal_item()],
                    series.clone(),
                ),
                subscription(
                    "sub_no_series",
                    "active",
                    vec![self_hosted_item()],
                    json!({}),
                ),
                subscription(
                    "sub_bad_series",
                    "active",
                    vec![self_hosted_item()],
                    json!({ "series": "lots" }),
                ),
                subscription(
                    "sub_canceled",
                    "canceled",
                    vec![self_hosted_item()],
                    series.clone(),
                ),
                subscription("sub_ended", "active", vec![ended], series.clone()),
                subscription(
                    "sub_incomplete",
                    "incomplete",
                    vec![self_hosted_item()],
                    series,
                ),
            ],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);
        let refusal = async |subscription: &str| {
            let error = biller
                .attach(
                    &self_hosted_attach(subscription, OrganizationUuid::new()),
                    OrganizationUuid::new(),
                    &licensor(),
                )
                .await
                .unwrap_err();
            let AttachError::Refused(refusal) = error else {
                panic!("not refused: {error}");
            };
            refusal
        };

        assert_eq!(
            refusal("sub_pro").await,
            AttachRefusal::NotEnterprise(PlanLevel::Pro)
        );
        assert_eq!(
            refusal("sub_no_series").await,
            AttachRefusal::Term(LicenseTermError::NoSeries)
        );
        assert_eq!(
            refusal("sub_bad_series").await,
            AttachRefusal::Term(LicenseTermError::BadSeries("lots".to_owned()))
        );
        assert_eq!(
            refusal("sub_canceled").await,
            AttachRefusal::Terminal(PlanStatus::Canceled)
        );
        assert_eq!(
            refusal("sub_incomplete").await,
            AttachRefusal::NotActive(PlanStatus::Incomplete)
        );
        let at = DateTime::try_from(NOW).unwrap();
        assert_eq!(
            refusal("sub_ended").await,
            AttachRefusal::Term(LicenseTermError::Ended {
                issued_at: at,
                term_end: at,
            })
        );
        assert_eq!(metadata_writes(&stub).len(), 0);
    }

    // Fails if a licensed plan does not read with the term its subscription pays for now, or
    // does not read at all once staff remove its `series` metadata.
    #[tokio::test]
    async fn get_licensed_plan_reads_the_term() {
        let organization = OrganizationUuid::new();
        let renewed = stub::item_in(
            &stub::price("price_self_hosted", ENTERPRISE_PRODUCT, "1200000"),
            NOW + DAY,
            NOW + 366 * DAY,
        );
        let stub = StripeStub::new(objects(
            vec![
                stub::subscription(
                    "sub_renewed",
                    "active",
                    "send_invoice",
                    vec![renewed],
                    json!({ "series": "750", "organization": organization.to_string() }),
                    stripe_customer(),
                    None,
                ),
                stub::subscription(
                    "sub_unseries",
                    "canceled",
                    "send_invoice",
                    vec![self_hosted_item()],
                    json!({ "organization": organization.to_string() }),
                    stripe_customer(),
                    None,
                ),
            ],
            vec![],
        ))
        .await;
        let biller = stub_biller(&stub);

        let renewed = biller
            .get_licensed_plan(&"sub_renewed".parse().unwrap())
            .await
            .unwrap();
        assert_eq!(renewed.plan.organization, organization);
        assert_eq!(renewed.plan.status, PlanStatus::Active);
        assert_eq!(
            renewed.term,
            Ok(LicenseTerm {
                series: entitlements(750),
                issued_at: DateTime::try_from(NOW + DAY).unwrap(),
                term_end: DateTime::try_from(NOW + 366 * DAY).unwrap(),
            })
        );

        let unseries = biller
            .get_licensed_plan(&"sub_unseries".parse().unwrap())
            .await
            .unwrap();
        assert_eq!(unseries.plan.status, PlanStatus::Canceled);
        assert_eq!(unseries.term, Err(LicenseTermError::NoSeries));
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
            },
            enterprise: JsonEnterpriseProduct {
                id: "prod_NLC7fDet2C8Nmk".into(),
            },
            bare_metal: JsonProduct {
                id: "prod_U6a28ecwqRZHYz".into(),
                metered: hmap! {
                    "default".to_owned() => "price_1T8Ms4Kal5vzTlmhSAPxSbrT".to_owned(),
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

    async fn metered_subscription(
        biller: &Biller,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        price_name: String,
        runner_minutes: usize,
        series_quantity: u32,
    ) {
        let create_subscription = biller
            .create_metered_subscription(
                organization,
                customer_id,
                payment_method_id.clone(),
                price_name.clone(),
            )
            .await
            .unwrap();

        let subscription_id = &create_subscription.id;
        let get_subscription = biller.get_subscription(subscription_id).await.unwrap();
        assert_eq!(create_subscription.id, get_subscription.id);

        let metered_plan_id = &attach_pro(biller, subscription_id, organization).await;

        let json_plan = biller.get_metered_plan(metered_plan_id).await.unwrap();
        assert_eq!(json_plan.level, PlanLevel::Pro);
        assert_eq!(json_plan.organization, organization);
        assert!(json_plan.tiers.is_some());

        let billing = biller
            .get_metered_plan_billing(metered_plan_id)
            .await
            .unwrap();
        // Pro starts in a native free trial.
        assert_eq!(billing.status, PlanStatus::Trialing);
        let customer_id = billing.customer_id;

        record_series_usage(biller, &customer_id, series_quantity).await;
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
        // Snapshot the latest invoice just before cancellation. Only the id is kept
        // across the awaits below; holding the whole `Subscription` would bloat the
        // future (`clippy::large_futures`).
        let pre_cancel_invoice_id = resumed.latest_invoice.map(Expandable::into_id);

        open_portal(biller, metered_plan_id).await;

        // DELETE path: immediate cancel. `invoice_now` generates a final invoice
        // so the usage recorded above is billed instead of discarded.
        let final_invoice_id = biller
            .cancel_metered_subscription(metered_plan_id)
            .await
            .unwrap()
            .latest_invoice
            .map(Expandable::into_id)
            .expect("final invoice for un-invoiced metered usage");
        // The final invoice must be new: distinct from the pre-cancel one when that
        // exists, and its mere presence (via `expect` above) proves generation when
        // no prior invoice existed.
        assert_ne!(Some(final_invoice_id), pre_cancel_invoice_id);
        let billing = biller
            .get_metered_plan_billing(metered_plan_id)
            .await
            .unwrap();
        assert_eq!(billing.status, PlanStatus::Canceled);

        resubscribe_has_no_trial(
            biller,
            organization,
            customer_id,
            payment_method_id,
            price_name,
        )
        .await;
    }

    /// The attach takes a Pro subscription for its own organization only, and reads it
    /// with its tiers, card, and payer.
    async fn attach_pro(
        biller: &Biller,
        subscription_id: &stripe_billing::SubscriptionId,
        organization: OrganizationUuid,
    ) -> MeteredPlanId {
        let plan = JsonNewPlan::Subscription(JsonPlanSubscription {
            subscription: subscription_id.as_ref().parse().unwrap(),
            self_hosted: None,
        });
        let (metered_plan_id, json_plan) = metered(
            biller
                .attach(&plan, organization, &licensor())
                .await
                .unwrap(),
        );
        assert_eq!(json_plan.tiers_mode, Some(TiersMode::Volume));
        assert!(json_plan.card.is_some());
        assert!(json_plan.customer.is_some());
        assert!(json_plan.runner_minute.is_some());
        assert!(matches!(
            biller
                .attach(&plan, OrganizationUuid::new(), &licensor())
                .await
                .unwrap_err(),
            AttachError::Refused(AttachRefusal::OtherOrganization(_))
        ));
        metered_plan_id
    }

    /// Re-subscribing after cancellation must not restart the free trial: the
    /// customer's prior (canceled) subscription disqualifies it, so the new Pro
    /// subscription is `Active` immediately, not `Trialing` (trial cycling guard).
    async fn resubscribe_has_no_trial(
        biller: &Biller,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        price_name: String,
    ) {
        let resubscription = biller
            .create_metered_subscription(organization, customer_id, payment_method_id, price_name)
            .await
            .unwrap();
        let metered_plan_id = &resubscription.id.as_ref().parse().unwrap();
        let billing = biller
            .get_metered_plan_billing(metered_plan_id)
            .await
            .unwrap();
        assert_eq!(billing.status, PlanStatus::Active);
        biller
            .cancel_metered_subscription(metered_plan_id)
            .await
            .unwrap();
    }

    /// The test-mode price of a deal, found again by this lookup key so runs share it.
    const DEAL_LOOKUP_KEY: &str = "bencher_test_enterprise_deal";
    /// The deal's flat fee, in cents, for its first 1,000 series.
    const DEAL_CENTS: i64 = 250_000;

    /// A deal's price as staff make it on the Enterprise product: graduated and metered on
    /// the active series meter, a flat fee for the first 1,000 series, then $0.40 a series.
    async fn deal_price(biller: &Biller) -> String {
        let found = ListPrice::new()
            .lookup_keys(vec![DEAL_LOOKUP_KEY.to_owned()])
            .active(true)
            .send(&biller.client)
            .await
            .unwrap();
        if let Some(price) = found.data.into_iter().next() {
            return price.id.to_string();
        }
        let meter = biller
            .products
            .pro
            .price(DEFAULT_PRICE_NAME)
            .unwrap()
            .recurring
            .as_ref()
            .and_then(|recurring| recurring.meter.clone())
            .expect("the Pro price is metered on active series");
        let mut recurring = CreatePriceRecurring::new(CreatePriceRecurringInterval::Month);
        recurring.meter = Some(meter);
        recurring.usage_type = Some(CreatePriceRecurringUsageType::Metered);
        let mut commitment = CreatePriceTiers::new(CreatePriceTiersUpTo::I64(1_000));
        commitment.flat_amount = Some(DEAL_CENTS);
        let mut overage = CreatePriceTiers::new(CreatePriceTiersUpTo::Inf);
        overage.unit_amount = Some(40);
        CreatePrice::new(Currency::USD)
            .product(biller.products.enterprise.as_str())
            .lookup_key(DEAL_LOOKUP_KEY)
            .transfer_lookup_key(true)
            .billing_scheme(stripe_shared::PriceBillingScheme::Tiered)
            .tiers_mode(stripe_shared::PriceTiersMode::Graduated)
            .tiers(vec![commitment, overage])
            .recurring(recurring)
            .send(&biller.client)
            .await
            .unwrap()
            .id
            .to_string()
    }

    /// The deal price and the bare metal price, as subscription items.
    async fn deal_items(biller: &Biller) -> Vec<CreateSubscriptionItems> {
        let bare_metal = biller
            .products
            .bare_metal
            .price(DEFAULT_PRICE_NAME)
            .unwrap()
            .id
            .to_string();
        [deal_price(biller).await, bare_metal]
            .into_iter()
            .map(|price| {
                let mut item = CreateSubscriptionItems::new();
                item.price = Some(price);
                item
            })
            .collect()
    }

    fn attach_body(subscription_id: &stripe_billing::SubscriptionId) -> JsonNewPlan {
        JsonNewPlan::Subscription(JsonPlanSubscription {
            subscription: subscription_id.as_ref().parse().unwrap(),
            self_hosted: None,
        })
    }

    /// A Cloud Enterprise subscription as staff create it for a card: the deal's own
    /// price on the Enterprise product, the bare metal price, and no `organization`
    /// metadata, which the attach writes.
    async fn enterprise_subscription(
        biller: &Biller,
        organization: OrganizationUuid,
        customer_id: CustomerId,
        payment_method_id: PaymentMethodId,
        runner_minutes: usize,
        series_quantity: u32,
    ) {
        let subscription_id = CreateSubscription::new()
            .customer(customer_id.to_string())
            .items(deal_items(biller).await)
            .default_payment_method(payment_method_id.to_string())
            .send(&biller.client)
            .await
            .unwrap()
            .id;

        let (metered_plan_id, json_plan) = metered(
            biller
                .attach(&attach_body(&subscription_id), organization, &licensor())
                .await
                .unwrap(),
        );
        let subscription = biller.get_subscription(&subscription_id).await.unwrap();
        assert_eq!(
            subscription.metadata.get(METADATA_ORGANIZATION),
            Some(&organization.to_string())
        );
        assert_eq!(json_plan.level, PlanLevel::Enterprise);
        assert!(json_plan.card.is_some());

        let json_plan = biller.get_metered_plan(&metered_plan_id).await.unwrap();
        assert_eq!(json_plan.level, PlanLevel::Enterprise);
        assert_eq!(json_plan.organization, organization);
        assert_eq!(u64::from(json_plan.unit_amount), DEAL_CENTS.unsigned_abs());
        assert_eq!(json_plan.tiers_mode, Some(TiersMode::Graduated));
        assert_eq!(json_plan.tiers.map(|tiers| tiers.len()), Some(2));

        let billing = biller
            .get_metered_plan_billing(&metered_plan_id)
            .await
            .unwrap();
        assert_eq!(billing.status, PlanStatus::Active);
        record_series_usage(biller, &billing.customer_id, series_quantity).await;
        record_runner_usage(biller, &billing.customer_id, runner_minutes).await;

        biller
            .cancel_metered_subscription(&metered_plan_id)
            .await
            .unwrap();
    }

    /// A Cloud Enterprise subscription billed by invoice, for a customer that staff made
    /// in Stripe: it attaches and reads with no card and no payer.
    async fn invoiced_enterprise_subscription(biller: &Biller, organization: OrganizationUuid) {
        let customer_id = CreateCustomer::new()
            .name("Courage")
            .email(format!("courage.{}@nowhere.com", rand::random::<u64>()))
            .send(&biller.client)
            .await
            .unwrap()
            .id;
        let subscription_id = CreateSubscription::new()
            .customer(customer_id.to_string())
            .items(deal_items(biller).await)
            .collection_method(stripe_shared::SubscriptionCollectionMethod::SendInvoice)
            .days_until_due(30u32)
            .send(&biller.client)
            .await
            .unwrap()
            .id;

        let (metered_plan_id, json_plan) = metered(
            biller
                .attach(&attach_body(&subscription_id), organization, &licensor())
                .await
                .unwrap(),
        );
        assert_eq!(json_plan.level, PlanLevel::Enterprise);
        assert!(json_plan.card.is_none());
        assert!(json_plan.customer.is_none());

        let json_plan = biller.get_metered_plan(&metered_plan_id).await.unwrap();
        assert!(json_plan.card.is_none());
        assert!(json_plan.customer.is_none());
        assert_eq!(json_plan.organization, organization);

        biller
            .cancel_metered_subscription(&metered_plan_id)
            .await
            .unwrap();
    }

    /// The test-mode price of a Self-Hosted deal, found again by this lookup key so runs share
    /// it.
    const SELF_HOSTED_LOOKUP_KEY: &str = "bencher_test_self_hosted_deal";

    /// A Self-Hosted deal's price as staff make it on the Enterprise product: a flat fee a year,
    /// licensed, since nothing on Bencher Cloud meters a Self-Hosted server.
    async fn self_hosted_price(biller: &Biller) -> String {
        let found = ListPrice::new()
            .lookup_keys(vec![SELF_HOSTED_LOOKUP_KEY.to_owned()])
            .active(true)
            .send(&biller.client)
            .await
            .unwrap();
        if let Some(price) = found.data.into_iter().next() {
            return price.id.to_string();
        }
        let mut recurring = CreatePriceRecurring::new(CreatePriceRecurringInterval::Year);
        recurring.usage_type = Some(CreatePriceRecurringUsageType::Licensed);
        CreatePrice::new(Currency::USD)
            .product(biller.products.enterprise.as_str())
            .lookup_key(SELF_HOSTED_LOOKUP_KEY)
            .transfer_lookup_key(true)
            .unit_amount(1_200_000)
            .recurring(recurring)
            .send(&biller.client)
            .await
            .unwrap()
            .id
            .to_string()
    }

    /// A Self-Hosted deal as staff create it: billed by invoice, for a customer that staff made
    /// in Stripe, on a licensed price on the Enterprise product, with `series` metadata and no
    /// `organization` metadata, which the attach writes. The attach signs the first license
    /// from Stripe's own period.
    async fn self_hosted_subscription(biller: &Biller, organization: OrganizationUuid) {
        let customer_id = CreateCustomer::new()
            .name("Eustace")
            .email(format!("eustace.{}@nowhere.com", rand::random::<u64>()))
            .send(&biller.client)
            .await
            .unwrap()
            .id;
        let mut item = CreateSubscriptionItems::new();
        item.price = Some(self_hosted_price(biller).await);
        let subscription = CreateSubscription::new()
            .customer(customer_id.to_string())
            .items(vec![item])
            .collection_method(stripe_shared::SubscriptionCollectionMethod::SendInvoice)
            .days_until_due(30u32)
            .metadata(HashMap::from([(
                METADATA_SERIES.to_owned(),
                "500".to_owned(),
            )]))
            .send(&biller.client)
            .await
            .unwrap();
        let item = &subscription.items.data[0];
        let (period_start, period_end) = (item.current_period_start, item.current_period_end);
        let subscription_id = subscription.id;

        let self_hosted = OrganizationUuid::new();
        let licensor = licensor();
        let plan = JsonNewPlan::Subscription(JsonPlanSubscription {
            subscription: subscription_id.as_ref().parse().unwrap(),
            self_hosted: Some(self_hosted),
        });
        let attachment = biller.attach(&plan, organization, &licensor).await.unwrap();
        let Attachment::Licensed(licensed_plan_id, license, json_plan) = attachment else {
            panic!("not a Self-Hosted license: {attachment:?}");
        };
        let claims = licensor.read(&license).unwrap();
        assert_eq!(claims.sub, self_hosted);
        assert_eq!(claims.ser, entitlements(500));
        assert_eq!(claims.iat, period_start);
        assert_eq!(claims.trm, period_end);
        assert_eq!(claims.exp, period_end + GRACE_PERIOD.num_seconds());
        assert_eq!(json_plan.level, PlanLevel::Enterprise);
        // Stripe makes a subscription billed by invoice active at once, paid or not.
        assert_eq!(json_plan.status, PlanStatus::Active);
        assert!(json_plan.card.is_none());
        let subscription = biller.get_subscription(&subscription_id).await.unwrap();
        assert_eq!(
            subscription.metadata.get(METADATA_ORGANIZATION),
            Some(&organization.to_string())
        );

        // Nothing has changed since the attach, so a read signs nothing new.
        let licensed = biller.get_licensed_plan(&licensed_plan_id).await.unwrap();
        assert_eq!(
            licensed.term,
            Ok(LicenseTerm {
                series: entitlements(500),
                issued_at: DateTime::try_from(period_start).unwrap(),
                term_end: DateTime::try_from(period_end).unwrap(),
            })
        );
        assert_eq!(licensed.renewal(HeldLicense::Verified(&claims)), Ok(None));

        biller
            .cancel_licensed_subscription(&licensed_plan_id)
            .await
            .unwrap();
    }

    async fn open_portal(biller: &Biller, metered_plan_id: &MeteredPlanId) {
        let portal = biller
            .new_portal_session(metered_plan_id.clone().into(), "https://bencher.dev")
            .await
            .unwrap();
        assert!(portal.starts_with("https://"));
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

    async fn record_series_usage(biller: &Biller, customer_id: &CustomerId, quantity: u32) {
        // Live Stripe test: a meter event needs a real, recent timestamp (a fixed
        // `DateTime::TEST` would be rejected as outside Stripe's accepted window).
        let event = biller
            .record_series_usage(customer_id, quantity, DateTime::now())
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

    fn price_tier(
        up_to: Option<i64>,
        unit_amount: Option<i64>,
        flat_amount: Option<i64>,
    ) -> PriceTier {
        PriceTier {
            flat_amount,
            flat_amount_decimal: None,
            unit_amount,
            unit_amount_decimal: None,
            up_to,
        }
    }

    fn make_price(price_id: &str, product_id: &str) -> stripe_shared::Price {
        stripe_shared::Price {
            active: false,
            billing_scheme: stripe_shared::PriceBillingScheme::PerUnit,
            created: 0,
            currency: Currency::USD,
            currency_options: None,
            custom_unit_amount: None,
            id: price_id.parse().unwrap(),
            livemode: false,
            lookup_key: None,
            metadata: HashMap::new(),
            nickname: None,
            product: Expandable::Id(product_id.parse::<stripe_shared::ProductId>().unwrap()),
            recurring: None,
            tax_behavior: None,
            tiers: None,
            tiers_mode: None,
            transform_quantity: None,
            type_: stripe_shared::PriceType::Recurring,
            unit_amount: Some(100),
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
            currency: Currency::USD,
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

    fn make_subscription_item(
        price_id: &str,
        product_id: &str,
    ) -> stripe_billing::SubscriptionItem {
        stripe_billing::SubscriptionItem {
            billed_until: None,
            billing_thresholds: None,
            created: 0,
            current_period_end: 0,
            current_period_start: 0,
            discounts: vec![],
            id: format!("si_test_{price_id}").parse().unwrap(),
            metadata: HashMap::new(),
            plan: make_plan(),
            price: make_price(price_id, product_id),
            quantity: None,
            subscription: "sub_test".to_owned(),
            tax_rates: None,
        }
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
        Box::pin(metered_subscription(
            &biller,
            organization,
            customer_id.clone(),
            payment_method_id.clone(),
            DEFAULT_PRICE_NAME.into(),
            3,
            7,
        ))
        .await;

        // Enterprise Cloud Plan, billed to a card
        let organization = OrganizationUuid::new();
        Box::pin(enterprise_subscription(
            &biller,
            organization,
            customer_id,
            payment_method_id,
            2,
            11,
        ))
        .await;

        // Enterprise Cloud Plan, billed by invoice
        Box::pin(invoiced_enterprise_subscription(
            &biller,
            OrganizationUuid::new(),
        ))
        .await;

        // Enterprise Self-Hosted license, billed by invoice
        Box::pin(self_hosted_subscription(&biller, OrganizationUuid::new())).await;
    }
}
