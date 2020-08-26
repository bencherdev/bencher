import React from "react"

import Workflows from "./Workflows"
import Flows from "./Flows"
import Templates from "./Templates"

const Panel = (props: { config: any; panel: string }) => {
  function panelSwitch() {
    switch (props.panel) {
      case "workflows":
        return <Workflows workflows={props?.config?.workflows} />
      case "flows":
        return <Flows flows={props?.config?.flows} />
      case "templates":
        return <Templates templates={props?.config?.templates} />
      case "contracts":
        return <p>TODO</p>
    }
  }

  return <React.Fragment>{props?.panel && panelSwitch()}</React.Fragment>
}

export default Panel
