import PageKind from "../page_kind";
import GitHubActions from "../../pages/how_to/GitHubActions.mdx";
import GitLabCiCd from "../../pages/how_to/GitLabCiCd.mdx";

const HowTo = [
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
