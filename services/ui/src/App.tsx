import "./styles/styles.scss";
import { Component } from "solid-js";

import { Analytics, AnalyticsInstance} from 'analytics'
import googleAnalytics from '@analytics/google-analytics'

import { LinePlot } from "./components/LinePlot";
import { Navbar } from "./components/site/Navbar";
import { GoogleAnalytics } from "./components/site/GoogleAnalytics";

const App: Component = () => {
  return (
    <>
      <GoogleAnalytics />
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
