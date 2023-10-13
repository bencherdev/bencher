import { Show, createMemo, createResource } from "solid-js";
import { isAllowedOrganization } from "../../../util/auth";
import { OrganizationPermission } from "../../../types/bencher";
import bencher_valid_init from "bencher_valid";
import type { Params } from "astro";

interface Props {
	apiUrl: string;
	params: Params;
}

enum Section {
	PROJECTS = "projects",
	MEMBERS = "members",
	SETTINGS = "settings",
	BILLING = "billing",
}

const OrgMenu = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const params = createMemo(() => props.params);
	const [billing] = createResource(bencher_valid, async (bv) => {
		if (!bv) {
			return false;
		}
		return await isAllowedOrganization(
			props.apiUrl,
			params(),
			OrganizationPermission.Manage,
		);
	});

	const path = (section: Section) =>
		`/console/organizations/${params()?.organization}/${section}`;

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
				<Show when={billing} fallback={<></>}>
					<li>
						<a href={path(Section.BILLING)}>Billing</a>
					</li>
				</Show>
			</ul>
		</aside>
	);
};

export default OrgMenu;
