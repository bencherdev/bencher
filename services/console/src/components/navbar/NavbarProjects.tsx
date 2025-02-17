import type { Params } from "astro";
import type { JsonAuthUser } from "../../types/bencher";
import { authUser } from "../../util/auth";
import ProjectsLink from "./ProjectsLink";

export interface Props {
	apiUrl: string;
	params: Params;
}

const NavbarProjects = (props: Props) => {
	const user = authUser();

	return (
		<ProjectsLink
			apiUrl={props.apiUrl}
			params={props.params as Params}
			user={user as JsonAuthUser}
		/>
	);
};

export default NavbarProjects;
