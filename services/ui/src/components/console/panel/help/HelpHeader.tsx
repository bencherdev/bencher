import { useLocation, useNavigate } from "solid-app-router";
import { createEffect, createMemo } from "solid-js";
import { pageTitle } from "../../../site/util";

const HelpHeader = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	createEffect(() => {
		pageTitle(props.config?.title);
	});

	return (
		<nav class="level">
			<div class="level-left">
				<button
					class="button is-outlined"
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
			<div class="level-left">
				<div class="level-item">
					<h3 class="title is-3">{props.config?.title}</h3>
				</div>
			</div>

			<div class="level-right" />
		</nav>
	);
};

export default HelpHeader;
