import "./styles/styles.scss";
import { Component } from "solid-js";

import { Analytics, AnalyticsInstance} from 'analytics'
import googleAnalytics from '@analytics/google-analytics'

import { LinePlot } from "./components/LinePlot";
import { Navbar } from "./components/site/Navbar";



const get_analytics: () => AnalyticsInstance | null = () => {
  let google_analytics = import.meta.env.VITE_GOOGLE_ANALYTICS;
  if (google_analytics === undefined) {
    return;
  } else {
    return Analytics({
      app: "bencher.dev",
      plugins: [
        googleAnalytics({
          trackingId: google_analytics,
        })
      ]
    });
  }
}

const count_page_view = (analytics: AnalyticsInstance | null) => {
  if (analytics === undefined) {
    return;
  } else {
    analytics?.id.page()
  }
}

const analytics = get_analytics();
count_page_view(analytics);

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
