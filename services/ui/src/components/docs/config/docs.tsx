import Page from "./page";
import QuickStart from "../pages/QuickStart.mdx";
import Changelog from "../pages/Changelog.mdx";
import PriorArt from "../pages/PriorArt.mdx";
import GitHubActions from "../pages/GitHubActions.mdx";

const docsConfig = {
  [Page.QUICK_START]: {
    kind: Page.QUICK_START,
    title: "Quick Start",
    page: {
      heading: "Quick Start",
      content: <QuickStart />,
    },
  },
  [Page.GITHUB_ACTIONS]: {
    kind: Page.GITHUB_ACTIONS,
    title: "GitHub Actions",
    page: {
      heading: "How to use Bencher in GitHub Actions",
      content: <GitHubActions />,
    },
  },
  [Page.API_V0]: {
    kind: Page.API_V0,
    title: "Bencher REST API",
  },
  [Page.PRIOR_ART]: {
    kind: Page.PRIOR_ART,
    title: "Prior Art",
    page: {
      heading: "Prior Art",
      content: <PriorArt />,
    },
  },
  [Page.CHANGELOG]: {
    kind: Page.CHANGELOG,
    title: "Changelog",
    page: {
      heading: "Changelog",
      content: <Changelog />,
    },
  },
};

export default docsConfig;
