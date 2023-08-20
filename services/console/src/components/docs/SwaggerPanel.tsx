import { createEffect } from "solid-js";
import { SwaggerUIBundle } from "swagger-ui-dist";
import { SWAGGER, BENCHER_CLOUD_API_URL, isBencherCloud } from "../../util/ext";

const BENCHER_CLOUD = "Bencher Cloud";
const BENCHER_SELF_HOSTED = "Bencher Self-Hosted";

export interface Props {
	apiUrl: string;
}

const SwaggerPanel = (props: Props) => {
	createEffect(() => {
		const swagger = SWAGGER;
		// https://swagger.io/docs/specification/api-host-and-base-path/
		swagger.servers = [];
		if (!isBencherCloud(props.apiUrl)) {
			swagger.servers.push({
				url: props.apiUrl,
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
					🐰{" "}
					{isBencherCloud(props.apiUrl) ? BENCHER_CLOUD : BENCHER_SELF_HOSTED}{" "}
					API Endpoint:{" "}
					<code>
						<a
							href={`${props.apiUrl}/v0/server/version`}
							target="_blank"
							rel="noreferrer"
						>
							{props.apiUrl}
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
