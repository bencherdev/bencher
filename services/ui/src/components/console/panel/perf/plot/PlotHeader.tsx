import { For } from "solid-js";
import { toCapitalized } from "../../../config/util";

const metric_kinds = ["latency"];

const PlotHeader = (props) => {
  return (
    <nav class="panel-heading level">
      <div class="level-left">
        <select
          class="card-header-title level-item"
          value={props.metric_kind()}
          onInput={(e) => props.handleMetricKind(e.currentTarget?.value)}
        >
          <For each={metric_kinds}>
            {(metric_kind) => (
              <option value={metric_kind}>{toCapitalized(metric_kind)}</option>
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
                value={props.start_date()}
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
                value={props.end_date()}
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
