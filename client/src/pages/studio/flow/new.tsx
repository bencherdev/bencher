// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

const NewFlow = Loadable(() =>
  import("../../../components/studio/modeler/newflow")
)

const NewFlowPage = () => <NewFlow />

export default NewFlowPage
