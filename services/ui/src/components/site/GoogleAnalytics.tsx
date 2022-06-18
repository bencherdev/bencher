import { createSignal, onCleanup } from "solid-js";

import { Analytics, AnalyticsInstance} from 'analytics'
import googleAnalytics from '@analytics/google-analytics'

export const GoogleAnalytics = () => {
  let google_analytics = import.meta.env.VITE_GOOGLE_ANALYTICS;
  if (google_analytics !== undefined) {
    let analytics = Analytics({
      app: "bencher.dev",
      plugins: [
        googleAnalytics({
          trackingId: google_analytics,
        })
      ]
    });
    analytics?.page();
  }
  
	return <></>;
};
