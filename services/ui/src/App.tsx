import "./styles/styles.scss";
import { Component } from "solid-js";

import { LinePlot } from "./components/LinePlot";

const App: Component = () => {
  return (
    <section class="section">
      <div class="container">
        <div class="columns">
          <div class="column">
            <LinePlot />
          </div>
        </div>
      </div>
    </section>
  );
};

export default App;
