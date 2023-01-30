import PageKind from "../page_kind";
import ContinuousBenchmarking from "../../pages/explanation/ContinuousBenchmarking.mdx";
import BranchSelection from "../../pages/explanation/BranchSelection.mdx";
import Talks from "../../pages/explanation/Talks.mdx";
import Adapters from "../../pages/explanation/Adapters.mdx";

const Explanation = [
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
		title: "CLI Adapters",
		slug: "cli-adapters",
		panel: {
			kind: PageKind.MDX,
			heading: (
				<>
					Adapters for <code>bencher run</code>
				</>
			),
			content: <Adapters />,
		},
	},
	{
		title: "CLI Branch Selection",
		slug: "cli-branch-selection",
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
