import type { Params } from "astro";
import bencher_valid_init, { type InitOutput } from "bencher_valid";
import BillingHeader from "../BillingHeader";
import {
	createResource,
	type Accessor,
	createMemo,
	Show,
	type Resource,
} from "solid-js";
import type { BillingPanelConfig } from "../BillingPanel";
import { authUser } from "../../../../util/auth";
import { validJwt } from "../../../../util/valid";
import { httpGet } from "../../../../util/http";
import { UsageKind, type JsonUsage } from "../../../../types/bencher";

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
				return resp?.data;
			})
			.catch((_error) => {
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
								when={usage?.kind === UsageKind.SelfHostedLicensed}
								fallback={<FreePanel usage={usage} />}
							>
								<p>TODO licensed</p>
							</Show>
						</div>
					</div>
				</div>
			</section>
		</>
	);
};

const FreePanel = (props: { usage: Resource<null | JsonUsage> }) => {
	return (
		<>
			<h4 class="title">How to get a Bencher Plus License</h4>
			<br />
			<h4 class="subtitle" style="margin-bottom: 3rem;">
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
						Enter your desired number of metrics for the <i>year</i>
					</li>
					<li>
						Enter your "Self-Hosted Organization UUID":{" "}
						<code style="overflow-wrap:anywhere;">
							{props.usage()?.organization}
						</code>
					</li>
					<li>Enter your billing information</li>
					<li>Copy the license key that is generated</li>
					<li>
						Back on <i>this</i> server,{" "}
						<a
							href={`/console/organizations/${
								props.usage()?.organization
							}/settings`}
						>
							navigate to your Organization Settings
						</a>
					</li>
					<li>
						Find the field named "License Key", click "Update", paste your
						license key, and hit "Save"
					</li>
					<li>
						🎉 Lettuce turnip the beet! You now have a Bencher Plus License!
					</li>
				</ol>
			</h4>
		</>
	);
};

export default SelfHostedPanel;
