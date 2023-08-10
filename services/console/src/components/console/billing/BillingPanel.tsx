// import {
// 	createMemo,
// 	createResource,
// 	createSignal,
// 	Match,
// 	Switch,
// } from "solid-js";
// import { Host } from "../../config/resources/billing";
// import MeteredBilling from "./MeteredBilling";
// import LicensedBilling from "./LicensedBilling";
// import BillingHeader from "./BillingHeader";
// import {
// 	BENCHER_BILLING_API_URL,
// 	get_options,
// 	validate_jwt,
// } from "../../../site/util";
// import axios from "axios";
// import Plan from "./Plan";

import bencher_valid_init, { InitOutput } from "bencher_valid";
import { Match, Switch, createMemo, createResource } from "solid-js";
import consoleConfig from "../../../config/console";
import { Operation, Resource } from "../../../config/types";
import type { Params } from "astro";
import { Host } from "../../../config/organization/billing";
import { authUser } from "../../../util/auth";
import { BENCHER_BILLING_API_URL, isBencherCloud } from "../../../util/ext";
import BillingHeader, { BillingHeaderConfig } from "./BillingHeader";
import LicensedBilling from "./plan/LicensedBilling";
import type { JsonPlan } from "../../../types/bencher";
import { httpGet } from "../../../util/http";
import { validJwt } from "../../../util/valid";
import MeteredBilling from "./plan/MeteredBilling";
import Plan from "./plan/Plan";

interface Props {
	params: Params;
}

interface BillingPanelConfig {
	operation: Operation;
	header: BillingHeaderConfig;
	host: Host;
}

const BillingPanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const user = authUser();
	const host = createMemo(() =>
		isBencherCloud() ? Host.BENCHER_CLOUD : Host.SELF_HOSTED,
	);
	const config = createMemo<BillingPanelConfig>(
		() => consoleConfig[Resource.BILLING]?.[host()],
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
		if (!fetcher.bencher_valid) {
			return null;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		const url = `${BENCHER_BILLING_API_URL()}/v0/organizations/${
			fetcher.params.organization
		}/plan`;
		return await httpGet(url, fetcher.token)
			.then((resp) => {
				return resp?.data as JsonPlan;
			})
			.catch((_error) => {
				return null;
			});
	};
	const [plan, { refetch }] = createResource(fetcher, fetchPlan);

	return (
		<>
			<BillingHeader config={config()?.header} />

			<Switch
				fallback={
					<section class="section">
						<div class="container">
							<h4 class="title">Loading...</h4>
						</div>
					</section>
				}
			>
				<Match when={config()?.host === Host.SELF_HOSTED}>
					<LicensedBilling />
				</Match>
				<Match when={config()?.host === Host.BENCHER_CLOUD && plan() === null}>
					<MeteredBilling
						params={props.params}
						bencher_valid={bencher_valid}
						user={user}
						handleRefresh={refetch}
					/>
				</Match>
				<Match when={config()?.host === Host.BENCHER_CLOUD && plan()}>
					<Plan
						params={props.params}
						bencher_valid={bencher_valid}
						user={user}
						plan={plan}
					/>
				</Match>
			</Switch>
		</>
	);
};

export default BillingPanel;
