enum Section {
	TOKENS = "tokens",
	INFO = "info",
	HELP = "help",
}

const UserMenu = () => {
	const path = (section: Section) =>
		`/console/settings/${section}`;

	return (
		<aside class="menu is-sticky">
			<p class="menu-label">User</p>
			<ul class="menu-list">
				<li>
					<a href={path(Section.INFO)}>User Info</a>
				</li>
				<li>
					<a href={path(Section.TOKENS)}>API Tokens</a>
				</li>
				<li>
					<a href={path(Section.HELP)}>Help</a>
				</li>
			</ul>
		</aside>
	);
};

export default UserMenu;
