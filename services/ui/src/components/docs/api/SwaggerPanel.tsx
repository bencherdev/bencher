import { createEffect } from "solid-js";
import SwaggerUI from "swagger-ui";
import swagger from "./swagger.json";
import { SwaggerUIBundle } from "swagger-ui-dist";

const SwaggerPanel = (props) => {
  createEffect(() => {
    // const x = SwaggerUI({
    //   // dom_id: "#swagger",
    //   spec: swagger,
    //   layout: "BaseLayout",
    // });
    // console.log(x);
    // console.log(
    //   SwaggerUI({
    //     // dom_id: "#swagger",
    //     spec: swagger,
    //     layout: "BaseLayout",
    //   }).getComponents()
    // );
    console.log(
      SwaggerUIBundle({
        dom_id: "#swagger",
        spec: swagger,
        layout: "BaseLayout",
      })
    );
  });

  return (
    <div>
      <div id="swagger" />
    </div>
  );
};

export default SwaggerPanel;
