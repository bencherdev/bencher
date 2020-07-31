/**
 * Layout component that queries for data
 * with Gatsby's useStaticQuery component
 *
 * See: https://www.gatsbyjs.org/docs/use-static-query/
 */

import React from "react"
import PropTypes from "prop-types"
import { useStaticQuery, graphql } from "gatsby"

import SiteNavbar from "../navbar/sitenavbar"
import SiteFooter from "../footer/sitefooter"

const SitePage = ({ link, children }: any) => {
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
    <>
      <SiteNavbar link={link} user={{ isAuth: false }} />
      <div>
        <main>{children}</main>
        <SiteFooter />
      </div>
    </>
  )
}

SitePage.propTypes = {
  children: PropTypes.node.isRequired,
}

export default SitePage
