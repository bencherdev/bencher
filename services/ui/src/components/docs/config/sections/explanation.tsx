import PageKind from "../page_kind";
import ContinuousBenchmarking from "../../pages/explanation/ContinuousBenchmarking.mdx";
import BranchSelection from "../../pages/explanation/BranchSelection.mdx";
import Talks from "../../pages/explanation/Talks.mdx";
import Adapters from "../../pages/explanation/Adapters.mdx";
import Thresholds from "../../pages/explanation/Thresholds.mdx";
import Benchmarking from "../../pages/explanation/Benchmarking.mdx";
import BenchmarkVolatility from "../../pages/explanation/BenchmarkVolatility.mdx";

const Explanation = [
	{
		title: "Benchmarking Overview",
		slug: "benchmarking",
		panel: {
			kind: PageKind.MDX,
			heading: "Benchmarking Overview",
			content: <Benchmarking />,
		},
	},
	{
		title: "Benchmark Adapters",
		slug: "adapters",
		panel: {
			kind: PageKind.MDX,
			heading: "Benchmark Harness Adapters",
			content: <Adapters />,
		},
	},
	{
		title: "Branch Selection",
		slug: "branch-selection",
		panel: {
			kind: PageKind.MDX,
			heading: (
				<>
					Branch Selection with <code>bencher run</code>
				</>
			),
			content: <BranchSelection />,
		},
	},
	{
		title: "Thresholds & Alerts",
		slug: "thresholds",
		panel: {
			kind: PageKind.MDX,
			heading: "Thresholds & Alerts",
			content: <Thresholds />,
		},
	},
	{
		title: "Continuous Benchmarking",
		slug: "continuous-benchmarking",
		panel: {
			kind: PageKind.MDX,
			heading: "What is Continuous Benchmarking?",
			content: <ContinuousBenchmarking />,
		},
	},
	{
		title: "Benchmark Volatility",
		slug: "benchmark-volatility",
		panel: {
			kind: PageKind.MDX,
			heading: "Managing Benchmark Volatility",
			content: <BenchmarkVolatility />,
		},
	},
	{
		title: "Talks",
		slug: "talks",
		panel: {
			kind: PageKind.MDX,
			heading: "Talks",
			content: <Talks />,
		},
	},
];

export default Explanation;
