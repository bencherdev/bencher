import { createSignal, For } from "solid-js";
import { PerKind, perfKindCapitalized } from "../../config/types";

const perf_kinds = [
  PerKind.LATENCY,
  PerKind.THROUGHPUT,
  PerKind.COMPUTE,
  PerKind.MEMORY,
  PerKind.STORAGE,
];

const PerfPlot = (props) => {
  return (
    <div class="columns">
      <div class="column">
        <div class="card">
          <header class="card-header">
            <p class="card-header-title">Component</p>
            <button class="card-header-icon" aria-label="more options">
              <span class="icon">
                <i class="fas fa-angle-down" aria-hidden="true" />
              </span>
            </button>
            <select
              value={props.query?.kind}
              onInput={(e) => props.handleKind(e.currentTarget?.value)}
            >
              <For each={perf_kinds}>
                {(kind) => (
                  <option value={kind}>{perfKindCapitalized(kind)}</option>
                )}
              </For>
            </select>
          </header>
          <div class="card-content">
            <div class="content">
              Lorem ipsum dolor sit amet, consectetur adipiscing elit. Phasellus
              nec iaculis mauris.
              <a href="#">@bulmaio</a>. <a href="#">#css</a>{" "}
              <a href="#">#responsive</a>
              <br />
              <time datetime="2016-1-1">11:09 PM - 1 Jan 2016</time>
            </div>
          </div>
          <footer class="card-footer">
            <a href="#" class="card-footer-item">
              Save
            </a>
            <a href="#" class="card-footer-item">
              Edit
            </a>
            <a href="#" class="card-footer-item">
              Delete
            </a>
          </footer>
        </div>
      </div>
    </div>
  );
};

export default PerfPlot;
