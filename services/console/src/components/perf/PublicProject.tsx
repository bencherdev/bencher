import type { Params } from "astro";
import PerfPanel from "../console/perf/PerfPanel";

export interface Props {
	params: Params;
}

const PublicProject = (props: Props) => {
	return (
		<section class="section">
			<div class="container">
				<div class="columns is-mobile">
					<div class="column">
						<PerfPanel params={props.params} isConsole={false} />
					</div>
				</div>
			</div>
		</section>
	);
};

export default PublicProject;
