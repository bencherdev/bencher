import workflowsTabs from "./workflowsTabs"
import flowsTabs from "./flowsTabs"
import templatesTabs from "./templatesTabs"

const studioSection = (path: string) => {
  return {
    title: "Studio",
    tabs: [
      workflowsTabs.dashboard(path, "Workflows"),
      flowsTabs.dashboard(path, "Flows"),
      templatesTabs.dashboard(path, "Templates"),
    ],
  }
}

export default studioSection
