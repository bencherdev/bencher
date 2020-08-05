// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

const Modeler = Loadable(() =>
  import("../../../components/studio/modeler/modeler")
)

const FlowModelerPage = () => <Modeler />

export default FlowModelerPage
