import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import ThemeToggle from "@bencherdev/ui/ThemeToggle";
import { For, Show } from "solid-js";
import { TABS, type Tab, projectPath } from "../paths";
import { THEME_KEY } from "../theme";
import Mark from "./Mark";

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
 * between the two, so the swap moves nothing. The Alerts tab holds its badge's
 * place with or without a count, and a space that never draws comes before
 * each badge, so the tab's text reads "Alerts 3", as its name does.
 */
const Shell = (props: ShellProps) => {
	/** The count a badge shows, two digits at most. */
	const count = () =>
		props.alerts ? (props.alerts > 99 ? "99+" : String(props.alerts)) : "";
	// On a phone the bell shows only its count, so its name starts there.
	const alertsLabel = () =>
		props.alerts
			? `${count()} active ${props.alerts === 1 ? "alert" : "alerts"}`
			: "Alerts";
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
						{" "}
						<span class="badge" aria-hidden="true">
							{count()}
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
										? `Alerts ${count()} active`
										: undefined
								}
								data-shell={tab === "alerts" ? "alerts" : undefined}
							>
								{label}
								<Show when={tab === "alerts"}>
									{" "}
									<span
										class="badge"
										aria-hidden="true"
										data-shell="badge"
										data-empty={props.alerts ? undefined : ""}
									>
										{count()}
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
