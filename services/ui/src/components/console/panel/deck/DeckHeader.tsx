import { useLocation, useNavigate } from "solid-app-router";
import { createEffect, createMemo } from "solid-js";
import { pageTitle } from "../../../site/util";

const DeckHeader = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	createEffect(() => {
		pageTitle(props.data?.[props.config?.key]);
	});

	return (
		<div class="columns is-centered">
			<div class="column is-narrow">
				<button
					class="button is-outlined is-fullwidth"
					onClick={(e) => {
						e.preventDefault();
						navigate(props.config?.path(pathname()));
					}}
				>
					<span class="icon">
						<i class="fas fa-chevron-left" aria-hidden="true" />
					</span>
					<span>Back</span>
				</button>
			</div>
			<div class="column">
				<div class="content has-text-centered">
					<h3 class="title is-3" style="overflow-wrap:anywhere;">
						{props.data?.[props.config?.key]}
					</h3>
				</div>
			</div>
			<div class="column is-narrow">
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
		</div>
	);
};

export default DeckHeader;
