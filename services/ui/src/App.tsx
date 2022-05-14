import "./styles/styles.scss";
import { Component, createSignal } from "solid-js";

import { Multiplier } from "./Counter";
import { LinePlot } from "./Line";

const App: Component = () => {
  return (
    <section class="section">
      <div class="container">
        <Multiplier by={3} />
        <Multiplier by={4} />
        <Multiplier by={11} />
        <LinePlot />
      </div>
    </section>
  );
};

export default App;
