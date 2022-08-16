import { createEffect } from "solid-js";

const Plot = (props) => {
  createEffect(() => {
    console.log(props.perf_data());
  });
  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-one-fifth">
            <div class="box">
              <div class="content">Benchmark</div>
            </div>
          </div>
          <div class="column">
            <div class="box">
              <div class="content">Plot</div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default Plot;
