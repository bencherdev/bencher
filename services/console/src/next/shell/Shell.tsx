import Icon from "@bencherdev/ui/Icon";
import { For, Show } from "solid-js";
import { TABS, type Tab, projectPath } from "../paths";
import type { AccountMenuProps } from "./AccountMenu";
import Avatar from "./Avatar";
import Crumb from "./Crumb";
import Feedback from "./Feedback";
import Tools from "./Tools";

interface ShellProps {
	/** The project's slug; an account or organization page's bar has no project. */
	slug?: string | undefined;
	/** An organization's page, whose crumb is the organization alone. */
	org?: boolean | undefined;
	tab?: Tab | undefined;
	/** Unknown until the project loads; the crumb holds its place until then. */
	organization?: { name: string; uuid: string } | undefined;
	project?: string | undefined;
	/** The active alert count, unknown until it loads. */
	alerts?: number | undefined;
	/** The signed in reader, whose projects the crumb's switcher lists. */
	reader?: string | undefined;
	/** The reader's account, unknown to the server's render. */
	user?: AccountMenuProps["user"] | undefined;
	signOut?: ((back?: boolean) => void) | undefined;
	tabsRef?: (tabs: HTMLElement) => void;
}

/**
 * The breadcrumb bar and the tab row. Astro renders it with nothing loaded for
 * the first paint, and the app renders it again with what it knows; the
 * `data-shell` hooks are where `ShellPaint.astro` fills in remembered values
 * between the two, so the swap moves nothing. The Alerts tab's count sits
 * inline, after a space that never draws, so its text reads "Alerts 3", as its
 * name does.
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
					<img src="/wordmark-dark.svg" alt="" />
					<img src="/wordmark-light.svg" alt="" />
				</a>
				<Show
					when={props.slug || props.org}
					fallback={<span class="crumbtext">Account</span>}
				>
					<Crumb
						slug={props.slug ?? ""}
						organization={props.organization}
						project={props.slug && (props.project ?? props.slug)}
						reader={props.reader}
					/>
				</Show>
				<span class="spacer" />
				<Show when={props.slug}>
					{(slug) => (
						<a
							class="bell"
							href={projectPath(slug(), "alerts")}
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
					)}
				</Show>
				<Tools>
					<Feedback
						about={() => ({
							project: props.organization
								? `${props.organization.name} / ${props.project ?? props.slug}`
								: (props.project ?? props.slug),
							slug: props.slug,
							email: props.user?.email ?? "",
						})}
					/>
				</Tools>
				<Avatar {...props} />
			</div>
			<Show when={props.slug}>
				{(slug) => (
					<nav
						class="tabs"
						aria-label="Project"
						ref={(el) => props.tabsRef?.(el)}
					>
						<For each={TABS}>
							{({ tab, label }) => (
								<>
									<Show when={tab === "settings"}>
										<span class="tabspacer" aria-hidden="true" />
									</Show>
									<a
										class="tab"
										href={projectPath(slug(), tab)}
										aria-current={tab === props.tab ? "page" : undefined}
										aria-label={
											tab === "alerts" && props.alerts
												? `Alerts ${count()} active`
												: undefined
										}
										data-shell={tab === "alerts" ? "alerts" : undefined}
									>
										{label}
										<Show when={tab === "alerts" && props.alerts}>
											{" "}
											<span class="badge" aria-hidden="true">
												{count()}
											</span>
										</Show>
									</a>
								</>
							)}
						</For>
					</nav>
				)}
			</Show>
		</header>
	);
};

export default Shell;
