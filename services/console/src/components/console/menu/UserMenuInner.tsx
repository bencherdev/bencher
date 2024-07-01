import type { JsonAuthUser } from "../../../types/bencher";

enum Section {
	TOKENS = "tokens",
	SETTINGS = "settings",
	HELP = "help",
}

const UserMenu = (props: { user?: JsonAuthUser }) => {
	const path = (section: Section) =>
		`/console/users/${props.user?.user?.slug}/${section}`;

	return (
		<aside class="menu is-sticky">
			<p class="menu-label">User</p>
			<ul class="menu-list">
				<li>
					<a href="/console/organizations">Organizations</a>
				</li>
				<li>
					<a href={path(Section.TOKENS)}>API Tokens</a>
				</li>
				<li>
					<a href={path(Section.SETTINGS)}>Settings</a>
				</li>
				<li>
					<a href={path(Section.HELP)}>Help</a>
				</li>
			</ul>
		</aside>
	);
};

export default UserMenu;
