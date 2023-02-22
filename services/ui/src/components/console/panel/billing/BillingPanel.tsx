import { createResource, Match, Switch } from "solid-js";
import { Host } from "../../config/resources/billing";
import MeteredBilling from "./MeteredBilling";
import LicensedBilling from "./LicensedBilling";
import BillingHeader from "./BillingHeader";
import { BENCHER_API_URL, get_options, validate_jwt } from "../../../site/util";
import axios from "axios";
import Plan from "./Plan";

const BillingPanel = (props) => {
	const fetchPlan = async (organization_slug: string) => {
		const EMPTY_OBJECT = {};
		try {
			const token = props.user?.token;
			if (!validate_jwt(props.user?.token)) {
				return EMPTY_OBJECT;
			}
			const url = `${BENCHER_API_URL()}/v0/organizations/${organization_slug}/plan`;
			const resp = await axios(get_options(url, token));
			return resp?.data;
		} catch (error) {
			console.error(error);
			return EMPTY_OBJECT;
		}
	};

	const [plan] = createResource(props.organization_slug, fetchPlan);

	return (
		<>
			<BillingHeader config={props.config?.header} />

			<Switch
				fallback={
					<MeteredBilling
						user={props.user}
						organization_slug={props.organization_slug}
					/>
				}
			>
				<Match when={plan()?.level}>
					<Plan plan={plan} />
				</Match>
				<Match when={props.host === Host.SELF_HOSTED}>
					<LicensedBilling />
				</Match>
			</Switch>
		</>
	);
};

export default BillingPanel;
