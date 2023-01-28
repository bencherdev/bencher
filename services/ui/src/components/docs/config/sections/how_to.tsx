import PageKind from "../page_kind";
import GitHubActions from "../../pages/how_to/GitHubActions.mdx";
import GitLabCiCd from "../../pages/how_to/GitLabCiCd.mdx";
import InstallCli from "../../pages/how_to/InstallCli.mdx";

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
