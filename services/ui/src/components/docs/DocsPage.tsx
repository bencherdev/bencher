import { createEffect, For } from "solid-js";
import { pageTitle } from "../site/util";
import DocsMenu from "./DocsMenu";
import DocsPanel from "./DocsPanel";

const DocsPage = (props) => {
	createEffect(() => {
		pageTitle(props.page?.title);
	});

	return (
		<section class="section">
			<div class="container">
				<div class="columns is-reverse-mobile">
					<div class="column is-narrow">
						<DocsMenu />
					</div>
					<div class="column is-10">
						<DocsPanel panel={props.page?.panel} />
						<For each={[...Array(3).keys()]}>{(_k, _i) => <br />}</For>
						<hr />
					</div>
				</div>
			</div>
		</section>
	);
};

export default DocsPage;
