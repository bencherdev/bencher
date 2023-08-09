import { authUser } from "../../../util/auth";

enum Section {
	TOKENS = "tokens",
	SETTINGS = "settings",
	HELP = "help",
}

const UserMenu = () => {
	const user = authUser();
	const path = (section: Section) =>
		`/console/users/${user?.user?.slug}/${section}`;

	return (
		<aside class="menu is-sticky">
			<p class="menu-label">User</p>
			<ul class="menu-list">
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
