import PageKind from "../page_kind";
import GitHubActions from "../../../../docs/how_to/GitHubActions.mdx";
import GitLabCiCd from "../../../../docs/how_to/GitLabCiCd.mdx";
import InstallCli from "../../../../docs/how_to/InstallCli.mdx";
import TrackBenchmarks from "../../../../docs/how_to/TrackBenchmarks.mdx";

const HowTo = [
	{
		title: "Install CLI",
		slug: "install-cli",
		panel: {
			kind: PageKind.MDX,
			heading: (
				<>
					How to Install <code>bencher</code> CLI
				</>
			),
			content: <InstallCli />,
		},
	},
	{
		title: "Track Benchmarks",
		slug: "track-benchmarks",
		panel: {
			kind: PageKind.MDX,
			heading: "How to use Bencher to Track Benchmarks",
			content: <TrackBenchmarks />,
		},
	},
	{
		title: "GitHub Actions",
		slug: "github-actions",
		panel: {
			kind: PageKind.MDX,
			heading: "How to use Bencher in GitHub Actions",
			content: <GitHubActions />,
		},
	},
	{
		title: "GitLab CI/CD",
		slug: "gitlab-ci-cd",
		panel: {
			kind: PageKind.MDX,
			heading: "How to use Bencher in GitLab CI/CD",
			content: <GitLabCiCd />,
		},
	},
];

export default HowTo;
