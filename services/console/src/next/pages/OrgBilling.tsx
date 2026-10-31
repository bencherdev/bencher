import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import Table from "@bencherdev/ui/Table";
import TextInput from "@bencherdev/ui/TextInput";
import Truncate from "@bencherdev/ui/Truncate";
import { useLocation, useNavigate, useParams } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import {
	For,
	type JSX,
	Match,
	Show,
	Switch,
	createSignal,
	onMount,
} from "solid-js";
import type {
	JsonCardDetails,
	JsonCheckout,
	JsonLicense,
	JsonPlan,
	JsonPlanPortal,
} from "../../types/bencher";
import { useAccount } from "../account/context";
import { ApiError } from "../api";
import {
	BARE_METAL_RATE,
	away,
	bandIndex,
	bands,
	bareMetalUsd,
	cardExpiry,
	cardName,
	estimate,
	fullDay,
	manageQuery,
	type Plan,
	period,
	shortDay,
	type Usage,
	usageQuery,
	usd,
	withoutPayer,
} from "../organization/billing";
import type { Enrollment } from "../organization/checkout";
import { consoleUserQuery } from "../queries";
import { useQueryResult } from "../query";
import {
	Frame,
	failureOf,
	organizationIn,
	organizationQuery,
} from "./OrgGeneral";
import { formatCount } from "../plot/count";

const CONTACT = "mailto:enterprise@bencher.dev";
const PAYMENT_REQUIRED = 402;

/** The organization's plan, usage, and payment method, or its license when self-hosted. */
const OrgBilling = () => {
	const { api, reader } = useAccount();
	const params = useParams();
	const key = () => params.organization ?? "";
	const organization = useQueryResult(() => organizationQuery(api, key()));
	const manage = useQueryResult(() => manageQuery(api, key()));
	const { enrollment, recorded } = useCheckoutReturn(key);
	// The API shows usage only to a reader who manages the organization, and Stripe's return reads it once the plan is recorded.
	const usage = useQueryResult(() => ({
		...usageQuery(api, key()),
		enabled:
			manage().data !== false &&
			(enrollment()?.status !== "enrolling" || recorded()),
	}));
	// Only read: an organization the reader has seen names the rail at once.
	const bootstrap = useQueryResult(() => ({
		...consoleUserQuery(api, reader().user.uuid),
		enabled: false,
	}));
	const name = () =>
		organization().data?.name ?? organizationIn(bootstrap().data, key())?.name;
	// An expired or invalid license answers 402, and its key still needs replacing.
	const unlicensed = () => {
		const error = usage().error;
		return error instanceof ApiError && error.status === PAYMENT_REQUIRED;
	};
	return (
		<Frame organization={key()} name={name()} section="billing">
			<div class="sethead">
				<div class="ph-title">
					<div class="crumb-line">
						Settings<span aria-hidden="true">/</span>
					</div>
					<Heading level={1} size="xl">
						Billing
					</Heading>
				</div>
			</div>
			<Show when={enrollment()?.status === "enrolled"}>
				<Banner status="success" role="status">
					Enrolled in {levelName(enrollment()?.level ?? "")}
				</Banner>
			</Show>
			<Show when={enrollment()?.status === "failed"}>
				<Banner status="error" role="alert">
					{failureOf(
						enrollment()?.error,
						`Bencher did not record the ${levelName(enrollment()?.level ?? "")} plan`,
					)}
				</Banner>
			</Show>
			<Switch fallback={<Skeleton size="card" />}>
				<Match when={enrollment()?.status === "enrolling"}>
					<Skeleton size="card" />
				</Match>
				<Match when={manage().data === false}>{null}</Match>
				<Match when={manage().data && usage().data}>
					{(shown) => <UsageView usage={shown()} name={name() ?? key()} />}
				</Match>
				<Match when={manage().data && unlicensed() && organization().data}>
					{(shown) => (
						<LicenseKey
							uuid={shown().uuid}
							current={shown().license ?? ""}
							editable
						/>
					)}
				</Match>
				<Match when={manage().error ?? (manage().data && usage().error)}>
					<Banner status="error" role="alert" class="load-error">
						<span class="grow">Billing did not load</span>
						<Button
							size="sm"
							onClick={() => {
								manage().refetch();
								usage().refetch();
							}}
						>
							Retry
						</Button>
					</Banner>
				</Match>
			</Switch>
		</Frame>
	);
};

export default OrgBilling;

/** Stripe's return: its handler loads only with one, records the plan once, and says how that went. */
const useCheckoutReturn = (organization: () => string) => {
	const { api } = useAccount();
	const client = useQueryClient();
	const location = useLocation();
	const navigate = useNavigate();
	const search = location.search;
	const level = new URLSearchParams(search).get("level") ?? "";
	const [enrollment, setEnrollment] = createSignal<Enrollment | undefined>(
		new URLSearchParams(search).has("checkout")
			? { status: "enrolling", level }
			: undefined,
	);
	const [recorded, setRecorded] = createSignal(false);
	onMount(() => {
		if (!enrollment()) {
			return;
		}
		// Off the link first, so a reload does not record the plan again; relative to the router's base.
		navigate(`/organizations/${organization()}/billing`, { replace: true });
		// Reading only its export spares the bundler a namespace object, whose helper it would load from an unrelated chunk.
		import("../organization/checkout")
			.then(({ recordCheckout }) =>
				recordCheckout(
					api,
					organization(),
					search,
					() => {
						// The page's read that this starts and the read below are one request.
						setRecorded(true);
						// A read that fails leaves its error in the cache for the page to show.
						return client.fetchQuery(usageQuery(api, organization())).then(
							() => undefined,
							() => undefined,
						);
					},
					setEnrollment,
				),
			)
			.catch((error) => setEnrollment({ status: "failed", level, error }));
	});
	return { enrollment, recorded };
};

const UsageView = (props: { usage: Usage; name: string }) => {
	// Compared as text, so the generated enums stay out of the page.
	const kind = (): string => props.usage.kind;
	return (
		<Switch>
			<Match when={kind() === "cloud_free"}>
				<Plans organization={props.usage.organization} />
				<ThisMonth usage={props.usage} />
			</Match>
			<Match when={kind() === "cloud_metered" && props.usage.plan}>
				{(plan) => <Paid usage={props.usage} plan={plan()} name={props.name} />}
			</Match>
			<Match when={props.usage.license}>
				{(license) => (
					<>
						<License usage={props.usage} license={license()} />
						<Show when={props.usage.plan}>
							{(plan) => (
								<Payment
									organization={props.usage.organization}
									card={plan().card}
								/>
							)}
						</Show>
						<LicenseKey
							uuid={license().organization}
							current={license().key}
							editable={kind() === "self_hosted_licensed"}
						/>
					</>
				)}
			</Match>
			<Match when={kind() === "self_hosted_free"}>
				<ThisMonth usage={props.usage} />
				<LicenseKey uuid={props.usage.organization} current="" editable />
			</Match>
		</Switch>
	);
};

const levelName = (level: string) =>
	`${level.charAt(0).toUpperCase()}${level.slice(1)}`;

const Stat = (props: { label: string; children: JSX.Element }) => (
	<div class="field">
		<span class="eyebrow">{props.label}</span>
		{props.children}
	</div>
);

const Meter = (props: { used: number; of: number; unit: string }) => (
	<div
		class="meter"
		role="img"
		aria-label={`${formatCount(props.used)} of ${formatCount(props.of)} ${props.unit}`}
	>
		<span
			style={{ width: `${Math.min(100, (props.used / props.of) * 100)}%` }}
		/>
	</div>
);

const Paid = (props: { usage: Usage; plan: Plan; name: string }) => {
	const { api } = useAccount();
	const client = useQueryClient();
	const params = useParams();
	const level = () => levelName(props.plan.level);
	// Pro bills on active series; the retiring plans bill on metrics.
	const seriesBilled = () => props.usage.active_series != null;
	const used = () =>
		(seriesBilled() ? props.usage.active_series : props.usage.metrics) ?? 0;
	const minutes = () => props.usage.runner_minutes ?? 0;
	const tiers = () => props.plan.tiers ?? [];
	const included = () => tiers()[0]?.up_to;
	const bill = () => estimate(props.plan, used(), minutes());
	const end = () => props.plan.current_period_end;
	const [confirming, setConfirming] = createSignal(false);
	const [busy, setBusy] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	let heading: HTMLHeadingElement | undefined;

	/** Schedule or clear the plan's end with this period. */
	const schedule = async (cancel: boolean) => {
		setBusy(true);
		setFailure(undefined);
		try {
			const { data } = await api.send<JsonPlan>(
				"PATCH",
				`/v0/organizations/${props.usage.organization}/plan`,
				{ cancel_at_period_end: cancel },
			);
			client.setQueryData<Usage>(
				usageQuery(api, params.organization ?? "").queryKey,
				(usage) => usage && { ...usage, plan: withoutPayer(data) },
			);
			setConfirming(false);
			// The control that held focus is gone.
			heading?.focus();
		} catch (error) {
			setFailure(
				failureOf(
					error,
					cancel
						? "Bencher did not cancel the plan"
						: "Bencher did not resume the plan",
				),
			);
		} finally {
			setBusy(false);
		}
	};

	const close = () => {
		setConfirming(false);
		setFailure(undefined);
	};

	return (
		<>
			<section class="setcard" aria-labelledby="sec-plan">
				<div class="setcard-head">
					<h2
						class="seclabel"
						id="sec-plan"
						tabindex="-1"
						ref={(element) => {
							heading = element;
						}}
					>
						{level()} Plan
					</h2>
					<span class="spacer" />
					<Show
						when={props.plan.cancel_at_period_end}
						fallback={
							<span class="muted sm">
								{period(props.plan.current_period_start, end())}
							</span>
						}
					>
						<span class="toolbar">
							<span class="muted sm">Ends {fullDay(end())}</span>
							<Button
								size="sm"
								disabled={busy()}
								onClick={() => schedule(false)}
							>
								Resume plan
							</Button>
						</span>
					</Show>
				</div>
				<div class="setcard-body">
					<Show when={!confirming() && failure()}>
						{(message) => (
							<p class="ferror" role="alert">
								<Truncate text={message()} reveal />
							</p>
						)}
					</Show>
					<div class="stats">
						<Stat label={seriesBilled() ? "Active series" : "Metrics"}>
							<span class="figure">
								{formatCount(used())}
								<Show when={included()}>
									{(of) => <small class="muted"> of {formatCount(of())}</small>}
								</Show>
							</span>
							<Show when={included()}>
								{(of) => <Meter used={used()} of={of()} unit="active series" />}
							</Show>
						</Stat>
						<Stat label="Bare metal">
							<span class="figure">
								{formatCount(minutes())} <small class="muted">min</small>
							</span>
							<span class="muted sm">
								{usd(bareMetalUsd(minutes()))} at {BARE_METAL_RATE}
							</span>
						</Stat>
						<Stat label="Estimated bill">
							<span class="figure">
								{bill() === undefined ? (
									<a class="lnk" href={CONTACT}>
										Get in touch
									</a>
								) : (
									usd(bill() ?? 0)
								)}
							</span>
							<span class="muted sm">Charged {shortDay(end())}</span>
						</Stat>
					</div>
					<Show when={tiers().length}>
						<Table aria-label={`${level()} price bands`}>
							<thead>
								<tr>
									<th scope="col">Active series</th>
									<th scope="col">Price</th>
									<th scope="col">
										<span class="sr-only">Now</span>
									</th>
								</tr>
							</thead>
							<tbody>
								<For each={bands(tiers())}>
									{(band, index) => {
										const now = () => index() === bandIndex(tiers(), used());
										return (
											<tr>
												<td>{now() ? <b>{band.range}</b> : band.range}</td>
												<td>
													{band.price ?? (
														<a class="lnk" href={CONTACT}>
															Get in touch
														</a>
													)}
												</td>
												<td class="act">
													<Show when={now()}>
														<span class="newtag">This period</span>
													</Show>
												</td>
											</tr>
										);
									}}
								</For>
							</tbody>
						</Table>
					</Show>
				</div>
			</section>
			<Payment organization={props.usage.organization} card={props.plan.card} />
			<Show when={!props.plan.cancel_at_period_end}>
				<section class="setcard danger" aria-labelledby="sec-danger">
					<div class="setcard-head">
						<h2 class="seclabel danger" id="sec-danger">
							Danger
						</h2>
					</div>
					<div class="setcard-body">
						<div class="danger-row">
							<div class="grow">
								<h3>Cancel plan</h3>
								<p id="dz-cancel" class="muted">
									{level()} ends with this period on {fullDay(end())}, and{" "}
									{props.name} moves to the Free plan.
								</p>
							</div>
							<Button
								variant="danger"
								aria-haspopup="dialog"
								aria-describedby="dz-cancel"
								onClick={() => setConfirming(true)}
							>
								Cancel plan
							</Button>
						</div>
					</div>
					<Show when={confirming()}>
						<Dialog
							role="alertdialog"
							aria-labelledby="cx-title"
							aria-describedby="cx-desc"
							onDismiss={close}
						>
							<div class="ui-dialog-head">
								<h2 id="cx-title">Cancel {level()}?</h2>
								<Button
									variant="ghost"
									iconOnly
									aria-label="Close"
									onClick={close}
								>
									<Icon name="close" />
								</Button>
							</div>
							<div class="ui-dialog-body">
								<p id="cx-desc" class="text2">
									Private projects lock on {shortDay(end())}.
								</p>
								<Show when={failure()}>
									{(message) => (
										<p class="ferror" role="alert">
											<Truncate text={message()} reveal />
										</p>
									)}
								</Show>
							</div>
							<div class="ui-dialog-foot">
								<Button onClick={close}>Keep {level()}</Button>
								<Button
									variant="destructive"
									disabled={busy()}
									onClick={() => schedule(true)}
								>
									Cancel {level()}
								</Button>
							</div>
						</Dialog>
					</Show>
				</section>
			</Show>
		</>
	);
};

const Payment = (props: { organization: string; card: JsonCardDetails }) => {
	const { api } = useAccount();
	const [opening, setOpening] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	const update = async () => {
		setOpening(true);
		setFailure(undefined);
		try {
			const { data } = await api.send<JsonPlanPortal>(
				"POST",
				`/v0/organizations/${props.organization}/plan/portal`,
			);
			away.to(data.url);
		} catch (error) {
			setFailure(failureOf(error, "Bencher did not open the billing portal"));
			setOpening(false);
		}
	};
	return (
		<section class="setcard" aria-labelledby="sec-pay">
			<div class="setcard-head">
				<h2 class="seclabel" id="sec-pay">
					Payment method
				</h2>
			</div>
			<div class="setcard-body">
				<div class="toolbar">
					<div>
						<b>{cardName(props.card)}</b>
						<div class="muted sm">{cardExpiry(props.card)}</div>
					</div>
					<span class="spacer" />
					<Button disabled={opening()} onClick={update}>
						Update card
					</Button>
				</div>
				<Show when={failure()}>
					{(message) => (
						<p class="ferror" role="alert">
							<Truncate text={message()} reveal />
						</p>
					)}
				</Show>
			</div>
		</section>
	);
};

/** The plans an organization on Free can move to. */
const Plans = (props: { organization: string }) => {
	const { api } = useAccount();
	const client = useQueryClient();
	const [starting, setStarting] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	const start = async () => {
		setStarting(true);
		setFailure(undefined);
		try {
			const { data } = await api.send<JsonCheckout>("POST", "/v0/checkout", {
				organization: props.organization,
				level: "pro",
			});
			away.to(data.url);
		} catch (error) {
			setFailure(failureOf(error, "Bencher did not start the checkout"));
			setStarting(false);
		}
	};
	// The pricing page's Pro leads here to start the checkout, once.
	const location = useLocation();
	const navigate = useNavigate();
	const params = useParams();
	onMount(async () => {
		if (new URLSearchParams(location.search).get("plan") !== "pro") {
			return;
		}
		// Relative to the router's base.
		navigate(`/organizations/${params.organization}/billing`, {
			replace: true,
		});
		// The cached plan may predate one bought elsewhere, so only this visit's read starts it.
		const fresh = await client
			.fetchQuery(usageQuery(api, params.organization ?? ""))
			.catch(() => undefined);
		const kind: string | undefined = fresh?.kind;
		if (kind === "cloud_free") {
			start();
		}
	});
	const card = (
		id: string,
		name: string,
		price: JSX.Element,
		features: string[],
		action?: JSX.Element,
	) => (
		<article class="setcard setcard-body" aria-labelledby={`plan-${id}`}>
			<div class="toolbar">
				<Heading level={3} id={`plan-${id}`}>
					{name}
				</Heading>
				<Show when={id === "free"}>
					<span class="spacer" />
					<span class="newtag">Current</span>
				</Show>
			</div>
			<span class="figure">{price}</span>
			<ul class="spacer text2 sm">
				<For each={features}>{(feature) => <li>{feature}</li>}</For>
			</ul>
			{action}
		</article>
	);
	return (
		<section class="setbody tight" aria-labelledby="sec-plans">
			<h2 class="seclabel" id="sec-plans">
				Plan
			</h2>
			<div class="stats">
				{card(
					"free",
					"Free",
					<>
						$0 <small class="muted">a month</small>
					</>,
					["Public projects", "1 bare metal job, 5 min"],
				)}
				{card(
					"pro",
					"Pro",
					<>
						$100 <small class="muted">a month</small>
					</>,
					[
						"250 series",
						"Public and private projects",
						`Bare metal ${BARE_METAL_RATE}`,
					],
					<Button variant="primary" disabled={starting()} onClick={start}>
						Start 1-month free trial
					</Button>,
				)}
				{card(
					"enterprise",
					"Enterprise",
					"Custom",
					["Single sign-on", "Custom bare metal runners"],
					<a class="ui-button" data-variant="secondary" href={CONTACT}>
						Contact us
					</a>,
				)}
			</div>
			<Show when={failure()}>
				{(message) => (
					<p class="ferror" role="alert">
						{message()}
					</p>
				)}
			</Show>
		</section>
	);
};

const ThisMonth = (props: { usage: Usage }) => (
	<section class="setcard" aria-labelledby="sec-month">
		<div class="setcard-head">
			<h2 class="seclabel" id="sec-month">
				This month
			</h2>
			<span class="spacer" />
			<span class="muted sm">
				{period(props.usage.start_time, props.usage.end_time)}
			</span>
		</div>
		<div class="setcard-body">
			<div class="stats">
				<Stat label="Metrics">
					<span class="figure">{formatCount(props.usage.metrics ?? 0)}</span>
				</Stat>
				<Stat label="Bare metal">
					<span class="figure">
						{formatCount(props.usage.runner_minutes ?? 0)}{" "}
						<small class="muted">min</small>
					</span>
				</Stat>
			</div>
		</div>
	</section>
);

const License = (props: { usage: Usage; license: JsonLicense }) => (
	<section class="setcard" aria-labelledby="sec-lic">
		<div class="setcard-head">
			<h2 class="seclabel" id="sec-lic">
				{levelName(props.license.level)} License
			</h2>
			<span class="spacer" />
			<span class="muted sm">
				{period(props.license.issued_at, props.license.expiration)}
			</span>
		</div>
		<Show when={props.usage.metrics != null}>
			<div class="setcard-body">
				<div class="stats">
					<Stat label="Metrics this term">
						<span class="figure">
							{formatCount(props.usage.metrics ?? 0)}
							<small class="muted">
								{" "}
								of {formatCount(props.license.entitlements)}
							</small>
						</span>
						<Meter
							used={props.usage.metrics ?? 0}
							of={props.license.entitlements}
							unit="metrics"
						/>
					</Stat>
					<Stat label="Bare metal">
						<span class="figure">
							{formatCount(props.usage.runner_minutes ?? 0)}{" "}
							<small class="muted">min</small>
						</span>
					</Stat>
				</div>
			</div>
		</Show>
	</section>
);

/** The organization's UUID, which a license is issued for, then the key itself. */
const LicenseKey = (props: {
	/** The organization the license is issued for. */
	uuid: string;
	current: string;
	editable: boolean;
}) => {
	const { api } = useAccount();
	const client = useQueryClient();
	const params = useParams();
	const current = () => props.current;
	const licensed = () => current() !== "";
	const [draft, setDraft] = createSignal(current());
	const [saving, setSaving] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	const changed = () => draft().trim() !== "" && draft().trim() !== current();
	const save = async () => {
		setSaving(true);
		setFailure(undefined);
		try {
			await api.send("PATCH", `/v0/organizations/${props.uuid}`, {
				license: draft().trim(),
			});
			await client.invalidateQueries({
				queryKey: usageQuery(api, params.organization ?? "").queryKey,
			});
		} catch (error) {
			setFailure(failureOf(error, "Bencher did not save the license key"));
		} finally {
			setSaving(false);
		}
	};
	const uuid = () => props.uuid;
	const [copied, setCopied] = createSignal(false);
	return (
		<section class="setcard" aria-labelledby="sec-key">
			<div class="setcard-head">
				<h2 class="seclabel" id="sec-key">
					{licensed() ? "License key" : "License"}
				</h2>
				<Show when={!licensed()}>
					<span class="spacer" />
					<a
						class="ui-button"
						data-variant="secondary"
						data-size="sm"
						href={CONTACT}
					>
						Contact us
					</a>
				</Show>
			</div>
			<div class="formrows">
				<div class="formrow">
					<div class="fl">
						<span>Organization UUID</span>
					</div>
					<div class="fvalue">
						<span class="mono">{uuid()}</span>
						<Button
							size="sm"
							aria-label={
								copied()
									? "Organization UUID copied"
									: "Copy the organization UUID"
							}
							onClick={() =>
								navigator.clipboard.writeText(uuid()).then(
									() => setCopied(true),
									() => setCopied(false),
								)
							}
						>
							{copied() ? "Copied" : "Copy"}
						</Button>
					</div>
				</div>
				<div class="formrow">
					<div class="fl">
						<label for="f-license">License key</label>
					</div>
					<div class="field">
						<div class="toolbar">
							<TextInput
								id="f-license"
								class="mono spacer"
								value={draft()}
								onInput={(event) => {
									setDraft(event.currentTarget.value);
									setFailure(undefined);
								}}
								readOnly={!props.editable}
								autocomplete="off"
								spellcheck={false}
							/>
							<Show when={props.editable}>
								<Button
									variant={licensed() ? "secondary" : "primary"}
									disabled={saving() || !changed()}
									onClick={save}
								>
									{licensed() ? "Replace key" : "Save key"}
								</Button>
							</Show>
						</div>
						<Show when={failure()}>
							{(message) => (
								<p class="ferror" role="alert">
									{message()}
								</p>
							)}
						</Show>
					</div>
				</div>
			</div>
		</section>
	);
};
