/**
 * Layout component that queries for data
 * with Gatsby's useStaticQuery component
 *
 * See: https://www.gatsbyjs.org/docs/use-static-query/
 */

import React from "react"
import PropTypes from "prop-types"
import { useStaticQuery, graphql, Link } from "gatsby"

import "react-bulma-components/dist/react-bulma-components.min.css"
import SiteNavbar from "../site/navbar/sitenavbar"
import SiteFooter from "../site/footer/sitefooter"
import "../../styles/_variables.scss"

const Layout = ({ children }: any) => {
  const data = useStaticQuery(graphql`
    query SiteTitleQuery {
      site {
        siteMetadata {
          title
        }
      }
    }
  `)

  return (
    <React.Fragment>
      <SiteNavbar link={Link} user={{ isAuth: false }} />
      <main>{children}</main>
      <SiteFooter />
    </React.Fragment>
  )
}

Layout.propTypes = {
  children: PropTypes.node.isRequired,
}

export default Layout
