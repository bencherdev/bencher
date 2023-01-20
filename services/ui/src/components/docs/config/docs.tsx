import QuickStart from "../pages/QuickStart.mdx";
import Changelog from "../pages/Changelog.mdx";
import PriorArt from "../pages/PriorArt.mdx";
import GitHubActions from "../pages/GitHubActions.mdx";
import GitLabCiCd from "../pages/GitLabCiCd.mdx";
import BranchManagement from "../pages/BranchManagement.mdx";
import Roadmap from "../pages/Roadmap.mdx";
import ServerConfig from "../pages/ServerConfig.mdx";
import ContinuousBenchmarking from "../pages/ContinuousBenchmarking.mdx";

export const getHref = (section_slug: string, page_slug: string) => {
	return `/docs/${getPath(section_slug, page_slug)}`;
};

export const getPath = (section_slug: string, page_slug: string) => {
	return `${section_slug}/${page_slug}`;
};

export const Section = {
	tutorial: {
		name: "Tutorial",
		slug: "tutorial",
	},
	how_to: {
		name: "How To",
		slug: "how-to",
	},
	explanation: {
		name: "Explanation",
		slug: "explanation",
	},
	reference: {
		name: "Reference",
		slug: "reference",
	},
};

export enum PageKind {
	MDX,
	SWAGGER,
}

export const QuickStartPage = {
	title: "Quick Start",
	slug: "quick-start",
	panel: {
		kind: PageKind.MDX,
		heading: "Quick Start",
		content: <QuickStart />,
	},
};

export const GitHubActionsPage = {
	title: "GitHub Actions",
	slug: "github-actions",
	panel: {
		kind: PageKind.MDX,
		heading: "How to use Bencher in GitHub Actions",
		content: <GitHubActions />,
	},
};

export const GitLabCiPage = {
	title: "GitLab CI/CD",
	slug: "gitlab-ci-cd",
	panel: {
		kind: PageKind.MDX,
		heading: "How to use Bencher in GitLab CI/CD",
		content: <GitLabCiCd />,
	},
};

export const ContinuousBenchmarkingPage = {
	title: "Continuous Benchmarking",
	slug: "continuous-benchmarking",
	panel: {
		kind: PageKind.MDX,
		heading: "What is Continuous Benchmarking?",
		content: <ContinuousBenchmarking />,
	},
};

export const BranchManagementPage = {
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
};

export const ApiPage = {
	title: "REST API",
	slug: "api",
	panel: {
		kind: PageKind.SWAGGER,
		heading: "",
		content: <></>,
	},
};

export const ServerConfigPage = {
	title: "Server Config",
	slug: "server-config",
	panel: {
		kind: PageKind.MDX,
		heading: "API Server Config",
		content: <ServerConfig />,
	},
};

export const PriorArtPage = {
	title: "Prior Art",
	slug: "prior-art",
	panel: {
		kind: PageKind.MDX,
		heading: "Prior Art",
		content: <PriorArt />,
	},
};

export const RoadmapPage = {
	title: "Roadmap",
	slug: "roadmap",
	panel: {
		kind: PageKind.MDX,
		heading: "Bencher Roadmap",
		content: <Roadmap />,
	},
};

export const ChangelogPage = {
	title: "Changelog",
	slug: "changelog",
	panel: {
		kind: PageKind.MDX,
		heading: "Changelog",
		content: <Changelog />,
	},
};

export const docs = [
	{
		section: Section.tutorial,
		pages: [QuickStartPage],
	},
	{ section: Section.how_to, pages: [GitHubActionsPage, GitLabCiPage] },
	{
		section: Section.explanation,
		pages: [ContinuousBenchmarkingPage, BranchManagementPage],
	},
	{
		section: Section.reference,
		pages: [
			ApiPage,
			ServerConfigPage,
			PriorArtPage,
			RoadmapPage,
			ChangelogPage,
		],
	},
];
