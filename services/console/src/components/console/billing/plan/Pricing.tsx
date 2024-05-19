import { Show, createSignal } from "solid-js";
import { PlanLevel } from "../../../../types/bencher";
import { Theme, getTheme, themeColor } from "../../../navbar/theme/theme";
import Plan from "../../../../pages/console/onboard/plan.astro";

export const per_metric_cost = (plan: PlanLevel) => {
	switch (plan) {
		case PlanLevel.Free:
			return 0;
		case PlanLevel.Team:
			return 1;
		case PlanLevel.Enterprise:
			return 5;
	}
};

interface Props {
	plan?: PlanLevel;
	freeText: string;
	handleFree: () => void;
	hideFree?: undefined | boolean;
	teamText: string;
	handleTeam: () => void;
	enterpriseText: string;
	handleEnterprise: () => void;
}

const Pricing = (props: Props) => {
	const [theme, setTheme] = createSignal(Theme.Light);
	setInterval(() => {
		const newTheme = getTheme();
		if (theme() != newTheme) {
			setTheme(newTheme);
		}
	}, 100);

	return (
		<>
		<div class="columns is-centered">
			<div class="column is-3">
				<div class={`panel ${themeColor(theme())}`}>
					<div class="panel-heading">
						<div class="content has-text-centered">
						<h2 class="title is-2">Free</h2>
						<h3 class="subtitle is-1">$0</h3>
						<br />
						<sup>per benchmark result</sup>
						</div>
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Public Projects
					</div>
					<div class="panel-block">
						<p>⎯⎯⎯</p>
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Community Support
					</div>
					<div class="panel-block">
						<p>⎯⎯⎯</p>
					</div>
					<div class="panel-block">
						<p>⎯⎯⎯</p>
					</div>
					<div class="panel-block">
						<button
							class="button is-fullwidth"
							type="button"
							onClick={(e) => {
								e.preventDefault();
								props.handleFree();
							}}
						>
							{props.freeText}
						</button>
					</div>
				</div>
			</div>
			<div class="column is-3">
				<div class={`panel ${themeColor(theme())}`}>
					<div class="panel-heading">
						<div class="content has-text-centered">
						<h2 class="title is-2">Team</h2>
						<h3 class="subtitle is-1">$0.01</h3>
						<br />
						<sup>per benchmark result</sup>
						</div>
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Public Projects
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Private Projects
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Customer Support
					</div>
					<div class="panel-block">
						<p>⎯⎯⎯</p>
					</div>
					<div class="panel-block">
						<p>⎯⎯⎯</p>
					</div>
					<div class="panel-block">
						<button
							class="button is-fullwidth"
							type="button"
							onClick={(e) => {
								e.preventDefault();
								props.handleTeam();
							}}
						>
							{props.teamText}
						</button>
					</div>
				</div>
			</div>
			<div class="column is-3">
				<div class={`panel ${themeColor(theme())}`}>
					<div class="panel-heading">
						<div class="content has-text-centered">
						<h2 class="title is-2">Enterprise</h2>
						<h3 class="subtitle is-1">$0.05</h3>
						<br />
						<sup>per benchmark result</sup>
						</div>
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Public Projects
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Private Projects
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Priority Support
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Single Sign-On (SSO)
					</div>
					<div class="panel-block">
						<span class="panel-icon">
      						<i class="fas fa-check" aria-hidden="true"></i>
    					</span>
						Dedicated Onboarding
					</div>
					<div class="panel-block">
						<button
							class={`button is-fullwidth ${props.plan === PlanLevel.Enterprise && "is-primary"}`}
							type="button"
							onClick={(e) => {
								e.preventDefault();
								props.handleEnterprise();
							}}
						>
							{props.enterpriseText}
						</button>
					</div>
				</div>
			</div>
		</div>
		<div id="plans" class="pricing-table is-comparative">
			<Show when={!props.hideFree}>
				<div
					class={`pricing-plan ${props.plan === PlanLevel.Free && "is-active"}`}
				>
					<div class="plan-header">
						<h2 class="title is-2">Free</h2>
					</div>
					<div class="plan-price">
						<span class="plan-price-amount">$0</span>
					</div>
					<div class="content has-text-centered">
						<sup>per benchmark result</sup>
					</div>
					<div class="plan-items">
						<div class="plan-item">Public Projects</div>
						<div class="plan-item" data-feature="Private Projects">
							⎯⎯⎯
						</div>
						<div class="plan-item">Community Support</div>
						<div class="plan-item" data-feature="Single Sign-On (SSO)">
							⎯⎯⎯
						</div>
						<div class="plan-item" data-feature="Dedicated Onboarding">
							⎯⎯⎯
						</div>
					</div>
					<Footer
						active={props.plan}
						plan={PlanLevel.Free}
						text={props.freeText}
						handlePlanLevel={props.handleFree}
					/>
				</div>
			</Show>

			<div
				class={`pricing-plan ${props.plan === PlanLevel.Team && "is-active"}`}
			>
				<div class="plan-header">
					<h2 class="title is-2">Team</h2>
				</div>
				<div class="plan-price">
					<span class="plan-price-amount">$0.01</span>
				</div>
				<div class="content has-text-centered">
					<sup>per benchmark result</sup>
				</div>
				<div class="plan-items">
					<div class="plan-item">Public Projects</div>
					<div class="plan-item">Private Projects</div>
					<div class="plan-item">Customer Support</div>
					<div class="plan-item" data-feature="Single Sign-On (SSO)">
						⎯⎯⎯
					</div>
					<div class="plan-item" data-feature="Dedicated Onboarding">
						⎯⎯⎯
					</div>
				</div>
				<Footer
					active={props.plan}
					plan={PlanLevel.Team}
					text={props.teamText}
					handlePlanLevel={props.handleTeam}
				/>
			</div>

			<div
				class={`pricing-plan ${
					props.plan === PlanLevel.Enterprise && "is-active"
				}`}
			>
				<div class="plan-header">
					<h2 class="title is-2">Enterprise</h2>
				</div>
				<div class="plan-price">
					<span class="plan-price-amount">$0.05</span>
				</div>
				<div class="content has-text-centered">
					<sup>per benchmark result</sup>
				</div>
				<div class="plan-items">
					<div class="plan-item">Public Projects</div>
					<div class="plan-item">Private Projects</div>
					<div class="plan-item">Priority Support</div>
					<div class="plan-item">Single Sign-On (SSO)</div>
					<div class="plan-item">Dedicated Onboarding</div>
				</div>
				<Footer
					active={props.plan}
					plan={PlanLevel.Enterprise}
					text={props.enterpriseText}
					handlePlanLevel={props.handleEnterprise}
				/>
			</div>
		</div>
		</>
	);
};

const Footer = (props: {
	active: undefined | string;
	plan: string;
	text: string;
	handlePlanLevel: () => void;
}) => {
	return (
		<div class="plan-footer">
			<Show
				when={props.active === props.plan}
				fallback={
					<div class="columns is-centered">
						<div class="column is-11">
							<button
								class="button is-fullwidth"
								type="button"
								onClick={(e) => {
									e.preventDefault();
									props.handlePlanLevel();
								}}
							>
								{props.text}
							</button>
						</div>
					</div>
				}
			>
				<button
					class="button is-fullwidth"
					type="button"
					onClick={(e) => {
						e.preventDefault();
						props.handlePlanLevel();
					}}
				>
					{props.text}
				</button>
			</Show>
		</div>
	);
};

export default Pricing;
