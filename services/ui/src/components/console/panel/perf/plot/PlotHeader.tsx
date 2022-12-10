import axios from "axios";
import { createMemo, createResource, For } from "solid-js";
import { get_options } from "../../../../site/util";
import { toCapitalized } from "../../../config/util";

const PlotHeader = (props) => {
  const metric_kinds_fetcher = createMemo(() => {
    return {
      refresh: props.refresh(),
      token: props.user()?.token,
    };
  });

  const getMetricKinds = async (fetcher) => {
    console.log(props.metric_kind());
    try {
      const url = props.config?.metric_kinds_url(props.path_params());
      const resp = await axios(get_options(url, fetcher.token));
      const data = resp.data;
      if (data.length > 0) {
        props.handleMetricKind(data[0]?.slug);
      }
      return data;
    } catch (error) {
      console.error(error);
      return [];
    }
  };

  const [metric_kinds] = createResource(metric_kinds_fetcher, getMetricKinds);

  return (
    <nav class="panel-heading level">
      <div class="level-left">
        <select
          class="card-header-title level-item"
          value={props.metric_kind()}
          onInput={(e) => props.handleMetricKind(e.currentTarget?.value)}
        >
          <For each={metric_kinds()}>
            {(metric_kind: { name: string; slug: string }) => (
              <option value={metric_kind?.slug}>{metric_kind?.name}</option>
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
