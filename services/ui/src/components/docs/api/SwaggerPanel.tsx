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

	return <div id="swagger" />;
};

export default SwaggerPanel;
