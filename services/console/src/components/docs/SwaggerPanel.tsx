import { createEffect } from "solid-js";
import { SwaggerUIBundle } from "swagger-ui-dist";
import {
	BENCHER_API_URL,
	SWAGGER,
	BENCHER_CLOUD_API_URL,
	isBencherCloud,
} from "../../util/ext";

const BENCHER_CLOUD = "Bencher Cloud";
const BENCHER_SELF_HOSTED = "Bencher Self-Hosted";

const SwaggerPanel = (_props) => {
	const API_URL = BENCHER_API_URL();

	createEffect(() => {
		const swagger = SWAGGER;
		// https://swagger.io/docs/specification/api-host-and-base-path/
		swagger.servers = [];
		if (!isBencherCloud()) {
			swagger.servers.push({
				url: API_URL,
				description: BENCHER_SELF_HOSTED,
			});
		}
		swagger.servers.push({
			url: BENCHER_CLOUD_API_URL,
			description: BENCHER_CLOUD,
		});
		SwaggerUIBundle({
			dom_id: "#swagger",
			spec: swagger,
			layout: "BaseLayout",
		});
	});

	return (
		<div class="content">
			<blockquote>
				<p>
					🐰 {isBencherCloud() ? BENCHER_CLOUD : BENCHER_SELF_HOSTED} API
					Endpoint:{" "}
					<code>
						<a
							href={`${API_URL}/v0/server/version`}
							target="_blank"
							rel="noreferrer"
						>
							{API_URL}
						</a>
					</code>
				</p>
			</blockquote>
			<hr />
			<div id="swagger" />
			<br />
		</div>
	);
};

export default SwaggerPanel;
