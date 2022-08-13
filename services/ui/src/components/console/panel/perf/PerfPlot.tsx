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
            <nav class="level">
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
                      <input type="date" onInput={(e) => console.log(e)} />
                    </div>
                  </nav>
                </div>
                <div class="level-item">
                  <nav class="level is-mobile">
                    <div class="level-item has-text-centered">
                      <p class="card-header-title">End Date</p>
                      <input type="date" onInput={(e) => console.log(e)} />
                    </div>
                  </nav>
                </div>
              </div>
            </nav>
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
