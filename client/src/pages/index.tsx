import React from "react"
import Loadable from "@loadable/component"

const ClientSideRedirect = Loadable(() =>
  import("../components/utils/redirect")
)

const RootIndex = () => <ClientSideRedirect to={"/studio/flow/new"} />

export default RootIndex
