import { For, type Accessor } from "solid-js";
import type { PlanLevel } from "../../../types/bencher";
import { Theme, themeWordmark } from "../../navbar/theme/theme";

export interface Props {
	theme?: Theme;
	step: OnboardStep;
	plan?: Accessor<PlanLevel>;
}

export enum OnboardStep {
	API_TOKEN = 1,
	PROJECT = 2,
	RUN = 3,
	INVITE = 4,
	PLAN = 5,
}

const stepPath = (step: OnboardStep) => {
	switch (step) {
		case OnboardStep.API_TOKEN:
			return "/console/onboard/token";
		case OnboardStep.PROJECT:
			return "/console/onboard/project";
		case OnboardStep.RUN:
			return "/console/onboard/run";
		case OnboardStep.INVITE:
			return "/console/onboard/invite";
		case OnboardStep.PLAN:
			return "/console/onboard/plan";
	}
};

const OnboardSteps = (props: Props) => {
	const stepHref = (step: OnboardStep) => {
		const path = stepPath(step);
		const plan = props.plan?.();
		return plan ? `${path}?plan=${plan}` : path;
	};

	return (
		<section class="section">
			<div class="container">
				<div class="columns is-centered">
					<div class="column is-half">
						<div class="content has-text-centered">
							<div title="Bencher - Continuous Benchmarking">
								<img
									src={themeWordmark(props.theme ?? Theme.Light)}
									width="150"
									height="28.25"
									alt="🐰 Bencher"
								/>
							</div>
						</div>
						<br />
						<nav
							class="breadcrumb is-centered has-bullet-separator"
							aria-label="breadcrumbs"
						>
							<ul>
								<For
									each={[
										OnboardStep.API_TOKEN,
										OnboardStep.PROJECT,
										OnboardStep.RUN,
										OnboardStep.INVITE,
										OnboardStep.PLAN,
									]}
								>
									{(step) => (
										<li class={props.step === step ? "is-active" : ""}>
											<a
												href={stepHref(step)}
												aria-current={props.step === step ? "page" : undefined}
											>
												<span
													class={`tag ${
														props.step >= step ? "is-primary" : ""
													}`}
												>
													{step}
												</span>
											</a>
										</li>
									)}
								</For>
							</ul>
						</nav>
					</div>
				</div>
			</div>
		</section>
	);
};

export default OnboardSteps;
