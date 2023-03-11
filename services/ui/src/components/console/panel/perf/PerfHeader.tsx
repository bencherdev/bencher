import axios from "axios";
import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { get_options, pageTitle } from "../../../site/util";
import token from "../../config/resources/fields/token";

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
				project={project}
				perf_query_string={props.perf_query_string}
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
							<div class="level-item">
								<button
									class="button is-outlined is-fullwidth"
									onClick={(e) => {
										e.preventDefault();
										set_share(true);
										navigator.clipboard.writeText(window.location.href);
									}}
								>
									<span class="icon">
										<i class="fas fa-share" aria-hidden="true" />
									</span>
									<span>Share</span>
								</button>
							</div>
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
	const perf_img = createMemo(() => {
		if (!(props.project()?.slug && props.perf_query_string())) {
			return null;
		}

		return `${props.config?.url(
			props.project()?.slug,
		)}?${props.perf_query_string()}`;
	});

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
					{perf_img() ? (
						<img src={perf_img()} alt={props.project()?.name} />
					) : (
						<p>Loading...</p>
					)}
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
