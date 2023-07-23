import { Link, useNavigate } from "solid-app-router";
import {
	is_allowed_organization,
	OrganizationPermission,
} from "../../site/util";
import { Match, Show, Switch } from "solid-js";

enum OrganizationSection {
	PROJECTS = "projects",
	MEMBERS = "members",
	SETTINGS = "settings",
	BILLING = "billing",
}

enum ProjectSection {
	PERF = "perf",
	REPORTS = "reports",
	METRIC_KINDS = "metric-kinds",
	BRANCHES = "branches",
	TESTBEDS = "testbeds",
	BENCHMARKS = "benchmarks",
	THRESHOLDS = "thresholds",
	ALERTS = "alerts",
	SETTINGS = "settings",
}

enum UserSection {
	TOKENS = "tokens",
	SETTINGS = "settings",
	HELP = "help",
}

const ConsoleMenu = (props) => {
	const navigate = useNavigate();

	const organizations_path = (section: OrganizationSection) => {
		return `/console/organizations/${props.organization_slug()}/${section}`;
	};

	const projects_path = (section: ProjectSection) => {
		return `/console/projects/${props.project_slug()}/${section}`;
	};

	const users_path = (section: UserSection) => {
		return `/console/users/${props.user?.user?.slug}/${section}`;
	};

	return (
		<aside class="menu">
			<Switch fallback={<></>}>
				<Match
					when={
						typeof props.organization_slug() === "string" &&
						typeof props.project_slug() !== "string"
					}
				>
					<>
						<p class="menu-label">Organization</p>
						<ul class="menu-list">
							<li>
								<Link
									title="Organization Projects"
									href={organizations_path(OrganizationSection.PROJECTS)}
								>
									Projects
								</Link>
							</li>
							<li>
								<Link
									title="Organization Members"
									href={organizations_path(OrganizationSection.MEMBERS)}
								>
									Members
								</Link>
							</li>
							<li>
								<Link
									title="Organization Settings"
									href={organizations_path(OrganizationSection.SETTINGS)}
								>
									Settings
								</Link>
							</li>
							<Show
								when={is_allowed_organization(
									props.path_params(),
									OrganizationPermission.MANAGE,
								)}
								fallback={<></>}
							>
								<li>
									<Link
										title="Organization Billing"
										href={organizations_path(OrganizationSection.BILLING)}
									>
										Billing
									</Link>
								</li>
							</Show>
						</ul>
					</>
				</Match>
				<Match when={typeof props.project_slug() === "string"}>
					<>
						<div class="menu-label">
							<button
								class="button is-outlined is-fullwidth"
								title="View Project Perf"
								onClick={(e) => {
									e.preventDefault();
									navigate(projects_path(ProjectSection.PERF));
								}}
							>
								<span class="icon">
									<i class="fas fa-home" aria-hidden="true" />
								</span>
							</button>
						</div>
						<p class="menu-label">Project</p>
						<ul class="menu-list">
							<li>
								<Link
									title="Project Reports"
									href={projects_path(ProjectSection.REPORTS)}
								>
									Reports
								</Link>
							</li>
							<li>
								<Link
									title="Project Metric Kinds"
									href={projects_path(ProjectSection.METRIC_KINDS)}
								>
									Metric Kinds
								</Link>
							</li>
							<li>
								<Link
									title="Project Branches"
									href={projects_path(ProjectSection.BRANCHES)}
								>
									Branches
								</Link>
							</li>
							<li>
								<Link
									title="Project Testbeds"
									href={projects_path(ProjectSection.TESTBEDS)}
								>
									Testbeds
								</Link>
							</li>
							<li>
								<Link
									title="Project Benchmarks"
									href={projects_path(ProjectSection.BENCHMARKS)}
								>
									Benchmarks
								</Link>
							</li>
							<li>
								<Link
									title="Project Thresholds"
									href={projects_path(ProjectSection.THRESHOLDS)}
								>
									Thresholds
								</Link>
							</li>
							<li>
								<Link
									title="Project Alerts"
									href={projects_path(ProjectSection.ALERTS)}
								>
									Alerts
								</Link>
							</li>
							<li>
								<Link
									title="Project Settings"
									href={projects_path(ProjectSection.SETTINGS)}
								>
									Settings
								</Link>
							</li>
						</ul>
					</>
				</Match>
			</Switch>
			<p class="menu-label">User</p>
			<ul class="menu-list">
				<li>
					<Link title="User API Tokens" href={users_path(UserSection.TOKENS)}>
						API Tokens
					</Link>
				</li>
				<li>
					<Link title="User Settings" href={users_path(UserSection.SETTINGS)}>
						Settings
					</Link>
				</li>
				<li>
					<Link title="Get Help" href={users_path(UserSection.HELP)}>
						Help
					</Link>
				</li>
			</ul>
		</aside>
	);
};

export default ConsoleMenu;
