export enum Plan {
	FREE = "free",
	TEAM = "team",
	ENTERPRISE = "enterprise",
}

const Pricing = (props: {
	active: string;
	free_text: string;
	handleFree: Function;
	team_text: string;
	handleTeam: Function;
	enterprise_text: string;
	handleEnterprise: Function;
}) => {
	return (
		<div class="pricing-table is-comparative">
			<div class={`pricing-plan ${props.active === Plan.FREE && "is-active"}`}>
				<div class="plan-header">
					<h2 class="title">Free</h2>
				</div>
				<div class="plan-price">
					<span class="plan-price-amount">$0</span>
				</div>
				<div class="content has-text-centered">
					<small>per metric/month</small>
				</div>
				<div class="plan-items">
					<div class="plan-item" data-feature="Public Projects">
						Public Projects
					</div>
					<div class="plan-item" data-feature="Team Roles">
						---
					</div>
					<div class="plan-item" data-feature="Private Projects">
						Community Support
					</div>
				</div>
				<Footer
					active={props.active}
					plan={Plan.FREE}
					button_text={props.free_text}
					handlePlan={props.handleFree}
				/>
			</div>

			<div class={`pricing-plan ${props.active === Plan.TEAM && "is-active"}`}>
				<div class="plan-header">
					<h2 class="title">Team</h2>
				</div>
				<div class="plan-price">
					<span class="plan-price-amount">$0.01</span>
				</div>
				<div class="content has-text-centered">
					<small>per metric/month</small>
				</div>
				<div class="plan-items">
					<div class="plan-item" data-feature="Public Projects">
						Public Projects
					</div>
					<div class="plan-item" data-feature="Private Projects">
						Private Projects
					</div>
					<div class="plan-item" data-feature="Team Roles">
						Customer Support
					</div>
				</div>
				<Footer
					active={props.active}
					plan={Plan.TEAM}
					button_text={props.team_text}
					handlePlan={props.handleTeam}
				/>
			</div>

			<div
				class={`pricing-plan ${
					props.active === Plan.ENTERPRISE && "is-active"
				}`}
			>
				<div class="plan-header">
					<h2 class="title">Enterprise</h2>
				</div>
				<div class="plan-price">
					<span class="plan-price-amount">$0.05</span>
				</div>
				<div class="content has-text-centered">
					<small>per metric/month</small>
				</div>
				<div class="plan-items">
					<div class="plan-item" data-feature="Public Projects">
						Public Projects
					</div>
					<div class="plan-item" data-feature="Private Projects">
						Private Projects
					</div>
					<div class="plan-item" data-feature="Team Roles">
						Priority Support
					</div>
				</div>
				<Footer
					active={props.active}
					plan={Plan.ENTERPRISE}
					button_text={props.enterprise_text}
					handlePlan={props.handleEnterprise}
				/>
			</div>
		</div>
	);
};

const Footer = (props: {
	active: string;
	plan: string;
	button_text: string;
	handlePlan: Function;
}) => {
	return (
		<div class="plan-footer">
			{props.plan === props.active ? (
				<button class="button is-fullwidth" onClick={props.handlePlan}>
					{props.button_text}
				</button>
			) : (
				<div class="columns is-centered">
					<div class="column is-four-fifths">
						<button class="button is-fullwidth" onClick={props.handlePlan}>
							{props.button_text}
						</button>
					</div>
				</div>
			)}
		</div>
	);
};

export default Pricing;
