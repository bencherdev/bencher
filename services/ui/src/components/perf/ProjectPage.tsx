import { useParams } from "solid-app-router";
import { createMemo } from "solid-js";
import projectsConfig from "../console/config/resources/projects";
import { Operation } from "../console/config/types";
import PerfPanel from "../console/panel/perf/PerfPanel";

const ProjectPage = (props) => {
	const params = useParams();
	const path_params = createMemo(() => params);

	const project_slug = createMemo(() => path_params().project_slug);

	return (
		<section class="section">
			<div class="container">
				<div class="columns is-mobile">
					<div class="column">
						<PerfPanel
							user={props.user}
							project_slug={project_slug}
							config={projectsConfig[Operation.PERF]}
							path_params={path_params}
						/>
					</div>
				</div>
			</div>
		</section>
	);
};

export default ProjectPage;
