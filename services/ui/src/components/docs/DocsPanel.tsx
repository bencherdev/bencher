import { Match, Switch } from "solid-js";

import SwaggerPanel from "./api/SwaggerPanel";
import PageKind from "./config/page_kind";

const DocsPanel = (props) => {
	return (
		<Switch fallback={<DocPanel panel={props.panel} />}>
			<Match when={props.panel?.kind === PageKind.SWAGGER}>
				<SwaggerPanel />
			</Match>
		</Switch>
	);
};

const DocPanel = (props) => {
	return (
		<div class="content">
			<h1 class="title">{props.panel?.heading}</h1>
			<hr />
			{props.panel?.content}
			<br />
		</div>
	);
};

export default DocsPanel;
