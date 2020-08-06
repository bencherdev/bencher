// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

import { Container } from "react-bulma-components"

const Modeler = Loadable(() =>
  import("../../../components/studio/flow/modeler")
)

const FlowModelerPage = () => (
  <Container breakpoint="widescreen">
    <Modeler />
  </Container>
)

export default FlowModelerPage
