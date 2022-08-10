import { createEffect } from "solid-js";

const PerfPanel = (props) => {
  createEffect(() => {
    const title = "Benchmark Perf";
    if (title) {
      props.handleTitle(title);
    }
  });

  return <div>Perf Page here</div>;
};

export default PerfPanel;
