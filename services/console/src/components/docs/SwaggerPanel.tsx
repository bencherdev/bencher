import { createEffect, createSignal } from "solid-js";
import { SwaggerUIBundle } from "swagger-ui-dist";
import {
	BENCHER_CLOUD,
	BENCHER_SELF_HOSTED,
	isBencherCloud,
	swaggerSpec,
	BENCHER_VERSION,
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
								🐰 {isBencherCloud() ? BENCHER_CLOUD : BENCHER_SELF_HOSTED} API
								Endpoint:{" "}
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
						<a
							class="button is-fullwidth"
							href={`${url()}/v0/server/spec`}
							target="_blank"
							rel="noreferrer"
						>
							View OpenAPI Spec
						</a>
					</div>
				</nav>
			</blockquote>
			<h2>🦀 Rust Client</h2>
			<p>
				If you're writing in Rust consider using the Bencher Rust API Client.
			</p>
			<code>
				bencher_client = {"{"} git = "https://github.com/bencherdev/bencher",
				tag = "v{BENCHER_VERSION}" {"}"}
			</code>
			<hr />
			<div id="swagger" />
			<br />
		</div>
	);
};

export default SwaggerPanel;
