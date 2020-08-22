// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { navigate } from "gatsby"

import SEO from "../../components/utils/seo"

const StudioPage = () => (
  <React.Fragment>
    <SEO title="Studio" />
    {navigate("/studio/flows")}
  </React.Fragment>
)

export default StudioPage
