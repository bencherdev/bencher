import createDebounce from "@solid-primitives/debounce";
import { createEffect, createMemo, createSignal } from "solid-js";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { pageTitle } from "../../../site/util";

const PerfHeader = (props) => {
	const [share, set_share] = createSignal(false);

	const project = createMemo(() => props.perf_data()?.project);

	createEffect(() => {
		pageTitle(project()?.name);
	});

	return (
		<div class="columns is-vcentered">
			<div class="column">
				<h3 class="title is-3" style="overflow-wrap:break-word;">
					{project()?.name}
				</h3>
			</div>
			<ShareModal
				user={props.user}
				config={props.config}
				perf_query={props.perf_query}
				isPlotInit={props.isPlotInit}
				project={project}
				share={share}
				set_share={set_share}
			/>
			<div class="column is-narrow">
				<nav class="level">
					<div class="level-right">
						{project()?.url && (
							<div class="level-item">
								<a
									class="button is-outlined is-fullwidth"
									href={project()?.url}
									rel="noreferrer nofollow"
									target="_blank"
								>
									<span class="icon">
										<i class="fas fa-globe" aria-hidden="true" />
									</span>
									<span>Website</span>
								</a>
							</div>
						)}
						<nav class="level is-mobile">
							{project()?.visibility === "public" && !props.isPlotInit() && (
								<div class="level-item">
									<button
										class="button is-outlined is-fullwidth"
										onClick={(e) => {
											e.preventDefault();
											set_share(true);
										}}
									>
										<span class="icon">
											<i class="fas fa-share" aria-hidden="true" />
										</span>
										<span>Share</span>
									</button>
								</div>
							)}
							<div class="level-item">
								<button
									class="button is-outlined is-fullwidth"
									onClick={(e) => {
										e.preventDefault();
										props.handleRefresh();
									}}
								>
									<span class="icon">
										<i class="fas fa-sync-alt" aria-hidden="true" />
									</span>
									<span>Refresh</span>
								</button>
							</div>
						</nav>
					</div>
				</nav>
			</div>
		</div>
	);
};

export default PerfHeader;

const ShareModal = (props) => {
	const location = window.location;

	const [title, set_title] = createSignal(null);

	const handle_title = createDebounce(
		(_key, value, _valid) => set_title(value),
		250,
	);

	const perf_page_url = createMemo(
		() =>
			`${location.protocol}//${location.hostname}${
				location.port ? `:${location.port}` : ""
			}/perf/${props.project()?.slug}${location.search}`,
	);

	const perf_img_url = createMemo(() => {
		if (
			props.isPlotInit() ||
			!(props.share() && props.project()?.slug && props.perf_query())
		) {
			return null;
		}

		const search_params = new URLSearchParams();
		for (const [key, value] of Object.entries(props.perf_query())) {
			if (value) {
				search_params.set(key, value);
			}
		}
		if (title()) {
			search_params.set("title", title());
		}
		return `${props.config?.url(
			props.project()?.slug,
		)}?${search_params.toString()}`;
	});

	const img_tag = createMemo(
		() =>
			`<a href="${perf_page_url()}"><img src="${perf_img_url()}" title="${
				title() ? title() : props.project()?.name
			}" alt="${title() ? `${title()} for ` : ""}${
				props.project()?.name
			} - Bencher" /></a>`,
	);

	return (
		<div class={`modal ${props.share() && "is-active"}`}>
			<div class="modal-background" />
			<div class="modal-card">
				<header class="modal-card-head">
					<p class="modal-card-title">Share {props.project()?.name}</p>
					<button
						class="delete"
						aria-label="close"
						onClick={(e) => {
							e.preventDefault();
							props.set_share(false);
						}}
					/>
				</header>
				<section class="modal-card-body">
					<Field
						kind={FieldKind.INPUT}
						fieldKey="title"
						label="Title (optional)"
						value={title()}
						valid={true}
						config={{
							type: "text",
							placeholder: props.project()?.name,
							icon: "fas fa-chart-line",
							help: null,
							validate: (_input) => true,
						}}
						handleField={handle_title}
					/>
					<br />
					{perf_img_url() ? (
						<img src={perf_img_url()} alt={props.project()?.name} />
					) : (
						<p>Loading...</p>
					)}
					<br />
					<br />
					<h4 class="title is-4">
						Click to Copy <code>img</code> Tag
					</h4>
					{/* rome-ignore lint/a11y/useValidAnchor: Copy tag */}
					<a
						href=""
						onClick={(e) => {
							e.preventDefault();
							navigator.clipboard.writeText(img_tag());
						}}
					>
						<code>{img_tag()}</code>
					</a>
					<br />
					<br />
					<blockquote>🐰 Add me to your README!</blockquote>

					<div class="is-divider" data-content="OR" />

					<h4 class="title is-4">Click to Copy URL</h4>
					{/* rome-ignore lint/a11y/useValidAnchor: Copy link */}
					<a
						href=""
						onClick={(e) => {
							e.preventDefault();
							navigator.clipboard.writeText(perf_page_url());
						}}
					>
						{perf_page_url()}
					</a>
				</section>
				<footer class="modal-card-foot">
					<button
						class="button is-primary is-outlined is-fullwidth"
						onClick={(e) => {
							e.preventDefault();
							props.set_share(false);
						}}
					>
						Close
					</button>
				</footer>
			</div>
		</div>
	);
};
