import { createEffect } from "solid-js";
import swagger from "./swagger.json";
import { SwaggerUIBundle } from "swagger-ui-dist";

const SwaggerPanel = (props) => {
	createEffect(() => {
		SwaggerUIBundle({
			dom_id: "#swagger",
			spec: swagger,
			layout: "BaseLayout",
		});
	});

	return (
		<div class="content">
			<blockquote>
				<h2 class="title">
					🐰 Bencher Cloud API Endpoint:{" "}
					<code>
						<a
							href="https://api.bencher.dev/v0/server/version"
							target="_blank"
							rel="noreferrer"
						>
							https://api.bencher.dev
						</a>
					</code>
				</h2>
			</blockquote>
			<hr />
			<div id="swagger" />
			<br />
		</div>
	);
};

export default SwaggerPanel;
