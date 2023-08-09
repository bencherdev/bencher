import { Show, createResource } from "solid-js";
import { authUser, isAllowedOrganization } from "../../../util/auth";
import { JsonOrganizationPermission } from "../../../types/bencher";
import bencher_valid_init from "bencher_valid";
import type { Params } from "astro";

interface Props {
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

	const user = authUser();

	const [billing] = createResource(bencher_valid, async (bv) => {
		if (!bv) {
			return false;
		}
		return await isAllowedOrganization(
			params,
			JsonOrganizationPermission.Manage,
		);
	});

	const path = (section: Section) =>
		`/console/organizations/${user?.user?.slug}/${section}`;

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
