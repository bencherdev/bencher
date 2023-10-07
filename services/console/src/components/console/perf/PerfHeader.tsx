import { debounce } from "@solid-primitives/scheduled";
import {
	type Accessor,
	createEffect,
	createMemo,
	createSignal,
	type Resource,
	Show,
} from "solid-js";
import type {
	JsonAuthUser,
	JsonPerf,
	JsonProject,
} from "../../../types/bencher";
import { setPageTitle } from "../../../util/resource";
import { apiUrl } from "../../../util/http";
import FieldKind from "../../field/kind";
import Field from "../../field/Field";

export interface Props {
	apiUrl: string;
	isConsole: boolean;
	user: JsonAuthUser;
	perfData: Resource<JsonPerf>;
	isPlotInit: Accessor<boolean>;
	perfQuery: Accessor<PerfQuery>;
	handleRefresh: () => void;
}

export interface PerfQuery {
	metric_kind: undefined | string;
	branches: string[];
	testbeds: string[];
	benchmarks: string[];
	start_time: undefined | string;
	end_time: undefined | string;
}

const PerfHeader = (props: Props) => {
	const [share, setShare] = createSignal(false);

	const project = createMemo(() => props.perfData()?.project);

	createEffect(() => {
		if (props.isConsole) {
			setPageTitle(project()?.name);
		}
	});

	return (
		<div class="columns is-centered">
			<div class="column">
				<h3 class="title is-3" style="overflow-wrap:anywhere;">
					{project()?.name}
				</h3>
			</div>
			<ShareModal
				apiUrl={props.apiUrl}
				user={props.user}
				perfQuery={props.perfQuery}
				isPlotInit={props.isPlotInit}
				project={project}
				share={share}
				setShare={setShare}
			/>
			<div class="column is-narrow">
				<nav class="level">
					<div class="level-right">
						<Show when={project()?.url} fallback={<></>}>
							<div class="level-item">
								<a
									class="button is-outlined is-fullwidth"
									title={`View ${project()?.name} website`}
									href={project()?.url ?? ""}
									rel="noreferrer nofollow"
									target="_blank"
								>
									<span class="icon">
										<i class="fas fa-globe" aria-hidden="true" />
									</span>
									<span>Website</span>
								</a>
							</div>
						</Show>
						<Show when={!props.isPlotInit()} fallback={<></>}>
							<nav class="level is-mobile">
								<Show
									when={project()?.visibility === "public"}
									fallback={<></>}
								>
									<div class="level-item">
										<button
											class="button is-outlined is-fullwidth"
											title={`Share ${project()?.name}`}
											onClick={(e) => {
												e.preventDefault();
												setShare(true);
											}}
										>
											<span class="icon">
												<i class="fas fa-share" aria-hidden="true" />
											</span>
											<span>Share</span>
										</button>
									</div>
								</Show>

								<div class="level-item">
									<button
										class="button is-outlined is-fullwidth"
										title="Refresh Query"
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
						</Show>
					</div>
				</nav>
			</div>
		</div>
	);
};

export default PerfHeader;

export interface ShareProps {
	apiUrl: string;
	user: JsonAuthUser;
	perfQuery: Accessor<PerfQuery>;
	isPlotInit: Accessor<boolean>;
	project: Accessor<undefined | JsonProject>;
	share: Accessor<boolean>;
	setShare: (share: boolean) => void;
}

const ShareModal = (props: ShareProps) => {
	const location = window.location;

	const [title, setTitle] = createSignal(null);

	const handle_title = debounce((_key, value, _valid) => setTitle(value), 250);

	const perf_page_url = createMemo(
		() =>
			`${location.protocol}//${location.hostname}${
				location.port ? `:${location.port}` : ""
			}/perf/${props.project()?.slug}${location.search}`,
	);

	const perf_img_url = createMemo(() => {
		const project_slug = props.project()?.slug;
		if (
			props.isPlotInit() ||
			!(props.share() && project_slug && props.perfQuery())
		) {
			return null;
		}

		const searchParams = new URLSearchParams();
		for (const [key, value] of Object.entries(props.perfQuery())) {
			if (value) {
				searchParams.set(key, value);
			}
		}
		const img_title = title();
		if (img_title) {
			searchParams.set("title", img_title);
		}
		const url = apiUrl(
			props.apiUrl,
			`/v0/projects/${project_slug}/perf/img?${searchParams.toString()}`,
		);
		return url;
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
							props.setShare(false);
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
							validate: (_input: string) => true,
						}}
						handleField={handle_title}
					/>
					<br />
					<Show when={perf_img_url()} fallback={<div>Loading...</div>}>
						<img src={perf_img_url() ?? ""} alt={props.project()?.name ?? ""} />
					</Show>
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
							props.setShare(false);
						}}
					>
						Close
					</button>
				</footer>
			</div>
		</div>
	);
};
