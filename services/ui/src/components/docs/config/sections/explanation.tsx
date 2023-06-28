import PageKind from "../page_kind";
import ContinuousBenchmarking from "../../../../docs/explanation/ContinuousBenchmarking.mdx";
import BranchSelection from "../../../../docs/explanation/BranchSelection.mdx";
import Talks from "../../../../docs/explanation/Talks.mdx";
import Adapters from "../../../../docs/explanation/Adapters.mdx";
import Thresholds from "../../../../docs/explanation/Thresholds.mdx";
import Benchmarking from "../../../../docs/explanation/Benchmarking.mdx";
import BencherRun from "../../../../docs/explanation/BencherRun.mdx";

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
		title: "bencher run",
		slug: "bencher-run",
		panel: {
			kind: PageKind.MDX,
			heading: (
				<>
					<code>bencher run</code> CLI Subcommand
				</>
			),
			content: <BencherRun />,
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
		title: "Benchmark Adapters",
		slug: "adapters",
		panel: {
			kind: PageKind.MDX,
			heading: "Benchmark Harness Adapters",
			content: <Adapters />,
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
