import "./styles/styles.scss";
import { Component } from "solid-js";

import { LinePlot } from "./components/LinePlot";
import { Navbar } from "./components/site/Navbar";

const App: Component = () => {
  return (
    <>
      <Navbar />
      <section class="section">
        <div class="container">
          <div class="columns">
            <div class="column">
              <LinePlot />
            </div>
          </div>
        </div>
      </section>
    </>
  );
};

export default App;
