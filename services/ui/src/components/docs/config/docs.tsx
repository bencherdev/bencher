import { Docs } from "./types";
import QuickStart from "../pages/QuickStart.mdx";

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
};

export default docsConfig;
