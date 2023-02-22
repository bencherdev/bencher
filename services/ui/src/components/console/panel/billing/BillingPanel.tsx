import { createMemo, createResource, Match, Switch } from "solid-js";
import { Host } from "../../config/resources/billing";
import MeteredBilling from "./MeteredBilling";
import LicensedBilling from "./LicensedBilling";
import BillingHeader from "./BillingHeader";
import { BENCHER_API_URL, get_options, validate_jwt } from "../../../site/util";
import axios from "axios";

const BillingPanel = (props) => {
	const fetchOrganization = async (organization_slug: string) => {
		const EMPTY_OBJECT = {};
		try {
			const token = props.user?.token;
			if (!validate_jwt(props.user?.token)) {
				return EMPTY_OBJECT;
			}

			const url = `${BENCHER_API_URL()}/v0/organizations/${props.organization_slug()}/plan`;
			const resp = await axios(get_options(url, token));
			console.log(resp);
			return resp?.data;
		} catch (error) {
			console.error(error);
			return EMPTY_OBJECT;
		}
	};

	const [organization] = createResource(
		props.organization_slug,
		fetchOrganization,
	);

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
				<Match when={organization()?.level}>
					<>HERE</>
				</Match>
				<Match when={props.host === Host.SELF_HOSTED}>
					<LicensedBilling />
				</Match>
			</Switch>
		</>
	);
};

export default BillingPanel;
