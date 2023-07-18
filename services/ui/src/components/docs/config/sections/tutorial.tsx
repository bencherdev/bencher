import PageKind from "../page_kind";
import QuickStart from "../../../../docs/tutorial/QuickStart.mdx";
import Docker from "../../../../docs/tutorial/Docker.mdx";

const Tutorial = [
	{
		title: "Quick Start",
		slug: "quick-start",
		panel: {
			kind: PageKind.MDX,
			heading: "Quick Start",
			content: <QuickStart />,
		},
	},
	{
		title: "Docker Self-Hosted",
		slug: "docker",
		panel: {
			kind: PageKind.MDX,
			heading: "Docker Self-Hosted",
			content: <Docker />,
		},
	},
];

export default Tutorial;
