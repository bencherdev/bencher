import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import { type JSX, Show } from "solid-js";

export interface Confirmation {
	/** What happened, such as "Pinned to the top of Plots as". */
	text: string;
	title: string;
	window: string;
}

const ShareIcon = () => (
	<svg
		viewBox="0 0 24 24"
		width="14"
		height="14"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d="M4 12v7a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-7" />
		<path d="M12 3v12" />
		<path d="M7 8l5-5 5 5" />
	</svg>
);

const PinIcon = () => (
	<svg
		viewBox="0 0 24 24"
		width="14"
		height="14"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d="M9 4h6l-1 6 3 3v2H7v-2l3-3z" />
		<path d="M12 15v6" />
	</svg>
);

/** What the query is, an unsaved plot or a pin, and what the reader can do with it. */
const Header = (props: {
	/** The pin the query edits, by its title; none for an unsaved plot. */
	pinned: string | undefined;
	plotsHref: string;
	dirty: boolean;
	/** Where an unsaved plot came from, under its eyebrow. */
	source: JSX.Element;
	/** The shell has said what the reader may do, so the actions draw once. */
	ready: boolean;
	/** The reader may pin and save. */
	canPin: boolean;
	hasLines: boolean;
	working: boolean;
	shared: boolean;
	confirmation: Confirmation | undefined;
	onShare: () => void;
	onPin: () => void;
	onUndo: () => void;
	onDiscard: () => void;
	onSave: () => void;
	onSaveNew: () => void;
}) => {
	const Share = () => (
		<Button
			size="sm"
			disabled={!props.hasLines}
			onClick={() => props.onShare()}
		>
			<ShareIcon />
			{props.shared ? "Link copied" : "Share"}
		</Button>
	);
	return (
		<>
			<div class="ex-head">
				<Show
					when={props.pinned}
					fallback={
						<div class="ex-title">
							<h1 class="eyebrow ex-eyebrow">Unsaved plot</h1>
							<span class="muted sm">{props.source}</span>
						</div>
					}
				>
					{(title) => (
						<div class="ex-title">
							<div class="crumb-line">
								<a href={props.plotsHref}>Plots</a>
								<span aria-hidden="true">/</span>
							</div>
							<Heading level={1} size="lg">
								{title()}
							</Heading>
							<Show when={props.dirty}>
								<span class="ex-pill">Unsaved changes</span>
							</Show>
						</div>
					)}
				</Show>
				<span class="spacer" />
				<div class="btnrow ex-actions">
					<Show when={props.ready}>
						<Show
							when={props.pinned}
							fallback={
								<>
									<Share />
									<Show when={props.canPin}>
										<Button
											size="sm"
											variant="primary"
											disabled={!props.hasLines || props.working}
											onClick={() => props.onPin()}
										>
											<PinIcon />
											Pin to Plots
										</Button>
									</Show>
								</>
							}
						>
							<Show when={props.canPin}>
								<Button
									size="sm"
									disabled={!props.dirty || props.working}
									onClick={() => props.onDiscard()}
								>
									Discard
								</Button>
								<Button
									size="sm"
									disabled={!props.hasLines || props.working}
									onClick={() => props.onSaveNew()}
								>
									Save as new
								</Button>
								<Button
									size="sm"
									variant="primary"
									disabled={!props.dirty || props.working}
									onClick={() => props.onSave()}
								>
									Save
								</Button>
							</Show>
							<Share />
						</Show>
					</Show>
				</div>
			</div>
			<Show when={props.confirmation}>
				{(confirmation) => (
					<p class="ex-confirm" role="status">
						<svg
							viewBox="0 0 24 24"
							width="14"
							height="14"
							fill="none"
							stroke="currentColor"
							stroke-width="2.6"
							stroke-linecap="round"
							stroke-linejoin="round"
							aria-hidden="true"
						>
							<path d="M5 12.5l4.5 4.5L19 7.5" />
						</svg>
						<span class="grow">
							{confirmation().text} <b>"{confirmation().title}"</b>,{" "}
							{confirmation().window} rolling, with its hidden and focused
							lines.
						</span>
						<button type="button" class="lnk" onClick={() => props.onUndo()}>
							Undo
						</button>
						<a class="lnk" href={props.plotsHref}>
							Open Plots
						</a>
					</p>
				)}
			</Show>
		</>
	);
};

export default Header;
