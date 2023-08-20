import type { Params } from "astro";
import PerfPanel from "../console/perf/PerfPanel";

export interface Props {
	apiUrl: string;
	params: Params;
}

const PublicProject = (props: Props) => {
	return (
		<section class="section">
			<div class="container">
				<div class="columns is-mobile">
					<div class="column">
						<PerfPanel
							apiUrl={props.apiUrl}
							params={props.params}
							isConsole={false}
						/>
					</div>
				</div>
			</div>
		</section>
	);
};

export default PublicProject;
