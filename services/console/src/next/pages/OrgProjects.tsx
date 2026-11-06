import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Skeleton from "@bencherdev/ui/Skeleton";
import Table from "@bencherdev/ui/Table";
import Truncate from "@bencherdev/ui/Truncate";
import { useParams } from "@solidjs/router";
import { For, Match, Show, Switch, createMemo } from "solid-js";
import { useAccount } from "../account/context";
import { Spacer } from "../list/TableRows";
import { useVirtualRows } from "../list/virtual";
import {
	type ProjectRow,
	endpointRows,
	organizationProjectsQuery,
	projectSource,
} from "../organization/projects";
import { ORGANIZATIONS_PATH, projectPath } from "../paths";
import { formatCount } from "../plot/count";
import { consoleUserQuery } from "../queries";
import { useQueryResult } from "../query";
import { relativeTime } from "../reports/format";
import {
	Frame,
	OrganizationNotFound,
	organizationIn,
	organizationQuery,
	refused,
} from "./OrgGeneral";

const counted = (count: number, one: string, many: string) =>
	`${formatCount(count)} ${count === 1 ? one : many}`;

/** The organization's projects, each leading to its Runs. */
const OrgProjects = () => {
	const { api, reader } = useAccount();
	const params = useParams();
	const key = () => params.organization ?? "";
	// The bar asks for it too, so this answer is shared.
	const organization = useQueryResult(() => organizationQuery(api, key()));
	const bootstrap = useQueryResult(() => consoleUserQuery(api, reader().uuid));
	const source = createMemo(() => projectSource(bootstrap().data, key()));
	// Only for an organization a current bootstrap lacks, once the API has shown it to the reader.
	const listed = useQueryResult(() => ({
		...organizationProjectsQuery(api, key()),
		enabled:
			source()?.from === "endpoint" &&
			!bootstrap().isFetching &&
			organization().isSuccess,
	}));
	const rows = createMemo(() => {
		const from = source();
		if (from?.from === "bootstrap") {
			return from.rows;
		}
		const projects = listed().data;
		return from && projects ? endpointRows(projects) : undefined;
	});
	const unseen = () =>
		source()?.from === "endpoint" &&
		(refused(organization().error) || refused(listed().error));
	const failure = () =>
		bootstrap().error ??
		(source()?.from === "endpoint"
			? (organization().error ?? listed().error)
			: undefined);
	const name = () =>
		organization().data?.name ?? organizationIn(bootstrap().data, key())?.name;
	return (
		<Frame organization={key()} name={name()} section="projects">
			<div class="sethead">
				<div class="ph-title">
					<div class="crumb-line">
						Settings<span aria-hidden="true">/</span>
					</div>
					<Heading level={1} size="xl">
						Projects
					</Heading>
					<p class="ph-sub">
						{rows()
							? counted(rows()?.length ?? 0, "project", "projects")
							: "\u00a0"}
					</p>
				</div>
				<Show when={source()?.from === "bootstrap"}>
					<div class="ph-actions">
						<a
							class="ui-button"
							data-variant="secondary"
							data-size="md"
							href={`${ORGANIZATIONS_PATH}/${key()}/projects/add`}
						>
							New project
						</a>
					</div>
				</Show>
			</div>
			<Switch fallback={<Skeleton size="card" />}>
				<Match when={rows()}>
					{(shown) => (
						<Show
							when={shown().length > 0}
							fallback={
								<div class="setcard">
									<p class="keys-empty">No projects</p>
								</div>
							}
						>
							<ProjectTable projects={shown()} />
						</Show>
					)}
				</Match>
				<Match when={unseen()}>
					<OrganizationNotFound />
				</Match>
				<Match when={failure()}>
					<Banner status="error" role="alert" class="load-error">
						<span class="grow">Projects did not load</span>
						<Button
							size="sm"
							onClick={() => {
								for (const query of [bootstrap(), organization(), listed()]) {
									if (query.error) {
										query.refetch();
									}
								}
							}}
						>
							Retry
						</Button>
					</Banner>
				</Match>
			</Switch>
		</Frame>
	);
};

export default OrgProjects;

/** The organization's projects, drawing only the rows on screen. */
const ProjectTable = (props: { projects: ProjectRow[] }) => {
	const rows = useVirtualRows({ rows: () => props.projects });
	const now = Date.now();
	return (
		<Table
			fold={328}
			aria-label="Projects"
			aria-rowcount={props.projects.length + 1}
			onLines={rows.onLines}
		>
			<thead>
				<tr aria-rowindex={1}>
					<th scope="col">Project</th>
					<th scope="col">Last run</th>
					<th scope="col" data-align="end">
						Alerts
					</th>
				</tr>
			</thead>
			<tbody ref={rows.body}>
				<Spacer height={rows.above()} columns={3} />
				<For each={rows.shown()}>
					{(project, index) => (
						<tr class="rowlink" aria-rowindex={rows.start() + index() + 2}>
							<td data-fold="l1" class="ui-table-name">
								<span class="ui-parts">
									<a
										class="namelink rowlink-name"
										href={projectPath(project.slug)}
									>
										<Truncate text={project.name} />
									</a>
								</span>
							</td>
							<td data-fold="l2" class="muted">
								{project.last_report === undefined
									? ""
									: project.last_report === null
										? "No runs"
										: relativeTime(project.last_report, now)}
							</td>
							<td data-fold="n1" data-align="end">
								<Show
									when={project.active_alerts}
									fallback={
										<Show when={project.active_alerts === 0}>
											<span class="muted">No alerts</span>
										</Show>
									}
								>
									{(alerts) => (
										<a class="lnk" href={projectPath(project.slug, "alerts")}>
											{counted(alerts(), "alert", "alerts")}
										</a>
									)}
								</Show>
							</td>
						</tr>
					)}
				</For>
				<Spacer height={rows.below()} columns={3} />
			</tbody>
		</Table>
	);
};
