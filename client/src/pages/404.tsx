// If you don't want to use TypeScript you can delete this file!
import React from "react"
import { Link } from "gatsby"

import SitePage from "../components/site/pages/sitepage"

const NotFoundPage = () => {
  let date = Date()
  return (
    <SitePage link={Link}>
      <h1>Page Not Found</h1>
      <p>Rendered at: {date}</p>
    </SitePage>
  )
}

export default NotFoundPage
