import { createEffect, For } from "solid-js";
import { pageTitle } from "../../../site/util";

const LicensedBilling = (props) => {
	createEffect(() => {
		pageTitle("License Billing");
	});

	return (
		<section class="section">
			<div class="container">
				<div class="columns">
					<div class="column">
						<h4 class="title">Contact Sales</h4>
						<br />
						<h4 class="subtitle">
							Email us directly at{" "}
							<a href="mailto:sales@bencher.dev">sales@bencher.dev</a>
						</h4>
						<br />
					</div>
				</div>
			</div>
		</section>
	);
};

export default LicensedBilling;
