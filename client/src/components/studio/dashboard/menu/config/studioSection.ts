import flowsTabs from "./flowsTabs"
import templatesTabs from "./templatesTabs"

const studioSection = (path: string) => {
  return {
    title: "Studio",
    tabs: [
      flowsTabs.dashboard(path, "Flows"),
      flowsTabs.new(path, "Flows"),
      templatesTabs.dashboard(path, "Templates"),
      templatesTabs.new(path, "Templates"),
    ],
  }
}

export default studioSection
