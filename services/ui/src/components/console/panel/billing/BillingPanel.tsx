import Billing from "./Billing";
import BillingHeader from "./BillingHeader";

const BillingPanel = (props) => {
	return (
		<>
			<BillingHeader config={props.config?.header} />
			<Billing />
		</>
	);
};

export default BillingPanel;
