import type { Params } from "astro";
import type { JsonProject } from "../../../types/bencher";

export interface Props {
	apiUrl: string;
	params: Params;
	project?: undefined | JsonProject;
}

const DashboardPanel = (props: Props) => {
	return (
		<div class="columns">
			<div class="column is-half">
				<div class="box">
					<div class="content has-text-centered">
						<h2 class="title is-2">
							Add a Perf Plot to your Project Dashboard
						</h2>
					</div>
					<div class="content">
						<ol>
							<li>Create a Perf Plot that you want to track.</li>
							<li>
								Click the <code>Pin</code> button.
							</li>
							<li>Name the pinned Perf Plot and set the time window.</li>
						</ol>
					</div>
					<a
						type="button"
						class="button is-primary is-fullwidth"
						href={`/console/projects/${props.params?.project}/perf?clear=true`}
					>
						Create a Perf Plot
					</a>
				</div>
			</div>
		</div>
	);
};

export default DashboardPanel;
