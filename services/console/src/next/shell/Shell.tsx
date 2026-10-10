import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import ThemeToggle from "@bencherdev/ui/ThemeToggle";
import { For, Show } from "solid-js";
import { TABS, type Tab, projectPath } from "../paths";
import { THEME_KEY } from "../theme";

interface ShellProps {
	slug: string;
	tab: Tab | undefined;
	/** Unknown until the project loads; the crumb holds its place until then. */
	organization?: { name: string; uuid: string } | undefined;
	project?: string | undefined;
	/** The active alert count, unknown until it loads. */
	alerts?: number | undefined;
	account?: string | undefined;
	tabsRef?: (tabs: HTMLElement) => void;
}

/**
 * The breadcrumb bar and the tab row. Astro renders it with nothing loaded for
 * the first paint, and the app renders it again with what it knows; the
 * `data-shell` hooks are where `ShellPaint.astro` fills in remembered values
 * between the two, so the swap moves nothing.
 */
const Shell = (props: ShellProps) => {
	const alertsLabel = () =>
		props.alerts ? `Alerts, ${props.alerts} active` : "Alerts";
	return (
		<header class="shell">
			<div class="bar">
				<a class="brand" href="/console" aria-label="Bencher, home">
					<Mark />
					<span class="brand-name">Bencher</span>
				</a>
				<nav
					class="crumbs"
					aria-label="Breadcrumb"
					aria-busy={props.organization ? undefined : "true"}
				>
					<Show
						when={props.organization}
						fallback={
							<Skeleton size="text" class="crumb-skel" data-shell="org" />
						}
					>
						{(organization) => (
							<a
								class="crumb-org"
								href={`/console/organizations/${organization().uuid}/projects`}
								data-shell="org"
							>
								{organization().name}
							</a>
						)}
					</Show>
					<span class="sep crumb-org" aria-hidden="true">
						/
					</span>
					<a
						class="projlink"
						href={projectPath(props.slug)}
						data-shell="project"
					>
						{props.project ?? props.slug}
					</a>
				</nav>
				<span class="spacer" />
				<a
					class="bell"
					href={projectPath(props.slug, "alerts")}
					aria-label={alertsLabel()}
					data-shell="bell"
				>
					<Icon name="alerts" />
					<Show when={props.alerts}>
						<span class="badge" aria-hidden="true">
							{props.alerts}
						</span>
					</Show>
				</a>
				<a class="docs" href="/docs/">
					Docs
				</a>
				<ThemeToggle storageKey={THEME_KEY} class="theme-toggle" />
				<a class="avatar" href={props.account ?? "/console"}>
					<span class="sr-only">Account</span>
				</a>
			</div>
			<nav class="tabs" aria-label="Project" ref={(el) => props.tabsRef?.(el)}>
				<For each={TABS}>
					{({ tab, label }) => (
						<>
							<Show when={tab === "settings"}>
								<span class="tabspacer" aria-hidden="true" />
							</Show>
							<a
								class="tab"
								href={projectPath(props.slug, tab)}
								aria-current={tab === props.tab ? "page" : undefined}
								aria-label={
									tab === "alerts" && props.alerts
										? `Alerts ${props.alerts} active`
										: undefined
								}
								data-shell={tab === "alerts" ? "alerts" : undefined}
							>
								{label}
								<Show when={tab === "alerts" && props.alerts}>
									<span class="badge" aria-hidden="true">
										{props.alerts}
									</span>
								</Show>
							</a>
						</>
					)}
				</For>
			</nav>
		</header>
	);
};

export default Shell;

const Mark = () => (
	<svg
		width="28"
		height="28"
		viewBox="0 0 26 26"
		fill="none"
		aria-hidden="true"
	>
		<path
			d="M8 3c1.5 0 2.5 3 2.5 7M18 3c-1.5 0-2.5 3-2.5 7"
			stroke="currentColor"
			stroke-width="2.2"
			stroke-linecap="round"
		/>
		<circle
			cx="13"
			cy="15.5"
			r="7.5"
			stroke="currentColor"
			stroke-width="2.2"
		/>
		<circle cx="10.5" cy="15" r="1.1" class="mark-eye" />
		<circle cx="15.5" cy="15" r="1.1" class="mark-eye" />
	</svg>
);
