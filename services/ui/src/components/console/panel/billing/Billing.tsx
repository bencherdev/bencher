import { createSignal } from "solid-js";
import Pricing, { Plan } from "./Pricing";

const Billing = (props) => {
	const [plan, setPlan] = createSignal(Plan.TEAM);

	return (
		<div class="content has-text-centered">
			<Pricing
				active={plan()}
				free_text="Choose Free"
				handleFree={(e) => {
					e.preventDefault();
					setPlan(Plan.FREE);
				}}
				team_text="Go with Team"
				handleTeam={(e) => {
					e.preventDefault();
					setPlan(Plan.TEAM);
				}}
				enterprise_text="Go with Enterprise"
				handleEnterprise={(e) => {
					e.preventDefault();
					setPlan(Plan.ENTERPRISE);
				}}
			/>
		</div>
	);
};

export default Billing;
