import React from "react"
import { Link } from "gatsby"

import { Breadcrumb } from "react-bulma-components"

const FlowBreadcrumb = (props: any) => (
  <Breadcrumb
    renderAs={Link}
    hrefAttr="to"
    items={[
      {
        name: "Main Subflow",
        url: "#a/1",
      },
      {
        name: "Nested Subflow",
        url: "#a/2",
      },
      {
        name: "Lower Subflow",
        url: "#a/3",
        active: true,
      },
    ]}
  />
)

export default FlowBreadcrumb
