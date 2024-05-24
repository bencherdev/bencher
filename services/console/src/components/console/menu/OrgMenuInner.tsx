import { Show, type Accessor, type Resource } from "solid-js";

enum Section {
	PROJECTS = "projects",
	MEMBERS = "members",
	SETTINGS = "settings",
	BILLING = "billing",
}

const OrgMenuInner = (props: {
	organization: Accessor<undefined | string>;
	billing: Resource<boolean>;
}) => {
	const path = (section: Section) =>
		`/console/organizations/${props.organization()}/${section}`;

	return (
		<aside class="menu is-sticky">
			<p class="menu-label">Organization</p>
			<ul class="menu-list">
				<li>
					<a href={path(Section.PROJECTS)}>Projects</a>
				</li>
				<li>
					<a href={path(Section.MEMBERS)}>Members</a>
				</li>
				<li>
					<a href={path(Section.SETTINGS)}>Settings</a>
				</li>
				<Show when={props.billing}>
					<li>
						<a href={path(Section.BILLING)}>Billing</a>
					</li>
				</Show>
			</ul>
		</aside>
	);
};

export default OrgMenuInner;
