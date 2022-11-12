import { createEffect } from "solid-js";
import SwaggerUI from "swagger-ui";
import swagger from "./swagger.json";

const SwaggerPanel = (props) => {
  createEffect(() => {
    SwaggerUI({
      dom_id: "#swagger",
      spec: swagger,
      layout: "BaseLayout",
    });
  });

  return <></>;
};

export default SwaggerPanel;
