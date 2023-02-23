import axios from "axios";
import { Link } from "solid-app-router";
import { createEffect, createMemo, createResource, For } from "solid-js";
import { BENCHER_API_URL, get_options, pageTitle } from "../site/util";

const ProjectsPage = (props) => {
	const url = createMemo(() => `${BENCHER_API_URL()}/v0/projects?public=true`);

	const fetchProjects = async (user) => {
		return await axios(get_options(url(), user?.token))
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				return [];
			});
	};

	const [projects] = createResource(props.user, fetchProjects);

	createEffect(() => {
		pageTitle("Projects");
	});

	return (
		<section class="section">
			<div class="container">
				<div class="columns is-mobile">
					<div class="column">
						<div class="content">
							<h2 class="title">Projects</h2>
							<hr />
							<br />
							<For each={projects()}>
								{(project) => (
									<Link class="box" href={`/perf/${project.slug}`}>
										{project.name}
									</Link>
								)}
							</For>
							<br />
						</div>
					</div>
				</div>
			</div>
		</section>
	);
};

export default ProjectsPage;
