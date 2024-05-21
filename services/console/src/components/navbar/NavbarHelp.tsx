import { authUser } from "../../util/auth";
import { BACK_PARAM, encodePath } from "../../util/url";

const NavbarHelp = () => {
	const user = authUser();

	return (
		<a
			class="button"
			href={`/console/users/${
				user?.user?.slug
			}/help/?${BACK_PARAM}=${encodePath()}`}
		>
			<span class="icon has-text-primary">
				<i class="fas fa-life-ring" />
			</span>
			<span>Help</span>
		</a>
	);
};

export default NavbarHelp;
