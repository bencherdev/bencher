import { Show, type Accessor, type Resource } from "solid-js";

enum Section {
	PLOTS = "plots",
	PERF = "perf",
	REPORTS = "reports",
	BRANCHES = "branches",
	TESTBEDS = "testbeds",
	BENCHMARKS = "benchmarks",
	MEASURES = "measures",
	THRESHOLDS = "thresholds",
	ALERTS = "alerts",
	SETTINGS = "settings",
}

const ProjectMenuInner = (props: {
	project: Accessor<undefined | string>;
	active_alerts: Resource<undefined | number>;
}) => {
	const path = (section?: Section) =>
		section
			? `/console/projects/${props.project()}/${section}`
			: `/console/projects/${props.project()}`;

	return (
		<aside class="menu is-sticky">
			<div class="menu-label">
				<a
					class="button is-fullwidth"
					title="View Project Plots"
					href={path(Section.PLOTS)}
				>
					<span class="icon">
						<i class="fas fa-th-large" />
					</span>
				</a>
			</div>
			<div class="menu-label">
				<a
					class="button is-fullwidth"
					title="View Project Perf"
					href={path(Section.PERF)}
				>
					<span class="icon">
						<i class="fas fa-chart-line" />
					</span>
				</a>
			</div>
			<p class="menu-label">Project</p>
			<ul class="menu-list">
				<li>
					<a href={path(Section.REPORTS)}>Reports</a>
				</li>
				<li>
					<a href={path(Section.BRANCHES)}>Branches</a>
				</li>
				<li>
					<a href={path(Section.TESTBEDS)}>Testbeds</a>
				</li>
				<li>
					<a href={path(Section.BENCHMARKS)}>Benchmarks</a>
				</li>
				<li>
					<a href={path(Section.MEASURES)}>Measures</a>
				</li>
				<li>
					<a href={path(Section.THRESHOLDS)}>Thresholds</a>
				</li>
				<li>
					<a href={path(Section.ALERTS)}>
						<nav class="level is-mobile">
							<div class="level-left">
								<div class="level-item">Alerts</div>
								<span class="tag is-primary is-small is-rounded">
									<Show
										when={props.active_alerts() !== undefined}
										fallback={<>&nbsp;&nbsp;</>}
									>
										{props.active_alerts()}
									</Show>
								</span>
							</div>
						</nav>
					</a>
				</li>
				<li>
					<a href={path(Section.SETTINGS)}>Settings</a>
				</li>
			</ul>
		</aside>
	);
};

export default ProjectMenuInner;
