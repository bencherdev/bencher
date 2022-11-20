import { createEffect } from "solid-js";

const DocPanel = (props) => {
  return (
    <div class="content">
      <h1 class="title">{props.page?.heading}</h1>
      <hr />
      {props.page?.content}
      <br />
    </div>
  );
};

export default DocPanel;
