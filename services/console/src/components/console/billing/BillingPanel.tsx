import type { Params } from "astro";
import bencher_valid_init, { type InitOutput } from "bencher_valid";
import {
	Match,
	type Resource,
	Switch,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import consoleConfig from "../../../config/console";
import { Host } from "../../../config/organization/billing";
import { BencherResource } from "../../../config/types";
import { type JsonUsage, type Jwt, UsageKind } from "../../../types/bencher";
import { authUser } from "../../../util/auth";
import {
	fmtDate,
	fmtUsd,
	planLevel,
	planLevelPrice,
	suggestedMetrics,
} from "../../../util/convert";
import { isBencherCloud } from "../../../util/ext";
import { httpGet, httpPatch } from "../../../util/http";
import { NotifyKind, pageNotify } from "../../../util/notify";
import { validJwt } from "../../../util/valid";
import Field from "../../field/Field";
import FieldKind from "../../field/kind";
import type { BillingHeaderConfig } from "./BillingHeader";
import BillingHeader from "./BillingHeader";
import BillingForm from "./plan/BillingForm";
import PaymentMethod from "./plan/PaymentMethod";

interface Props {
	apiUrl: string;
	params: Params;
}

export interface BillingPanelConfig {
	header: BillingHeaderConfig;
	host: Host;
}

const BillingPanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const user = authUser();
	const config = createMemo<BillingPanelConfig>(
		() =>
			consoleConfig[BencherResource.BILLING]?.[
				isBencherCloud() ? Host.BENCHER_CLOUD : Host.SELF_HOSTED
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
		bencher_valid: InitOutput;
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
			.catch((error) => {
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
		<>
			<BillingHeader config={config()?.header} />

			<section class="section">
				<div class="container">
					<div class="columns">
						<div class="column">
							<Switch>
								{/* Bencher Cloud */}
								<Match when={usage()?.kind === UsageKind.CloudFree}>
									<BillingForm
										apiUrl={props.apiUrl}
										params={props.params}
										bencher_valid={bencher_valid}
										user={user}
										usage={usage}
										handleRefresh={refetch}
									/>
								</Match>
								<Match when={usage()?.kind === UsageKind.CloudMetered}>
									<CloudMeteredPanel usage={usage} />
								</Match>
								<Match when={usage()?.kind === UsageKind.CloudLicensed}>
									<CloudLicensedPanel usage={usage} />
								</Match>
								<Match
									when={usage()?.kind === UsageKind.CloudSelfHostedLicensed}
								>
									<CloudSelfHostedLicensedPanel usage={usage} />
								</Match>
								{/* Self-Hosted */}
								<Match when={usage()?.kind === UsageKind.SelfHostedFree}>
									<SelfHostedFreePanel
										apiUrl={props.apiUrl}
										params={props.params}
										bencher_valid={bencher_valid}
										usage={usage}
										refetch={refetch}
									/>
								</Match>
								<Match when={usage()?.kind === UsageKind.SelfHostedLicensed}>
									<SelfHostedLicensedPanel usage={usage} />
								</Match>
							</Switch>
						</div>
					</div>
				</div>
			</section>
		</>
	);
};

const CloudMeteredPanel = (props: {
	usage: Resource<null | JsonUsage>;
}) => {
	const price = createMemo(() => planLevelPrice(props.usage()?.plan?.level));
	const estCost = createMemo(() => (props.usage()?.usage ?? 0) * price());

	return (
		<div class="content">
			<h2 class="title is-2">
				{planLevel(props.usage()?.plan?.level)} Tier (Bencher Cloud Metered)
			</h2>
			<h3 class="subtitle is-3">
				{fmtDate(props.usage()?.start_time)} -{" "}
				{fmtDate(props.usage()?.end_time)}
			</h3>
			<h4>Cost per Metric: {fmtUsd(price())}</h4>
			<h4>
				Estimated Metrics Used: {props.usage()?.usage?.toLocaleString() ?? 0}
			</h4>
			<h4>Current Estimated Cost: {fmtUsd(estCost())}</h4>
			<br />
			<PaymentMethod usage={props.usage} />
			<br />
			<p>
				To update or cancel your subscription please email{" "}
				<a href="mailto:everett@bencher.dev">everett@bencher.dev</a>
			</p>
		</div>
	);
};

const CloudLicensedPanel = (props: {
	usage: Resource<null | JsonUsage>;
}) => {
	return (
		<div class="content">
			<h2 class="title is-2">
				{planLevel(props.usage()?.license?.level)} Tier (Bencher Cloud Licensed)
			</h2>
			<h3 class="subtitle is-3">
				{fmtDate(props.usage()?.start_time)} -{" "}
				{fmtDate(props.usage()?.end_time)}
			</h3>
			<h4>
				Entitlements:{" "}
				{props.usage()?.license?.entitlements?.toLocaleString() ?? 0}
			</h4>
			<h4>Metrics Used: {props.usage()?.usage?.toLocaleString() ?? 0}</h4>
			<h4>
				Metrics Remaining:{" "}
				{(
					(props.usage()?.license?.entitlements ?? 0) -
					(props.usage()?.usage ?? 0)
				).toLocaleString()}
			</h4>
			<br />
			<PaymentMethod usage={props.usage} />
			<br />
			<p>
				To update or cancel your subscription please email{" "}
				<a href="mailto:everett@bencher.dev">everett@bencher.dev</a>
			</p>
		</div>
	);
};

const CloudSelfHostedLicensedPanel = (props: {
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
				<a
					title="Copy to clipboard"
					onClick={(_) =>
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
						<a
							title="Copy to clipboard"
							onClick={(_) =>
								navigator.clipboard.writeText(props.usage()?.license?.key ?? "")
							}
						>
							Click here to copy your Self-Hosted license key
						</a>
					</li>
					<li>
						Navigate to this same page on your Bencher Self-Hosted account,
						Organization Billing
					</li>
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
			<p>
				To update or cancel your subscription please email{" "}
				<a href="mailto:everett@bencher.dev">everett@bencher.dev</a>
			</p>
		</div>
	);
};

const SelfHostedFreePanel = (props: {
	apiUrl: string;
	params: Params;
	bencher_valid: Resource<InitOutput>;
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
				pageNotify(
					NotifyKind.ERROR,
					`Lettuce romaine calm! Failed to post license. Please, try again.`,
				);
			});
	};

	return (
		<div class="content">
			<h2 class="title is-2">Free Tier (Self-Hosted Unlicensed)</h2>
			<h3 class="subtitle is-3">
				{fmtDate(props.usage()?.start_time)} -{" "}
				{fmtDate(props.usage()?.end_time)}
			</h3>
			<h4>Metrics Used: {props.usage()?.usage?.toLocaleString() ?? 0}</h4>
			<br />
			<h2 class="title is-2">How to get a Bencher Self-Hosted License Key</h2>
			<h4>
				<ol>
					<li>
						Create an account on{" "}
						<a href="https://bencher.dev" target="_blank">
							Bencher Cloud
						</a>{" "}
						if you don't already have one
					</li>
					<li>
						Navigate to this same page on your Bencher Cloud account,
						Organization Billing
					</li>
					<li>Select either the "Team" or "Enterprise" plan</li>
					<li>Select "Self-Hosted License"</li>
					<li>
						Enter your desired number of metrics for the <i>year</i> (Suggested:{" "}
						{suggestedMetrics(props.usage()?.usage).toLocaleString()})
					</li>
					<li>
						Enter your "Self-Hosted Organization UUID":{" "}
						<code style="word-break: break-word;">
							<a
								title="Copy to clipboard"
								onClick={(_) =>
									navigator.clipboard.writeText(
										props.usage()?.organization ?? "",
									)
								}
							>
								{props.usage()?.organization}
							</a>
						</code>
					</li>
					<li>Click "Subscribe" and enter your billing information</li>
					<li>Copy the Self-Hosted license key that is generated</li>
					<li>
						Back on <i>this</i> server, enter your license key below ⬇️
					</li>
					<li>
						🎉 Lettuce turnip the beet! You now have a Bencher Plus License!
					</li>
				</ol>
			</h4>
			<div class="columns">
				<div class="column is-two-thirds">
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
								onClick={(e) => {
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
			<h4>Metrics Used: {props.usage()?.usage?.toLocaleString() ?? 0}</h4>
			<h4>
				Metrics Remaining:{" "}
				{(
					(props.usage()?.license?.entitlements ?? 0) -
					(props.usage()?.usage ?? 0)
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
			<p>
				To update or cancel your subscription please email{" "}
				<a href="mailto:everett@bencher.dev">everett@bencher.dev</a>
			</p>
		</div>
	);
};

export default BillingPanel;
