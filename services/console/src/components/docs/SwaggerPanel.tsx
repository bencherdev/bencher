import { createEffect, createSignal } from "solid-js";
import { SwaggerUIBundle } from "swagger-ui-dist";
import {
	BENCHER_CLOUD,
	BENCHER_SELF_HOSTED,
	isBencherCloud,
	swaggerSpec,
} from "../../util/ext";

export interface Props {
	apiUrl: string;
}

const SwaggerPanel = (props: Props) => {
	const [url, setUrl] = createSignal("");

	createEffect(() => {
		const [url, swagger] = swaggerSpec(props.apiUrl);
		setUrl(url);
		SwaggerUIBundle({
			dom_id: "#swagger",
			spec: swagger,
			layout: "BaseLayout",
		});
	});

	return (
		<div class="content">
			<blockquote>
				<nav class="level">
					<div class="level-left">
						<div class="level-item">
							<p>
								🐰 {isBencherCloud(url()) ? BENCHER_CLOUD : BENCHER_SELF_HOSTED}{" "}
								API Endpoint:{" "}
								<code>
									<a
										href={`${url()}/v0/server/version`}
										target="_blank"
										rel="noreferrer"
									>
										{url()}
									</a>
								</code>
							</p>
						</div>
					</div>

					<div class="level-right">
						<button class="button">View OpenAPI Spec</button>
					</div>
				</nav>
			</blockquote>
			<hr />
			<div id="swagger" />
			<br />
		</div>
	);
};

export default SwaggerPanel;
