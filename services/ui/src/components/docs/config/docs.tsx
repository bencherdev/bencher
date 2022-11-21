import { Docs } from "./types";
import QuickStart from "../pages/QuickStart.mdx";
import Changelog from "../pages/Changelog.mdx";

const docsConfig = {
  [Docs.QUICK_START]: {
    docs: Docs.QUICK_START,
    title: "Quick Start",
    page: {
      heading: "Quick Start",
      content: <QuickStart />,
    },
  },
  [Docs.API_V0]: {
    docs: Docs.API_V0,
    title: "Bencher REST API",
  },
  [Docs.CHANGELOG]: {
    docs: Docs.CHANGELOG,
    title: "Changelog",
    page: {
      heading: "Changelog",
      content: <Changelog />,
    },
  },
};

export default docsConfig;
