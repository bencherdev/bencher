import React from "react"
import { Link } from "@reach/router"
import styled from "styled-components"

import {
  Footer,
  Container,
  Columns,
  Heading,
  Content,
} from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faExternalLinkAlt } from "@fortawesome/free-solid-svg-icons"
import {
  faTwitterSquare,
  faGithubSquare,
} from "@fortawesome/free-brands-svg-icons"

import ExternalLink from "../../utils/externallink"

const BrandName = styled.h1`
  color: black;
  font-size: 300%;
`

const SiteFooter = () => {
  return (
    <Footer>
      <Container>
        <Columns>
          <Columns.Column size="half">
            <Columns breakpoint="mobile">
              <Columns.Column size="one-third">
                TableFlow
                <ul>
                  <li>
                    <Link to="/about">About</Link>
                  </li>
                  <li>
                    <Link to="/studio">Studio</Link>
                  </li>
                  <li>
                    <Link to="/docs">Docs</Link>
                  </li>
                  <li>
                    <ExternalLink to="https://github.com/tableflow/tableflow">
                      GitHub
                      {" ".repeat(1)}
                      <span className="icon has-text-primary">
                        <FontAwesomeIcon icon={faExternalLinkAlt} />
                      </span>
                    </ExternalLink>
                  </li>
                </ul>
              </Columns.Column>
              <Columns.Column size="one-third">
                TableFlow Docs
                <ul>
                  <li>
                    <Link to="/docs/quick-start">Quick Start</Link>
                  </li>
                  <li>
                    <Link to="/docs/tutorial">Tutorial</Link>
                  </li>
                  <li>
                    <Link to="/docs/faq">FAQ</Link>
                  </li>
                  <li>
                    <Link to="/docs/changelog">Change Log</Link>
                  </li>
                </ul>
              </Columns.Column>
              {/* <Columns.Column size="one-third">
                TableFlow Enterprise
                <ul>
                  <li>
                    <Link to="/pricing">Pricing</Link>
                  </li>
                </ul>
              </Columns.Column> */}
            </Columns>
          </Columns.Column>

          <Columns.Column size="half">
            <Columns>
              <Columns.Column size="half">
                <Content className="has-text-centered">
                  <Heading size={2}> TableFlow</Heading>
                  <a href="https://github.com/tableflow">
                    <span className="icon has-text-primary">
                      <FontAwesomeIcon icon={faGithubSquare} size="2x" />
                    </span>
                  </a>
                  <>{"⠀".repeat(1)}</>
                  <a href="https://twitter.com/tableflow">
                    <span className="icon has-text-primary">
                      <FontAwesomeIcon icon={faTwitterSquare} size="2x" />
                    </span>
                  </a>
                </Content>
              </Columns.Column>
              <Columns.Column>
                <Content>
                  The information provided on TableFlow is not financial advice,
                  does not constitute a financial service, and no confidential
                  or advisor-client relationship is formed by using this site.
                  Your use of this site constitutes acceptance of the{" "}
                  <Link to="/terms">Terms of Use</Link>,{" "}
                  <Link to="/privacy">Privacy Policy</Link>, and{" "}
                  <Link to="/licensing">Licensing Policy</Link>.
                  <br />
                  <br />© Pompeii LLC, All Rights Reserved{" "}
                  {new Date().getFullYear()}
                </Content>
              </Columns.Column>
            </Columns>
          </Columns.Column>
        </Columns>
      </Container>
    </Footer>
  )
}

export default SiteFooter
