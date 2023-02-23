import {
	createMemo,
	createResource,
	createSignal,
	Match,
	Switch,
} from "solid-js";
import { Host } from "../../config/resources/billing";
import MeteredBilling from "./MeteredBilling";
import LicensedBilling from "./LicensedBilling";
import BillingHeader from "./BillingHeader";
import { BENCHER_API_URL, get_options, validate_jwt } from "../../../site/util";
import axios from "axios";
import Plan from "./Plan";

const BillingPanel = (props) => {
	const fetchPlan = async (plan_fetcher: {
		organization: string;
		refresh: number;
	}) => {
		const EMPTY_OBJECT = {};
		if (!plan_fetcher.organization) {
			return EMPTY_OBJECT;
		}
		const token = props.user?.token;
		if (!validate_jwt(props.user?.token)) {
			return EMPTY_OBJECT;
		}
		const url = `${BENCHER_API_URL()}/v0/organizations/${
			plan_fetcher?.organization
		}/plan`;
		return await axios(get_options(url, token))
			.then((resp) => {
				setNewBilling(false);
				return resp?.data;
			})
			.catch((_error) => {
				setNewBilling(true);
				return EMPTY_OBJECT;
			});
	};

	// Refresh plan query
	const [refresh, setRefresh] = createSignal(0);
	const handleRefresh = () => {
		setRefresh(refresh() + 1);
	};
	const plan_fetcher = createMemo(() => {
		return {
			organization: props.organization_slug(),
			refresh: refresh(),
		};
	});
	const [plan] = createResource(plan_fetcher, fetchPlan);
	const [newBilling, setNewBilling] = createSignal(false);

	return (
		<>
			<BillingHeader config={props.config?.header} />

			<Switch
				fallback={
					<section class="section">
						<div class="container">
							<div class="columns">
								<div class="column">
									<h4 class="title">Loading...</h4>
								</div>
							</div>
						</div>
					</section>
				}
			>
				<Match when={!plan()?.level && newBilling() === true}>
					<MeteredBilling
						user={props.user}
						organization_slug={props.organization_slug}
						handleRefresh={handleRefresh}
					/>
				</Match>
				<Match when={plan()?.level}>
					<Plan
						user={props.user}
						organization_slug={props.organization_slug}
						plan={plan}
					/>
				</Match>
				<Match when={props.host === Host.SELF_HOSTED}>
					<LicensedBilling />
				</Match>
			</Switch>
		</>
	);
};

export default BillingPanel;
