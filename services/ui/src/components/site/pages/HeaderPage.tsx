import { createEffect } from "solid-js";
import { pageTitle } from "../util";

const HeaderPage = (props) => {
	createEffect(() => {
		pageTitle(props.page?.title);
	});

	return (
		<section class="section">
			<div class="container">
				<div class="columns is-mobile">
					<div class="column">
						<div class="content">
							<h2 class="title">{props.page?.heading}</h2>
							<hr />
							<br />
							{props.page?.content}
							<br />
						</div>
					</div>
				</div>
			</div>
		</section>
	);
};

export default HeaderPage;
