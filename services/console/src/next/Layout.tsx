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
	forgetVersion,
	readShell,
	rememberShell,
	rememberVersion,
} from "./memory";
import { TABS, parseNextPath, tabOf } from "./paths";
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
		host?: string | undefined;
		/** Where the router keeps project paths, under its base. */
		route?: string | undefined;
		signOut?: (back?: boolean) => void;
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

	// The project's own version, fetched on this load, corrects the memory.
	createEffect(() => {
		const version = props.versions()[slug()];
		if (version !== undefined) {
			rememberVersion(localStorage, slug(), version);
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

	const gone = () => failure() === "not_found" || failure() === "forbidden";
	// A project that no longer answers must not stay the reader's landing.
	createEffect(() => {
		if (gone()) {
			forgetVersion(localStorage, slug());
		}
	});

	return (
		<ProjectContext.Provider
			value={{ api: props.api, slug, host: props.host, route: props.route }}
		>
			<Shell
				slug={slug()}
				tab={tab()}
				organization={organization()}
				project={data()?.project.name ?? remembered()?.name}
				alerts={data()?.active_alerts ?? remembered()?.alerts}
				reader={reader()}
				user={props.reader.user}
				signOut={props.signOut}
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
				<Match when={gone()}>
					<ProjectNotFound />
				</Match>
				<Match when={failure()}>
					<main class="page">
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">{slug()} did not load</span>
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

const ProjectNotFound = () => (
	<main class="page">
		<div class="pagehead">
			<Heading level={1} size="xl">
				Project not found
			</Heading>
		</div>
	</main>
);
