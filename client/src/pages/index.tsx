import React from "react"
import Loadable from "@loadable/component"

import SEO from "../components/utils/seo"
const ClientSideRedirect = Loadable(() =>
  import("../components/utils/redirect")
)

const RootIndex = () => (
  <React.Fragment>
    <SEO title="TableFlow" />
    <ClientSideRedirect to={"/studio/flow/new"} />
  </React.Fragment>
)

export default RootIndex
