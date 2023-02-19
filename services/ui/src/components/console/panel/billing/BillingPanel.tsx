import { Match, Switch } from "solid-js";
import { Host } from "../../config/resources/billing";
import MeteredBilling from "./MeteredBilling";
import LicensedBilling from "./LicensedBilling";
import BillingHeader from "./BillingHeader";

const BillingPanel = (props) => {
	return (
		<>
			<BillingHeader config={props.config?.header} />

			<Switch fallback={<MeteredBilling />}>
				<Match when={props.host === Host.SELF_HOSTED}>
					<LicensedBilling />
				</Match>
			</Switch>
		</>
	);
};

export default BillingPanel;
