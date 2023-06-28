import PageKind from "../page_kind";
import Changelog from "../../../../docs/reference/Changelog.mdx";
import PriorArt from "../../../../docs/reference/PriorArt.mdx";
import Roadmap from "../../../../docs/reference/Roadmap.mdx";
import ServerConfig from "../../../../docs/reference/ServerConfig.mdx";

const Reference = [
	{
		title: "REST API",
		slug: "api",
		panel: {
			kind: PageKind.SWAGGER,
			heading: "",
			content: <></>,
		},
	},
	{
		title: "Server Config",
		slug: "server-config",
		panel: {
			kind: PageKind.MDX,
			heading: "API Server Config",
			content: <ServerConfig />,
		},
	},
	{
		title: "Prior Art",
		slug: "prior-art",
		panel: {
			kind: PageKind.MDX,
			heading: "Prior Art",
			content: <PriorArt />,
		},
	},
	{
		title: "Roadmap",
		slug: "roadmap",
		panel: {
			kind: PageKind.MDX,
			heading: "Bencher Roadmap",
			content: <Roadmap />,
		},
	},
	{
		title: "Changelog",
		slug: "changelog",
		panel: {
			kind: PageKind.MDX,
			heading: "Changelog",
			content: <Changelog />,
		},
	},
];

export default Reference;
