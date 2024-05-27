import { debounce } from "@solid-primitives/scheduled";
import {
	type Accessor,
	type Resource,
	Show,
	createEffect,
	createMemo,
	createSignal,
} from "solid-js";
import { embedHeight } from "../../../../config/types";
import {
	type JsonAuthUser,
	type JsonPerfQuery,
	type JsonProject,
	Visibility,
} from "../../../../types/bencher";
import { apiUrl } from "../../../../util/http";
import { setPageTitle } from "../../../../util/resource";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { DEBOUNCE_DELAY } from "../../../../util/valid";
import { useSearchParams } from "../../../../util/url";
import {
	EMBED_TITLE_PARAM,
	PERF_PLOT_EMBED_PARAMS,
	PERF_PLOT_PARAMS,
} from "../PerfPanel";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	perfQuery: Accessor<JsonPerfQuery>;
	isPlotInit: Accessor<boolean>;
	project: Accessor<undefined | JsonProject>;
	share: Accessor<boolean>;
	setShare: (share: boolean) => void;
}

const ShareModal = (props: Props) => {
	const location = window.location;
	const [searchParams, _setSearchParams] = useSearchParams();

	const [title, setTitle] = createSignal(null);

	const handle_title = debounce(
		(_key, value, _valid) => setTitle(value),
		DEBOUNCE_DELAY,
	);

	const perfPlotParams = createMemo(() => {
		const newParams = new URLSearchParams();
		for (const [key, value] of Object.entries(searchParams)) {
			if (value && PERF_PLOT_PARAMS.includes(key)) {
				newParams.set(key, value);
			}
		}
		return newParams.toString();
	});

	const perf_page_url = createMemo(
		() =>
			`${location.protocol}//${location.hostname}${
				location.port ? `:${location.port}` : ""
			}/perf/${props.project()?.slug}?${perfPlotParams()}`,
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

	const perfPlotEmbedParams = createMemo(() => {
		const newParams = new URLSearchParams();
		for (const [key, value] of Object.entries(searchParams)) {
			if (value && PERF_PLOT_EMBED_PARAMS.includes(key)) {
				newParams.set(key, value);
			}
		}
		const img_title = title();
		if (img_title) {
			newParams.set(EMBED_TITLE_PARAM, img_title);
		}
		return newParams.toString();
	});

	const perf_embed_url = createMemo(
		() =>
			`${location.protocol}//${location.hostname}${
				location.port ? `:${location.port}` : ""
			}/perf/${props.project()?.slug}/embed?${perfPlotEmbedParams()}`,
	);

	const img_tag = createMemo(
		() =>
			`<a href="${perf_page_url()}"><img src="${perf_img_url()}" title="${
				title() ? title() : props.project()?.name
			}" alt="${title() ? `${title()} for ` : ""}${
				props.project()?.name
			} - Bencher" /></a>`,
	);

	const embed_tag = createMemo(
		() =>
			`<iframe src="${perf_embed_url()}" title="${
				title() ? title() : props.project()?.name
			}" width="100%" height="${embedHeight}px" allow="fullscreen"></iframe>`,
	);

	return (
		<div class={`modal ${props.share() && "is-active"}`}>
			<div
				class="modal-background"
				onClick={(e) => {
					e.preventDefault();
					props.setShare(false);
				}}
				onKeyDown={(e) => {
					e.preventDefault();
					props.setShare(false);
				}}
			/>
			<div class="modal-card">
				<header class="modal-card-head">
					<p class="modal-card-title">Share {props.project()?.name}</p>
					<button
						class="delete"
						type="button"
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
					{/* biome-ignore lint/a11y/useValidAnchor: Copy tag */}
					<a
						style="word-break: break-all;"
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

					<hr />

					<h4 class="title is-4">Embed Perf Plot</h4>
					<h4 class="subtitle is-4">Click to Copy Embed Tag</h4>
					{/* biome-ignore lint/a11y/useValidAnchor: Copy link */}
					<a
						style="word-break: break-all;"
						href=""
						onClick={(e) => {
							e.preventDefault();
							navigator.clipboard.writeText(embed_tag());
						}}
					>
						{embed_tag()}
					</a>

					<hr />

					<h4 class="title is-4">Click to Copy Public URL</h4>
					{/* biome-ignore lint/a11y/useValidAnchor: Copy link */}
					<a
						style="word-break: break-all;"
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
						class="button is-primary is-fullwidth"
						type="button"
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

export default ShareModal;
