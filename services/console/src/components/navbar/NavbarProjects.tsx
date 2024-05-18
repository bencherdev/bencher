import type { Params } from "astro";
import { Show } from "solid-js";
import type { JsonAuthUser } from "../../types/bencher";
import { authUser } from "../../util/auth";
import ProjectSelect from "./ProjectSelect";

export interface Props {
	apiUrl: string;
	params: Params;
}

const NavbarProjects = (props: Props) => {
	const user = authUser();

	return (
		<Show
			when={user && (props.params?.organization || props.params?.project)}
		>
			<div class="navbar-item">
				<ProjectSelect
					apiUrl={props.apiUrl}
					params={props.params as Params}
					user={user as JsonAuthUser}
				/>
			</div>
		</Show>
	);
};

export default NavbarProjects;
