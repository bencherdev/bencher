import { Link } from "solid-app-router";
import slugify from "slugify";
import Page from "./config/page";

enum Section {
	HOW_TO = "How To",
	REFERENCE = "Reference",
}

const asSlug = (text: string) => {
	return slugify(text, {
		lower: true,
	});
};

const getDocsPath = (section: Section, page: string) => {
	return `/docs/${asSlug(section)}/${asSlug(page)}`;
};

const DocsMenu = (props) => {
	return (
		<aside class="menu">
			<p class="menu-label">{Section.HOW_TO}</p>
			<ul class="menu-list">
				<li>
					<Link href={getDocsPath(Section.HOW_TO, Page.QUICK_START)}>
						{Page.QUICK_START}
					</Link>
				</li>
				<li>
					<Link href={getDocsPath(Section.HOW_TO, Page.GITHUB_ACTIONS)}>
						{Page.GITHUB_ACTIONS}
					</Link>
				</li>
				<li>
					<Link href={getDocsPath(Section.HOW_TO, Page.GITLAB_CI)}>
						{Page.GITLAB_CI}
					</Link>
				</li>
				<li>
					<Link href={getDocsPath(Section.HOW_TO, Page.BRANCH_MANAGEMENT)}>
						{Page.BRANCH_MANAGEMENT}
					</Link>
				</li>
			</ul>

			<p class="menu-label">{Section.REFERENCE}</p>
			<ul class="menu-list">
				<Link href={getDocsPath(Section.REFERENCE, Page.API_V0)}>
					{Page.API_V0}
				</Link>
				<Link href={getDocsPath(Section.REFERENCE, Page.PRIOR_ART)}>
					{Page.PRIOR_ART}
				</Link>
				<Link href={getDocsPath(Section.REFERENCE, Page.CHANGELOG)}>
					{Page.CHANGELOG}
				</Link>
			</ul>
		</aside>
	);
};

export default DocsMenu;
