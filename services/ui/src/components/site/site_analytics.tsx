import { Analytics } from "analytics";
import googleAnalytics from "@analytics/google-analytics";

export const site_analytics = () => {
  let plugins = [];

  const google_analytics_id = import.meta.env.VITE_GOOGLE_ANALYTICS_ID;
  if (google_analytics_id) {
    plugins.push(
      googleAnalytics({
        measurementIds: [google_analytics_id],
      })
    );
  }

  return Analytics({
    app: "bencher.dev",
    plugins: plugins,
  });
};
