import * as Sentry from "@sentry/astro";
import type { Params } from "astro";
import {
	For,
	Match,
	type Resource,
	Show,
	Switch,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import consoleConfig from "../../../config/console";
import { Host } from "../../../config/organization/billing";
import { BencherResource } from "../../../config/types";
import {
	type JsonAuthUser,
	type JsonUsage,
	type Jwt,
	PlanLevel,
	UsageKind,
} from "../../../types/bencher";
import { authUser } from "../../../util/auth";
import { useSearchParams } from "../../../util/url";
import {
	currentSeriesTier,
	fmtDate,
	fmtTierPrice,
	fmtUsd,
	fmtUsdPrecise,
	isContactTier,
	isFirstBillingPeriod,
	planLevel,
	planLevelPrice,
	runnerMinutePrice,
	seriesTierRange,
	tierEstimateUsd,
} from "../../../util/convert";
import { BENCHER_CALENDLY_URL } from "../../../util/ext";
import { httpGet, httpPatch } from "../../../util/http";
import { NotifyKind, pageNotify } from "../../../util/notify";
import { type InitValid, init_valid, validJwt } from "../../../util/valid";
import { PLAN_PARAM } from "../../auth/auth";
import Field from "../../field/Field";
import FieldKind from "../../field/kind";
import ConsoleFallbackPricingTable from "../../pricing/ConsoleFallbackPricingTable";
import type { BillingHeaderConfig } from "./BillingHeader";
import BillingHeader from "./BillingHeader";
import BillingForm from "./plan/BillingForm";
import CheckoutLoading from "./plan/CheckoutLoading";
import PaymentMethod from "./plan/PaymentMethod";

interface Props {
	apiUrl: string;
	isBencherCloud: boolean;
	params: Params;
	onboard?: boolean;
}

export interface BillingPanelConfig {
	header: BillingHeaderConfig;
	host: Host;
}

const BillingPanel = (props: Props) => {
	const [bencher_valid] = createResource(init_valid);
	const user = authUser();
	const config = createMemo<BillingPanelConfig>(
		() =>
			consoleConfig[BencherResource.BILLING]?.[
				props.isBencherCloud ? Host.BENCHER_CLOUD : Host.SELF_HOSTED
			],
	);

	const fetcher = createMemo(() => {
		return {
			params: props.params,
			bencher_valid: bencher_valid(),
			token: user?.token,
		};
	});
	const fetchPlan = async (fetcher: {
		params: Params;
		bencher_valid: InitValid;
		token: string;
	}) => {
		if (!fetcher.bencher_valid || !validJwt(fetcher.token)) {
			return null;
		}
		const path = `/v0/organizations/${fetcher.params.organization}/usage`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				// console.log(resp.data);
				return resp?.data;
			})
			.catch((_error) => {
				// This is not an error because it is expected
				// console.log(error);
				return null;
			});
	};
	const [usage, { refetch }] = createResource<null | JsonUsage>(
		fetcher,
		fetchPlan,
	);

	return (
		<Show
			when={!props.onboard}
			fallback={
				<BillingPanelSwitch
					apiUrl={props.apiUrl}
					params={props.params}
					onboard={true}
					bencher_valid={bencher_valid}
					user={user}
					usage={usage}
					handleRefresh={refetch}
				/>
			}
		>
			<BillingHeader config={config()?.header} />
			<section class="section">
				<div class="container">
					<div class="columns">
						<div class="column">
							<BillingPanelSwitch
								apiUrl={props.apiUrl}
								params={props.params}
								onboard={false}
								bencher_valid={bencher_valid}
								user={user}
								usage={usage}
								handleRefresh={refetch}
							/>
						</div>
					</div>
				</div>
			</section>
		</Show>
	);
};

const BillingPanelSwitch = (props: {
	apiUrl: string;
	params: Params;
	onboard: boolean;
	bencher_valid: Resource<InitValid>;
	user: JsonAuthUser;
	usage: Resource<null | JsonUsage>;
	handleRefresh: () => void;
}) => {
	const [searchParams] = useSearchParams();
	// With ?plan=pro the checkout activates automatically (onboarding, or a
	// /pricing deep link to billing). While the usage resource loads, show the
	// redirect loader instead of the pricing table so the visitor sees one calm
	// loading state, not a flash of plans before being sent to checkout.
	const autoActivatePro = createMemo(
		() => searchParams[PLAN_PARAM] === PlanLevel.Pro,
	);
	return (
		<Switch
			fallback={
				<Show
					when={autoActivatePro()}
					fallback={
						<ConsoleFallbackPricingTable
							freeCtaText={
								props.onboard ? "Continue with Free" : "Sign up for Free"
							}
						/>
					}
				>
					<CheckoutLoading onboard={props.onboard} />
				</Show>
			}
		>
			{/* Bencher Cloud */}
			<Match when={props.usage()?.kind === UsageKind.CloudFree}>
				<BillingForm
					apiUrl={props.apiUrl}
					params={props.params}
					onboard={props.onboard ?? false}
					bencher_valid={props.bencher_valid}
					user={props.user}
					usage={props.usage}
					handleRefresh={props.handleRefresh}
				/>
			</Match>
			<Match when={props.usage()?.kind === UsageKind.CloudMetered}>
				<CloudMeteredPanel
					apiUrl={props.apiUrl}
					user={props.user}
					usage={props.usage}
					handleRefresh={props.handleRefresh}
				/>
			</Match>
			<Match when={props.usage()?.kind === UsageKind.CloudSelfHostedLicensed}>
				<CloudSelfHostedLicensedPanel
					onboard={props.onboard}
					usage={props.usage}
				/>
			</Match>
			{/* Self-Hosted */}
			<Match when={props.usage()?.kind === UsageKind.SelfHostedFree}>
				<SelfHostedFreePanel
					apiUrl={props.apiUrl}
					params={props.params}
					onboard={props.onboard}
					bencher_valid={props.bencher_valid}
					usage={props.usage}
					refetch={props.handleRefresh}
				/>
			</Match>
			<Match when={props.usage()?.kind === UsageKind.SelfHostedLicensed}>
				<SelfHostedLicensedPanel usage={props.usage} />
			</Match>
		</Switch>
	);
};

const manageSubscription = (
	<a href="https://pay.bencher.dev/p/login/5kAbJU83ieF8dTG5kk">
		Click here to manage your subscription.
	</a>
);

const CancelPlanButton = (props: {
	apiUrl: string;
	user: JsonAuthUser;
	organization: undefined | string;
	handleRefresh: () => void;
}) => {
	const [clicked, setClicked] = createSignal(false);
	const [canceling, setCanceling] = createSignal(false);

	const sendCancel = () => {
		const token = props.user?.token;
		const organization = props.organization;
		if (!validJwt(token) || !organization) {
			return;
		}
		setCanceling(true);
		httpPatch(props.apiUrl, `/v0/organizations/${organization}/plan`, token, {
			cancel_at_period_end: true,
		})
			.then((_resp) => {
				setCanceling(false);
				setClicked(false);
				props.handleRefresh();
				pageNotify(
					NotifyKind.OK,
					"Your Pro plan will cancel at the end of the current billing period. You keep access until then.",
				);
			})
			.catch((error) => {
				setCanceling(false);
				console.error(error);
				Sentry.captureException(error);
				pageNotify(
					NotifyKind.ERROR,
					"Lettuce romaine calm! Failed to cancel your plan. Please, try again.",
				);
			});
	};

	return (
		<Switch>
			<Match when={!clicked()}>
				<button
					class="button is-small"
					type="button"
					onMouseDown={(e) => {
						e.preventDefault();
						setClicked(true);
					}}
				>
					Cancel plan
				</button>
			</Match>
			<Match when={clicked()}>
				<div class="content">
					<p>
						Cancel your plan? It stays active until the end of the current
						billing period, then downgrades to Free. Private projects will no
						longer be accessible after that.
					</p>
				</div>
				<div class="columns">
					<div class="column">
						<button
							class="button is-fullwidth"
							type="submit"
							disabled={canceling()}
							onMouseDown={(e) => {
								e.preventDefault();
								sendCancel();
							}}
						>
							Yes, cancel at period end
						</button>
					</div>
					<div class="column">
						<button
							class="button is-primary is-fullwidth"
							type="button"
							onMouseDown={(e) => {
								e.preventDefault();
								setClicked(false);
							}}
						>
							Keep plan
						</button>
					</div>
				</div>
			</Match>
		</Switch>
	);
};

const ResumePlanButton = (props: {
	apiUrl: string;
	user: JsonAuthUser;
	organization: undefined | string;
	handleRefresh: () => void;
}) => {
	const [resuming, setResuming] = createSignal(false);

	const sendResume = () => {
		const token = props.user?.token;
		const organization = props.organization;
		if (!validJwt(token) || !organization) {
			return;
		}
		setResuming(true);
		httpPatch(props.apiUrl, `/v0/organizations/${organization}/plan`, token, {
			cancel_at_period_end: false,
		})
			.then((_resp) => {
				setResuming(false);
				props.handleRefresh();
				pageNotify(
					NotifyKind.OK,
					"Your Pro plan has been resumed. It will keep renewing each billing period.",
				);
			})
			.catch((error) => {
				setResuming(false);
				console.error(error);
				Sentry.captureException(error);
				pageNotify(
					NotifyKind.ERROR,
					"Lettuce romaine calm! Failed to resume your plan. Please, try again.",
				);
			});
	};

	return (
		<button
			class="button is-primary is-small"
			type="button"
			disabled={resuming()}
			onMouseDown={(e) => {
				e.preventDefault();
				sendResume();
			}}
		>
			Resume plan
		</button>
	);
};

// Shared cancel/resume control for any Bencher Cloud metered plan.
const PlanCancelResume = (props: {
	apiUrl: string;
	user: JsonAuthUser;
	usage: Resource<null | JsonUsage>;
	handleRefresh: () => void;
}) => {
	return (
		<Show
			when={props.usage()?.plan?.cancel_at_period_end}
			fallback={
				<CancelPlanButton
					apiUrl={props.apiUrl}
					user={props.user}
					organization={props.usage()?.organization}
					handleRefresh={props.handleRefresh}
				/>
			}
		>
			<p class="has-text-grey">
				Your plan is scheduled to cancel on{" "}
				{fmtDate(props.usage()?.plan?.current_period_end)}. You keep access
				until then.
			</p>
			<ResumePlanButton
				apiUrl={props.apiUrl}
				user={props.user}
				organization={props.usage()?.organization}
				handleRefresh={props.handleRefresh}
			/>
		</Show>
	);
};

// Bencher Cloud metered plans. Pro bills on active series (its own panel); legacy Team and
// metered Enterprise still bill on metrics.
const CloudMeteredPanel = (props: {
	apiUrl: string;
	user: JsonAuthUser;
	usage: Resource<null | JsonUsage>;
	handleRefresh: () => void;
}) => {
	const isPro = createMemo(() => props.usage()?.plan?.level === PlanLevel.Pro);
	return (
		<Show
			when={isPro()}
			fallback={
				<MeteredMetricsPanel
					apiUrl={props.apiUrl}
					user={props.user}
					usage={props.usage}
					handleRefresh={props.handleRefresh}
				/>
			}
		>
			<ProMeteredPanel
				apiUrl={props.apiUrl}
				user={props.user}
				usage={props.usage}
				handleRefresh={props.handleRefresh}
			/>
		</Show>
	);
};

// Legacy metered metrics plans (Team and metered Enterprise): billed per metric plus bare
// metal runner minutes.
const MeteredMetricsPanel = (props: {
	apiUrl: string;
	user: JsonAuthUser;
	usage: Resource<null | JsonUsage>;
	handleRefresh: () => void;
}) => {
	const metricPrice = createMemo(() =>
		planLevelPrice(props.usage()?.plan?.level),
	);
	const estMetricsCost = createMemo(
		() => (props.usage()?.metrics ?? 0) * metricPrice(),
	);
	const minutePrice = createMemo(() =>
		runnerMinutePrice(props.usage()?.plan?.level),
	);
	const estRunnerCost = createMemo(
		() => (props.usage()?.runner_minutes ?? 0) * minutePrice(),
	);
	const estTotalCost = createMemo(() => estMetricsCost() + estRunnerCost());

	return (
		<div class="content">
			<h2 class="title is-2">
				{planLevel(props.usage()?.plan?.level)} Tier (Bencher Cloud Metered)
			</h2>
			<h3 class="subtitle is-3">
				{fmtDate(props.usage()?.start_time)} -{" "}
				{fmtDate(props.usage()?.end_time)}
			</h3>
			<h4>Cost per Metric: {fmtUsd(metricPrice())}</h4>
			<h4>
				Estimated Metrics Used: {props.usage()?.metrics?.toLocaleString() ?? 0}
			</h4>
			<h4>Estimated Metrics Cost: {fmtUsd(estMetricsCost())}</h4>
			<br />
			<h4>Cost per Runner Minute: {fmtUsdPrecise(minutePrice())} / min</h4>
			<h4>
				Estimated Runner Minutes Used:{" "}
				{props.usage()?.runner_minutes?.toLocaleString() ?? 0}
			</h4>
			<h4>Estimated Runner Cost: {fmtUsd(estRunnerCost())}</h4>
			<br />
			<h4>Current Estimated Total: {fmtUsd(estTotalCost())}</h4>
			<br />
			<PaymentMethod usage={props.usage} />
			<br />
			{manageSubscription}
			<br />
			<br />
			<PlanCancelResume
				apiUrl={props.apiUrl}
				user={props.user}
				usage={props.usage}
				handleRefresh={props.handleRefresh}
			/>
		</div>
	);
};

// Pro plans: billed a flat monthly fee per active-series band, read from the Stripe price
// tiers (`usage.plan.tiers`). Bare metal runner minutes bill separately on top, except
// during the first-month free trial, when the base fee is waived and runner minutes are
// included (the whole bill is $0).
const ProMeteredPanel = (props: {
	apiUrl: string;
	user: JsonAuthUser;
	usage: Resource<null | JsonUsage>;
	handleRefresh: () => void;
}) => {
	const tiers = createMemo(() => props.usage()?.plan?.tiers);
	const activeSeries = createMemo(() => props.usage()?.active_series ?? 0);
	const currentTier = createMemo(() =>
		currentSeriesTier(tiers(), activeSeries()),
	);
	const currentRange = createMemo(() => {
		const tierList = tiers();
		const tier = currentTier();
		if (!tierList || !tier) {
			return "";
		}
		const index = tierList.indexOf(tier);
		return index >= 0 ? seriesTierRange(tierList, index) : "";
	});
	const contactSales = createMemo(() => isContactTier(currentTier()));
	const minutePrice = createMemo(() =>
		runnerMinutePrice(props.usage()?.plan?.level),
	);
	const estRunnerCost = createMemo(
		() => (props.usage()?.runner_minutes ?? 0) * minutePrice(),
	);
	const firstPeriod = createMemo(() =>
		isFirstBillingPeriod(
			props.usage()?.plan?.created,
			props.usage()?.plan?.current_period_start,
		),
	);
	// During the first-month trial the base fee is waived and runner minutes are included,
	// so the whole bill is $0. Otherwise it is the current band's flat fee plus runner cost.
	const estTotal = createMemo(() =>
		firstPeriod()
			? 0
			: (tierEstimateUsd(currentTier(), activeSeries()) ?? 0) + estRunnerCost(),
	);

	return (
		<div class="content">
			<h2 class="title is-2">
				{planLevel(props.usage()?.plan?.level)} Tier (Bencher Cloud Metered)
			</h2>
			<h3 class="subtitle is-3">
				{fmtDate(props.usage()?.start_time)} -{" "}
				{fmtDate(props.usage()?.end_time)}
			</h3>
			<h4>Active Benchmark Series: {activeSeries().toLocaleString()}</h4>
			<Show when={currentTier()}>
				<h4>
					Series Tier: {currentRange()} ({fmtTierPrice(currentTier())})
				</h4>
			</Show>
			<br />
			<h4>
				Bare Metal Runner Minutes Used:{" "}
				{props.usage()?.runner_minutes?.toLocaleString() ?? 0}
			</h4>
			<Show
				when={firstPeriod()}
				fallback={
					<>
						<h4>
							Cost per Runner Minute: {fmtUsdPrecise(minutePrice())} / min
						</h4>
						<h4>Estimated Runner Cost: {fmtUsd(estRunnerCost())}</h4>
					</>
				}
			>
				<h4>Runner minutes are included during your first month.</h4>
			</Show>
			<br />
			<Show
				when={!contactSales()}
				fallback={
					<p>
						Your usage is beyond the self-serve tiers.{" "}
						<a href={BENCHER_CALENDLY_URL} target="_blank" rel="noreferrer">
							Get in touch
						</a>{" "}
						to set up a plan that fits.
					</p>
				}
			>
				<h4>Current Estimated Total: {fmtUsd(estTotal())}</h4>
				<Show when={firstPeriod()}>
					<h4>
						First Month Free Trial: base fee waived and runner minutes included
					</h4>
					<h4>Estimated Total This Period: {fmtUsd(0)}</h4>
				</Show>
			</Show>
			<br />
			<h3 class="subtitle is-4">Pro pricing</h3>
			<p>
				You are billed a flat monthly fee based on your number of active
				benchmark series:
			</p>
			<table class="table">
				<thead>
					<tr>
						<th>Active series</th>
						<th>Price</th>
					</tr>
				</thead>
				<tbody>
					<For each={tiers() ?? []}>
						{(tier, index) => (
							<tr>
								<td>{seriesTierRange(tiers() ?? [], index())}</td>
								<td>
									<Show
										when={!isContactTier(tier)}
										fallback={
											<a
												href={BENCHER_CALENDLY_URL}
												target="_blank"
												rel="noreferrer"
											>
												Get in Touch
											</a>
										}
									>
										{fmtTierPrice(tier)}
									</Show>
								</td>
							</tr>
						)}
					</For>
				</tbody>
			</table>
			<br />
			<PaymentMethod usage={props.usage} />
			<br />
			{manageSubscription}
			<br />
			<br />
			<PlanCancelResume
				apiUrl={props.apiUrl}
				user={props.user}
				usage={props.usage}
				handleRefresh={props.handleRefresh}
			/>
		</div>
	);
};

const CloudSelfHostedLicensedPanel = (props: {
	onboard: boolean;
	usage: Resource<null | JsonUsage>;
}) => {
	return (
		<div class="content">
			<h2 class="title is-2">
				{planLevel(props.usage()?.license?.level)} Tier (Self-Hosted Licensed)
			</h2>
			<h3 class="subtitle is-3">
				{fmtDate(props.usage()?.start_time)} -{" "}
				{fmtDate(props.usage()?.end_time)}
			</h3>
			<h4>
				Entitlements:{" "}
				{props.usage()?.license?.entitlements?.toLocaleString() ?? 0}
			</h4>
			<h4>
				Self-Hosted Organization UUID:{" "}
				<code style="word-break: break-word;">
					{props.usage()?.license?.organization}
				</code>
			</h4>
			<h4>Self-Hosted License Key:</h4>
			<code style="word-break: break-word;">
				{/* biome-ignore lint/a11y/useValidAnchor: action on press */}
				<a
					title="Copy to clipboard"
					onMouseDown={(_) =>
						navigator.clipboard.writeText(props.usage()?.license?.key ?? "")
					}
				>
					{props.usage()?.license?.key}
				</a>
			</code>
			<h2 class="title is-2">
				What to do with your Bencher Self-Hosted License Key
			</h2>
			<h4>
				<ol>
					<li>
						{/* biome-ignore lint/a11y/useValidAnchor: copy to clipboard */}
						<a
							title="Copy to clipboard"
							onMouseDown={(_) =>
								navigator.clipboard.writeText(props.usage()?.license?.key ?? "")
							}
						>
							Click here to copy your Self-Hosted license key
						</a>
					</li>
					{props.onboard ? (
						<li>
							Navigate to the Organization Billing page in your Bencher
							Self-Hosted account
						</li>
					) : (
						<li>
							Navigate to this same page on your Bencher Self-Hosted account,
							Organization Billing
						</li>
					)}
					<li>Enter your license key in the "Self-Hosted License Key" box</li>
					<li>
						🎉 Lettuce turnip the beet! You now have a Bencher Plus Self-Hosted
						License!
					</li>
				</ol>
			</h4>
			<br />
			<PaymentMethod usage={props.usage} />
			<br />
			{manageSubscription}
		</div>
	);
};

const SelfHostedFreePanel = (props: {
	apiUrl: string;
	params: Params;
	onboard: boolean;
	bencher_valid: Resource<InitValid>;
	usage: Resource<null | JsonUsage>;
	refetch: () => void;
}) => {
	const [submitting, setSubmitting] = createSignal(false);
	const [license, setLicense] = createSignal<null | Jwt>(null);
	const [valid, setValid] = createSignal<null | boolean>(null);

	const isSendable = (): boolean => {
		return !submitting() && valid() === true;
	};

	const sendForm = () => {
		if (!props.bencher_valid()) {
			return;
		}
		const token = authUser()?.token;
		if (!validJwt(token)) {
			return;
		}
		if (!isSendable()) {
			return;
		}
		setSubmitting(true);
		const data = {
			license: license()?.trim(),
		};
		const path = `/v0/organizations/${props.params.organization}`;
		httpPatch(props.apiUrl, path, token, data)
			.then((_resp) => {
				setSubmitting(false);
				props.refetch();
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				Sentry.captureException(error);
				pageNotify(
					NotifyKind.ERROR,
					"Lettuce romaine calm! Failed to post license. Please, try again.",
				);
			});
	};

	return (
		<div class="content">
			<Show when={!props.onboard}>
				<h2 class="title is-2">Free Tier (Self-Hosted Unlicensed)</h2>
				<h3 class="subtitle is-3">
					{fmtDate(props.usage()?.start_time)} -{" "}
					{fmtDate(props.usage()?.end_time)}
				</h3>
				<h4>Metrics Used: {props.usage()?.metrics?.toLocaleString() ?? 0}</h4>
				<h4>
					Runner Minutes Used:{" "}
					{props.usage()?.runner_minutes?.toLocaleString() ?? 0}
				</h4>
				<br />
			</Show>
			<h2 class="title is-2">How to get a Bencher Self-Hosted License Key</h2>
			<h4>
				<ol>
					<li>
						<a href={BENCHER_CALENDLY_URL} target="_blank" rel="noreferrer">
							Contact us
						</a>{" "}
						to set up a Bencher Plus Enterprise (On-Prem) license
					</li>
					<li>
						Share your "Self-Hosted Organization UUID":{" "}
						<code style="word-break: break-word;">
							{/* biome-ignore lint/a11y/useValidAnchor: copy to clipboard */}
							<a
								title="Copy to clipboard"
								onMouseDown={(_) =>
									navigator.clipboard.writeText(
										props.usage()?.organization ?? "",
									)
								}
							>
								{props.usage()?.organization}
							</a>
						</code>
					</li>
					<li>We will provision your license and send you your license key</li>
					<li>
						Back on <i>this</i> server, enter your license key below ⬇️
					</li>
					<li>
						🎉 Lettuce turnip the beet! You now have a Bencher Plus License!
					</li>
				</ol>
			</h4>
			<div class={`columns ${props.onboard ? "is-centered" : ""}`}>
				<div class={`column ${props.onboard ? "" : "is-two-thirds"}`}>
					<Field
						kind={FieldKind.INPUT}
						fieldKey="license"
						label="Self-Hosted License Key"
						value={license()}
						valid={valid()}
						config={{
							label: "Self-Hosted License Key",
							type: "text",
							placeholder: "jwt_header.jwt_payload.jwt_verify_signature",
							icon: "fas fa-key",
							help: "Must be a valid JWT (JSON Web Token)",
							validate: validJwt,
						}}
						handleField={(_key, value, valid) => {
							setLicense(value as Jwt);
							setValid(valid);
						}}
					/>
					<div class="field">
						<p class="control">
							<button
								class="button is-primary is-fullwidth"
								type="submit"
								disabled={!isSendable()}
								onMouseDown={(e) => {
									e.preventDefault();
									sendForm();
								}}
							>
								Save
							</button>
						</p>
					</div>
				</div>
			</div>
			<Show when={props.onboard}>
				<div class="has-text-centered">
					<a class="button" href="/console">
						Continue Unlicensed
					</a>
				</div>
			</Show>
		</div>
	);
};

const SelfHostedLicensedPanel = (props: {
	usage: Resource<null | JsonUsage>;
}) => {
	return (
		<div class="content">
			<h2 class="title is-2">
				{planLevel(props.usage()?.license?.level)} Tier (Self-Hosted Licensed)
			</h2>
			<h3 class="subtitle is-3">
				{fmtDate(props.usage()?.start_time)} -{" "}
				{fmtDate(props.usage()?.end_time)}
			</h3>
			<h4>
				Entitlements:{" "}
				{props.usage()?.license?.entitlements?.toLocaleString() ?? 0}
			</h4>
			<h4>Metrics Used: {props.usage()?.metrics?.toLocaleString() ?? 0}</h4>
			<h4>
				Runner Minutes Used:{" "}
				{props.usage()?.runner_minutes?.toLocaleString() ?? 0}
			</h4>
			<h4>
				Metrics Remaining:{" "}
				{(
					(props.usage()?.license?.entitlements ?? 0) -
					(props.usage()?.metrics ?? 0)
				).toLocaleString()}
			</h4>
			<br />
			<h4>
				<a
					href={`/console/organizations/${
						props.usage()?.license?.organization
					}/settings#License%20Key`}
				>
					View/Update License Key in Organization Settings
				</a>
			</h4>
			<br />
			{manageSubscription}
		</div>
	);
};

export default BillingPanel;
