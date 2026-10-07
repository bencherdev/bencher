import { type RouteSectionProps, useLocation } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import {
	Match,
	Suspense,
	Switch,
	createEffect,
	createMemo,
	on,
} from "solid-js";
import type { JsonAuthUser } from "../types/bencher";
import { type Api, ApiError } from "./api";
import ContentSkeleton from "./ContentSkeleton";
import {
	type BmfVersion,
	readShell,
	rememberShell,
	rememberVersion,
} from "./memory";
import { TABS, classicHref, parseNextPath, tabOf } from "./paths";
import { ProjectContext } from "./project";
import { consoleProjectQuery } from "./queries";
import { useQueryResult } from "./query";
import Announcer from "./shell/Announcer";
import Shell from "./shell/Shell";
import { centerCurrentTab } from "./shell/scroll";

/** The shell and the page under it, for every path the router owns. */
const Layout = (
	props: RouteSectionProps & {
		api: Api;
		reader: JsonAuthUser;
		versions: () => Record<string, BmfVersion>;
	},
) => {
	const location = useLocation();
	const place = createMemo(() => parseNextPath(location.pathname));
	const slug = () => place()?.slug ?? "";
	const tab = () => tabOf(place()?.rest ?? []);
	const reader = () => props.reader.user.uuid;

	const bootstrap = useQueryResult(() =>
		consoleProjectQuery(props.api, slug()),
	);
	const data = createMemo(() => bootstrap().data);
	const remembered = createMemo(() =>
		readShell(localStorage, reader(), slug()),
	);

	// The project's own version, fetched on this load, corrects the memory, and
	// a version 0 project belongs to the classic console.
	createEffect(() => {
		const version = props.versions()[slug()];
		if (version === undefined) {
			return;
		}
		rememberVersion(localStorage, slug(), version);
		const classic = version === 0 && classicHref(window.location);
		if (classic) {
			window.location.replace(classic);
		}
	});

	createEffect(() => {
		const project = data();
		if (project) {
			rememberShell(localStorage, reader(), slug(), {
				organization: project.organization.name,
				organizationUuid: project.organization.uuid,
				name: project.project.name,
				alerts: project.active_alerts,
			});
		}
	});

	const organization = () => {
		const project = data();
		if (project) {
			return {
				name: project.organization.name,
				uuid: project.organization.uuid,
			};
		}
		const memory = remembered();
		return memory
			? { name: memory.organization, uuid: memory.organizationUuid }
			: undefined;
	};

	const title = createMemo(() => {
		const label = TABS.find((entry) => entry.tab === tab())?.label;
		const name = data()?.project.name ?? remembered()?.name ?? slug();
		return label ? `${label} | ${name} | Bencher` : `${name} | Bencher`;
	});
	createEffect(() => {
		document.title = title();
	});

	let tabs: HTMLElement | undefined;
	// A count arriving widens the Alerts tab and can push the current tab out of view.
	createEffect(
		on([tab, () => data()?.active_alerts], () => centerCurrentTab(tabs)),
	);

	// Data already seen keeps the page up; only a project never loaded shows a failure.
	const failure = () => {
		const { error } = bootstrap();
		return data() || !error
			? undefined
			: error instanceof ApiError
				? error.kind
				: "network";
	};

	return (
		<ProjectContext.Provider value={{ api: props.api, slug }}>
			<Shell
				slug={slug()}
				tab={tab()}
				organization={organization()}
				project={data()?.project.name ?? remembered()?.name}
				alerts={data()?.active_alerts ?? remembered()?.alerts}
				account={`/console/users/${props.reader.user.slug}/settings`}
				tabsRef={(el) => {
					tabs = el;
				}}
			/>
			<Announcer path={location.pathname} title={title()} />
			<Switch
				fallback={
					<Suspense fallback={<ContentSkeleton />}>{props.children}</Suspense>
				}
			>
				<Match when={failure() === "not_found" || failure() === "forbidden"}>
					<ProjectNotFound slug={slug()} />
				</Match>
				<Match when={failure()}>
					<main class="page">
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">
								{slug()} did not load: the Bencher API did not answer.
							</span>
							<Button size="sm" onClick={() => bootstrap().refetch()}>
								Retry
							</Button>
						</Banner>
					</main>
				</Match>
			</Switch>
		</ProjectContext.Provider>
	);
};

export default Layout;

const ProjectNotFound = (props: { slug: string }) => (
	<main class="page">
		<div class="pagehead">
			<Heading level={1} size="xl">
				Project not found
			</Heading>
			<p class="lede">
				There is no project {props.slug} that you can see. It may be private,
				deleted, or mistyped.
			</p>
		</div>
	</main>
);
