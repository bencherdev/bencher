import { createSignal, For } from "solid-js";
import { PerKind, perfKindCapitalized } from "../../../config/types";

const perf_kinds = [
  PerKind.LATENCY,
  PerKind.THROUGHPUT,
  PerKind.COMPUTE,
  PerKind.MEMORY,
  PerKind.STORAGE,
];

const PlotHeader = (props) => {
  return (
    <nav class="panel-heading level">
      <div class="level-left">
        <select
          class="card-header-title level-item"
          value={props.query?.kind}
          onInput={(e) => props.handleKind(e.currentTarget?.value)}
        >
          <For each={perf_kinds}>
            {(kind) => (
              <option value={kind}>{perfKindCapitalized(kind)}</option>
            )}
          </For>
        </select>
      </div>
      <div class="level-right">
        <div class="level-item">
          <nav class="level is-mobile">
            <div class="level-item has-text-centered">
              <p class="card-header-title">Start Date</p>
              <input
                type="date"
                onInput={(e) => props.handleStartTime(e.currentTarget?.value)}
              />
            </div>
          </nav>
        </div>
        <div class="level-item">
          <nav class="level is-mobile">
            <div class="level-item has-text-centered">
              <p class="card-header-title">End Date</p>
              <input
                type="date"
                onInput={(e) => props.handleEndTime(e.currentTarget?.value)}
              />
            </div>
          </nav>
        </div>
      </div>
    </nav>
  );
};

export default PlotHeader;
