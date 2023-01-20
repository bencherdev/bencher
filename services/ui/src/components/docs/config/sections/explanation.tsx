import PageKind from "../page_kind";
import ContinuousBenchmarking from "../../pages/explanation/ContinuousBenchmarking.mdx";
import BranchManagement from "../../pages/explanation/BranchManagement.mdx";

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
		title: "Branch Management",
		slug: "branch-management",
		panel: {
			kind: PageKind.MDX,
			heading: (
				<>
					Branch Management with <code>bencher run</code>
				</>
			),
			content: <BranchManagement />,
		},
	},
];

export default Explanation;
