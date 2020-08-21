// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

import { Container } from "react-bulma-components"

const Toolbar = Loadable(() =>
  import("../../../components/studio/notebook/Toolbar")
)
const Breadcrumb = Loadable(() =>
  import("../../../components/studio/notebook/Breadcrumb")
)
const Notebook = Loadable(() =>
  import("../../../components/studio/notebook/Notebook")
)

const FlowModelerPage = () => (
  <React.Fragment>
    <Toolbar />
    <Breadcrumb />
    <Container fluid={true} breakpoint="widescreen">
      <Notebook />
    </Container>
  </React.Fragment>
)

export default FlowModelerPage
