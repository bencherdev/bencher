import React from "react"
import Loadable from "@loadable/component"

const ClientSide = Loadable(() => import("../components/utils/clientside"))
// const ClientSideRedirect = Loadable(() =>
//   import("../components/utils/redirect")
// )

const RootIndex = () => <ClientSide />
// const RootIndex = () => <ClientSideRedirect to={"/studio/flow/new"} />

export default RootIndex
