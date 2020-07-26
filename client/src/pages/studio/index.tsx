// If you don't want to use TypeScript you can delete this file!
import React, { Component } from "react"
import { PageProps, Link, graphql } from "gatsby"
import Loadable from "@loadable/component"

import Layout from "../../components/layout"
import SEO from "../../components/seo"

const Flows = Loadable(() => import("../../components/studio/flows"))

const flows: string[] = ["A", "B", "C"]

const StudioPage = () => (
  <Layout>
    <SEO title="TableFlow Studio" />
    <h1>TableFlow Studio</h1>
    {flows.map((flow, index) => {
      return <Flows key={index} flow={flow} />
    })}
    <Link to="/">Go back to the homepage</Link>
  </Layout>
)

export default StudioPage
