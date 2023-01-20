import PageKind from "../page_kind";
import QuickStart from "../../pages/tutorial/QuickStart.mdx";

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
];

export default Tutorial;
