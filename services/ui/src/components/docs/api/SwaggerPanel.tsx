import { createEffect } from "solid-js";
import swagger from "./swagger.json";
import { SwaggerUIBundle } from "swagger-ui-dist";
import { BENCHER_API_URL } from "../../site/util";

const API_BENCHER_DEV = "https://api.bencher.dev";
const API_URL = BENCHER_API_URL();
const BENCHER_CLOUD = "Bencher Cloud";
const BENCHER_SELF_HOSTED = "Bencher Self-Hosted";

const is_bencher_cloud = API_URL === API_BENCHER_DEV;

const SwaggerPanel = (_props) => {
	createEffect(() => {
		const swagger_spec = swagger;
		// https://swagger.io/docs/specification/api-host-and-base-path/
		swagger_spec.servers = [];
		if (!is_bencher_cloud) {
			swagger_spec.servers.push({
				url: API_URL,
				description: BENCHER_SELF_HOSTED,
			});
		}
		swagger_spec.servers.push({
			url: API_BENCHER_DEV,
			description: BENCHER_CLOUD,
		});
		SwaggerUIBundle({
			dom_id: "#swagger",
			spec: swagger_spec,
			layout: "BaseLayout",
		});
	});

	return (
		<div class="content">
			<blockquote>
				<p>
					🐰 {is_bencher_cloud ? BENCHER_CLOUD : BENCHER_SELF_HOSTED} API
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
