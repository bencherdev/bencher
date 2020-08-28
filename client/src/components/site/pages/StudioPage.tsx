// If you don't want to use TypeScript you can delete this file!
import React from "react"
import Loadable from "@loadable/component"

import SEO from "../../utils/seo"
const Studio = Loadable(() => import("../../studio/dashboard/Studio"))

const StudioPage = (props: { title: string }) => (
  <React.Fragment>
    <SEO title={props.title} />
    <Studio />
  </React.Fragment>
)

export default StudioPage
