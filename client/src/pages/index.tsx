import React from "react"
import Loadable from "@loadable/component"

import "react-bulma-components/dist/react-bulma-components.min.css"
import "../styles/_variables.sass"

const ClientSideRedirect = Loadable(() =>
  import("../components/utils/redirect")
)

const RootIndex = () => <ClientSideRedirect to={"/studio/flow"} />

export default RootIndex
