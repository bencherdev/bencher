import { authUser } from "../../util/auth";
import { BENCHER_VERSION } from "../../util/ext";
import { BACK_PARAM, encodePath } from "../../util/url";

const NavbarDropdown = () => {
	const user = authUser();

	return (
		<div class="navbar-item has-dropdown is-hoverable">
			<div class="navbar-link">
				{(user?.user?.name ? user.user.name : "Account").padStart(12, "\xa0")}
			</div>
			<div class="navbar-dropdown">
				<a class="navbar-item" href="/console/organizations">
					Organizations
				</a>
				<a
					class="navbar-item"
					href={`/console/users/${user?.user?.slug}/tokens`}
				>
					Tokens
				</a>
				<a
					class="navbar-item"
					href={`/console/users/${
						user?.user?.slug
					}/settings?${BACK_PARAM}=${encodePath()}`}
				>
					Settings
				</a>
				<hr class="navbar-divider" />
				<div class="navbar-item">
					<a class="button is-fullwidth" href="/auth/logout">
						Log out
					</a>
				</div>
				<hr class="navbar-divider" />
				<div class="navbar-item">v{BENCHER_VERSION}</div>
			</div>
		</div>
	);
};

export default NavbarDropdown;
