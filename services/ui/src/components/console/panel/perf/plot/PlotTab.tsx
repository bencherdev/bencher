import { For } from "solid-js";
import { PerKind, perfKindCapitalized } from "../../../config/types";

const PlotTab = (props) => {
  return (
    <>
      <For each={["A", "B", "C"]}>
        {(item) => (
          <a class="panel-block">
            <input type="checkbox" />
            {item}
          </a>
        )}
      </For>
    </>
  );
};

export default PlotTab;
