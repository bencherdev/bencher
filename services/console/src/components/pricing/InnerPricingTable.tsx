import { For, Show } from "solid-js";
import { PlanLevel } from "../../types/bencher";

interface Props {
	themeColor: string;
	handleFree: () => void;
	handleTeam: () => void;
	handleEnterprise: () => void;
}

type FeatureMark = "check" | "dash";

interface Feature {
	mark: FeatureMark;
	label: string;
}

interface RunnerSpec {
	concurrentJobs: string;
	jobTimeout: string;
	queuePriority: string;
	runnerRate: string;
}

interface Tier {
	plan: PlanLevel;
	title: string;
	tagline: string;
	price: string;
	projectsNote: string;
	popular?: boolean;
	ctaStyle: "primary" | "outlined";
	features: Feature[];
	runners: RunnerSpec;
	billingFooter?: string;
}

const TIERS: Tier[] = [
	{
		plan: PlanLevel.Free,
		title: "Free",
		tagline: "For open source projects",
		price: "$0",
		projectsNote: "Public projects only",
		ctaStyle: "outlined",
		features: [
			{ mark: "check", label: "Public projects" },
			{ mark: "dash", label: "Private projects" },
			{ mark: "check", label: "Community support" },
		],
		runners: {
			concurrentJobs: "1",
			jobTimeout: "5 min",
			queuePriority: "Standard",
			runnerRate: "Free",
		},
	},
	{
		plan: PlanLevel.Team,
		title: "Team",
		tagline: "For engineering teams",
		price: "$0.01",
		projectsNote: "Public & private projects",
		popular: true,
		ctaStyle: "primary",
		features: [
			{ mark: "check", label: "Public projects" },
			{ mark: "check", label: "Private projects" },
			{ mark: "check", label: "Customer support" },
		],
		runners: {
			concurrentJobs: "Unlimited",
			jobTimeout: "Unlimited",
			queuePriority: "Priority",
			runnerRate: "$1.00 / hr",
		},
		billingFooter: "billed at $0.01666 / min · no minimums",
	},
	{
		plan: PlanLevel.Enterprise,
		title: "Enterprise",
		tagline: "For performance-critical organizations",
		price: "$0.05",
		projectsNote: "Public & private projects",
		ctaStyle: "outlined",
		features: [
			{ mark: "check", label: "Public projects" },
			{ mark: "check", label: "Private projects" },
			{ mark: "check", label: "Priority support" },
			{ mark: "check", label: "Single sign-on (SSO)" },
			{ mark: "check", label: "Dedicated onboarding" },
		],
		runners: {
			concurrentJobs: "Unlimited",
			jobTimeout: "Unlimited",
			queuePriority: "Priority",
			runnerRate: "$1.00 / hr",
		},
		billingFooter: "billed at $0.01666 / min · no minimums",
	},
];

const ctaTextFor = (plan: PlanLevel, popular: boolean | undefined): string => {
	if (plan === PlanLevel.Free) {
		return "Sign up for free";
	}
	if (plan === PlanLevel.Team) {
		return "Continue with Team";
	}
	return "Continue with Enterprise";
};

const handlerFor = (plan: PlanLevel, props: Props): (() => void) => {
	switch (plan) {
		case PlanLevel.Free:
			return props.handleFree;
		case PlanLevel.Team:
			return props.handleTeam;
		case PlanLevel.Enterprise:
			return props.handleEnterprise;
	}
};

const InnerPricingTable = (props: Props) => {
	return (
		<div class="pricing-grid">
			<For each={TIERS}>
				{(tier) => (
					<div
						class={`pricing-card${tier.popular ? " is-popular" : ""}`}
					>
						<Show when={tier.popular}>
							<div class="pricing-ribbon">MOST POPULAR</div>
						</Show>
						<div class="pricing-header">
							<div class="pricing-title">{tier.title}</div>
							<div class="pricing-tagline">{tier.tagline}</div>
						</div>
						<div class="pricing-price-row">
							<span class="pricing-price">{tier.price}</span>
							<span class="pricing-price-unit">
								{" "}/ benchmark result
							</span>
						</div>
						<div class="pricing-projects">{tier.projectsNote}</div>
						<button
							type="button"
							class={`pricing-cta pricing-cta-${tier.ctaStyle}`}
							onMouseDown={(e) => {
								e.preventDefault();
								handlerFor(tier.plan, props)();
							}}
						>
							{ctaTextFor(tier.plan, tier.popular)}
						</button>
						<div class="pricing-section-label">BENCHMARK METRICS</div>
						<ul class="pricing-feature-list">
							<For each={tier.features}>
								{(f) => (
									<li class="pricing-feature">
										<span
											class={`pricing-mark pricing-mark-${f.mark}`}
											aria-hidden="true"
										>
											{f.mark === "check" ? "✓" : "—"}
										</span>
										<span>{f.label}</span>
									</li>
								)}
							</For>
						</ul>
						<div class="pricing-section-label">BARE METAL RUNNERS</div>
						<dl class="pricing-specs">
							<div class="pricing-spec-row">
								<dt>Concurrent jobs</dt>
								<dd>{tier.runners.concurrentJobs}</dd>
							</div>
							<div class="pricing-spec-row">
								<dt>Job timeout</dt>
								<dd>{tier.runners.jobTimeout}</dd>
							</div>
							<div class="pricing-spec-row">
								<dt>Queue priority</dt>
								<dd>{tier.runners.queuePriority}</dd>
							</div>
							<div
								class={`pricing-spec-row pricing-spec-highlight${
									tier.popular ? " is-popular" : ""
								}`}
							>
								<dt>On-Demand Runners</dt>
								<dd>{tier.runners.runnerRate}</dd>
							</div>
						</dl>
						<Show when={tier.billingFooter}>
							<div class="pricing-billing-footer">
								{tier.billingFooter}
							</div>
						</Show>
					</div>
				)}
			</For>
		</div>
	);
};

export default InnerPricingTable;
