// If you don't want to use TypeScript you can delete this file!
import React, { Component } from "react"
import { PageProps, Link, graphql } from "gatsby"
import Loadable from "@loadable/component"

import Layout from "../components/layout"
import SEO from "../components/seo"

const ClientSide = Loadable(() => import("../components/client"))

type DataProps = {
  site: {
    buildTime: string
  }
}

const ClientSidePage: React.FC<PageProps<DataProps>> = ({ data, path }) => (
  <Layout>
    <SEO title="Client Side" />
    <h1>Client Side</h1>
    <ClientSide />
    <p>
      You're currently on the page "{path}" which was built on{" "}
      {data.site.buildTime}.
    </p>
    <Link to="/">Go back to the homepage</Link>
  </Layout>
)

export default ClientSidePage

export const query = graphql`
  {
    site {
      buildTime(formatString: "YYYY-MM-DD hh:mm:ss a z")
    }
  }
`
