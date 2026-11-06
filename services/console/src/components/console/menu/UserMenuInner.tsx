import type { Reader } from "../../../next/reader";

enum Section {
	KEYS = "keys",
	APPS = "apps",
	SETTINGS = "settings",
	HELP = "help",
}

const UserMenu = (props: { user?: undefined | Reader }) => {
	const path = (section: Section) =>
		`/console/users/${props.user?.slug}/${section}`;

	return (
		<aside class="menu is-sticky">
			<p class="menu-label">User</p>
			<ul class="menu-list">
				<li>
					<a href="/console/organizations">Organizations</a>
				</li>
				<li>
					<a href={path(Section.KEYS)}>API Keys</a>
				</li>
				<li>
					<a href={path(Section.APPS)}>Connected Apps</a>
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
