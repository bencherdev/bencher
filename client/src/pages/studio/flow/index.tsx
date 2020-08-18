// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

import { Container } from "react-bulma-components"

const Toolbar = Loadable(() =>
  import("../../../components/studio/flow/toolbar")
)
const Notebook = Loadable(() =>
  import("../../../components/studio/flow/notebook")
)

const FlowModelerPage = () => (
  <React.Fragment>
    <Toolbar />
    <Container fluid={true} breakpoint="widescreen">
      <Notebook />
    </Container>
  </React.Fragment>
)

export default FlowModelerPage
