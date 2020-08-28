import dashboardTabs from "./dashboardTabs"
import workflowsTabs from "./workflowsTabs"
import flowsTabs from "./flowsTabs"
import templatesTabs from "./templatesTabs"

const studioSection = (path: string) => {
  return {
    title: "Studio",
    tabs: [
      dashboardTabs.dashboard(path),
      workflowsTabs.workflows(path),
      workflowsTabs.new(path),
      flowsTabs.flows(path),
      flowsTabs.new(path),
      templatesTabs.templates(path),
      templatesTabs.new(path),
    ],
  }
}

export default studioSection
