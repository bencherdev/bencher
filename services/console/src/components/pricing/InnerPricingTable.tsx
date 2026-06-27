import { For, Show } from "solid-js";
import { PlanLevel } from "../../types/bencher";

interface Props {
	freeCtaText?: string;
	handleFree: () => void;
	handlePro: () => void;
	handleEnterprise: () => void;
}

type FeatureMark = "check" | "dash";

interface Feature {
	mark: FeatureMark;
	label: string;
}

interface HighlightLine {
	label: string;
	value?: string;
}

interface RunnerSpec {
	concurrentJobs: string;
	jobTimeout: string;
	queuePriority: string;
	runnerTypes: HighlightLine[];
}

interface Tier {
	plan: PlanLevel;
	title: string;
	tagline: string;
	price: string;
	priceUnit: string;
	popular?: boolean;
	ctaStyle: "primary" | "outlined";
	features: Feature[];
	series: HighlightLine[];
	runners: RunnerSpec;
}

const TIERS: Tier[] = [
	{
		plan: PlanLevel.Free,
		title: "Free",
		tagline: "For open source projects",
		price: "$0",
		priceUnit: " / month",
		ctaStyle: "outlined",
		features: [
			{ mark: "check", label: "Public projects" },
			{ mark: "dash", label: "Private projects" },
			{ mark: "check", label: "Community support" },
		],
		series: [{ label: "Public Series", value: "Free" }],
		runners: {
			concurrentJobs: "1",
			jobTimeout: "5 min",
			queuePriority: "Standard",
			runnerTypes: [{ label: "On-Demand Runners", value: "Free" }],
		},
	},
	{
		plan: PlanLevel.Pro,
		title: "Pro",
		tagline: "For performance-critical projects",
		price: "$100",
		priceUnit: " / month",
		popular: true,
		ctaStyle: "primary",
		features: [
			{ mark: "check", label: "250 benchmark series included" },
			{ mark: "check", label: "Public & Private projects" },
			{ mark: "check", label: "Priority support" },
		],
		series: [
			{ label: "Included series", value: "250" },
			{ label: "Additional series", value: "Tiered" },
		],
		runners: {
			concurrentJobs: "Unlimited",
			jobTimeout: "Unlimited",
			queuePriority: "Priority",
			runnerTypes: [{ label: "On-Demand Runners", value: "$1.00 / hr" }],
		},
	},
	{
		plan: PlanLevel.Enterprise,
		title: "Enterprise",
		tagline: "For performance-critical organizations",
		price: "Custom",
		priceUnit: "",
		ctaStyle: "outlined",
		features: [
			{ mark: "check", label: "Single sign-on (SSO)" },
			{ mark: "check", label: "On-premise deployment" },
			{ mark: "check", label: "Dedicated onboarding" },
		],
		series: [{ label: "Public Series" }, { label: "Private Series" }],
		runners: {
			concurrentJobs: "Unlimited",
			jobTimeout: "Unlimited",
			queuePriority: "Priority",
			runnerTypes: [
				{ label: "On-Demand Runners" },
				{ label: "Dedicated Runners" },
				{ label: "Custom Runners" },
			],
		},
	},
];

const ctaTextFor = (plan: PlanLevel, freeCtaText?: string): string => {
	switch (plan) {
		case PlanLevel.Pro:
			return "Start 1-month free trial";
		case PlanLevel.Enterprise:
			return "Contact Us";
		default:
			return freeCtaText ?? "Sign up for Free";
	}
};

const handlerFor = (plan: PlanLevel, props: Props): (() => void) => {
	switch (plan) {
		case PlanLevel.Pro:
			return props.handlePro;
		case PlanLevel.Enterprise:
			return props.handleEnterprise;
		default:
			return props.handleFree;
	}
};

const HighlightBox = (props: { lines: HighlightLine[]; popular?: boolean }) => (
	<dl class={`pricing-highlight${props.popular ? " is-popular" : ""}`}>
		<For each={props.lines}>
			{(line) => (
				<div class="pricing-highlight-row">
					<dt>{line.label}</dt>
					{line.value ? (
						<dd>{line.value}</dd>
					) : (
						<dd class="pricing-highlight-check">✓</dd>
					)}
				</div>
			)}
		</For>
	</dl>
);

const InnerPricingTable = (props: Props) => {
	return (
		<div class="pricing-grid">
			<For each={TIERS}>
				{(tier) => (
					<div class={`pricing-card${tier.popular ? " is-popular" : ""}`}>
						<Show when={tier.popular}>
							<div class="pricing-ribbon">MOST POPULAR</div>
						</Show>
						<div class="pricing-header">
							<div class="pricing-title">{tier.title}</div>
							<div class="pricing-tagline">{tier.tagline}</div>
						</div>
						<div class="pricing-price-row">
							<span class="pricing-price">{tier.price}</span>
							<span class="pricing-price-unit">{tier.priceUnit}</span>
						</div>
						<button
							type="button"
							class={`pricing-cta pricing-cta-${tier.ctaStyle}`}
							onClick={() => handlerFor(tier.plan, props)()}
						>
							{ctaTextFor(tier.plan, props.freeCtaText)}
						</button>
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
						<div class="pricing-section-label">BENCHMARK SERIES</div>
						<HighlightBox lines={tier.series} popular={tier.popular ?? false} />
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
						</dl>
						<HighlightBox
							lines={tier.runners.runnerTypes}
							popular={tier.popular ?? false}
						/>
					</div>
				)}
			</For>
		</div>
	);
};

export default InnerPricingTable;
