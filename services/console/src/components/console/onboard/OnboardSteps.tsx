import { For } from "solid-js";
import { BENCHER_WORDMARK } from "../../../util/ext";

interface Props {
	step: OnboardStep;
}

export enum OnboardStep {
	API_TOKEN = 1,
	PROJECT = 2,
	RUN = 3,
	INVITE = 4,
	PLUS = 5,
}

const stepHref = (step: OnboardStep) => {
	switch (step) {
		case OnboardStep.API_TOKEN:
			return "/console/onboard/token";
		case OnboardStep.PROJECT:
			return "/console/onboard/project";
		case OnboardStep.RUN:
			return "/console/onboard/report";
		case OnboardStep.INVITE:
			return "/console/onboard/invite";
		case OnboardStep.PLUS:
			return "/console/onboard/billing";
	}
};

const OnboardSteps = (props: Props) => {
	return (
		<section class="section">
			<div class="container">
				<div class="columns is-centered">
					<div class="column is-half">
						<div class="content has-text-centered">
							<div title="Bencher - Continuous Benchmarking">
								<img
									src={BENCHER_WORDMARK}
									width="150"
									height="28.25"
									alt="🐰 Bencher"
								/>
							</div>
						</div>
						<br />
						<div class="steps">
							<For
								each={[
									OnboardStep.API_TOKEN,
									OnboardStep.PROJECT,
									OnboardStep.RUN,
									OnboardStep.INVITE,
									OnboardStep.PLUS,
								]}
							>
								{(step) => (
									<div
										class={`step-item ${
											props.step >= step ? " is-active is-primary" : ""
										}`}
									>
										<a class="step-marker" href={stepHref(step)}>
											{step}
										</a>
									</div>
								)}
							</For>
						</div>
					</div>
				</div>
			</div>
		</section>
	);
};

export default OnboardSteps;
