import type { Params } from "astro";
import bencher_valid_init, { type InitOutput } from "bencher_valid";
import BillingHeader from "../BillingHeader";
import {
	createResource,
	type Accessor,
	createMemo,
	Show,
	type Resource,
	createSignal,
} from "solid-js";
import type { BillingPanelConfig } from "../BillingPanel";
import { authUser } from "../../../../util/auth";
import { validJwt } from "../../../../util/valid";
import { httpGet, httpPatch } from "../../../../util/http";
import { UsageKind, type JsonUsage, type Jwt } from "../../../../types/bencher";
import { NotifyKind, pageNotify } from "../../../../util/notify";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { fmtDate, planLevel, suggestedMetrics } from "../../../../util/convert";

interface Props {
	apiUrl: string;
	params: Params;
	config: Accessor<BillingPanelConfig>;
}

const SelfHostedPanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const user = authUser();

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
		if (!fetcher.bencher_valid) {
			return null;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		const path = `/v0/organizations/${fetcher.params.organization}/usage`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				console.log(resp.data);
				return resp?.data;
			})
			.catch((error) => {
				console.log(error);
				return null;
			});
	};
	const [usage, { refetch }] = createResource<null | JsonUsage>(
		fetcher,
		fetchPlan,
	);

	return (
		<>
			<BillingHeader config={props.config()?.header} />
			<section class="section">
				<div class="container">
					<div class="columns">
						<div class="column">
							<Show
								when={usage()?.kind === UsageKind.SelfHostedLicensed}
								fallback={
									<FreePanel
										apiUrl={props.apiUrl}
										params={props.params}
										bencher_valid={bencher_valid}
										usage={usage}
										refetch={refetch}
									/>
								}
							>
								<LicensedPanel usage={usage} />
							</Show>
						</div>
					</div>
				</div>
			</section>
		</>
	);
};

const FreePanel = (props: {
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
			<h2 class="title">Free Tier Usage</h2>
			<h3 class="subtitle">
				{fmtDate(props.usage()?.start_time)} -{" "}
				{fmtDate(props.usage()?.end_time)}
			</h3>
			<h4>Metrics Used: {props.usage()?.usage?.toLocaleString() ?? 0}</h4>
			<br />
			<h2 class="title">How to get a Bencher Plus License</h2>
			<h4>
				<ol>
					<li>
						Create an account on{" "}
						<a href="https://bencher.dev" target="_blank">
							Bencher Cloud
						</a>{" "}
						if you don't have one already
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
						<code style="overflow-wrap:anywhere;">
							{props.usage()?.organization}
						</code>
					</li>
					<li>Enter your billing information</li>
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
						label="Self-Hosted License"
						value={license()}
						valid={valid()}
						config={{
							label: "Self-Hosted License",
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

const LicensedPanel = (props: {
	usage: Resource<null | JsonUsage>;
}) => {
	return (
		<div class="content">
			<h2 class="title">
				{planLevel(props.usage()?.license?.level)} Tier Usage
			</h2>
			<h3 class="subtitle">
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
		</div>
	);
};

export default SelfHostedPanel;
