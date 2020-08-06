import React, { useState } from "react"
import Loadable from "@loadable/component"

const ClientSideRedirect = Loadable(() => import("../../utils/redirect"))

import {
  Section,
  Container,
  Columns,
  Content,
  Heading,
  Button,
} from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import {
  faStream,
  faStopwatch,
  faPlusCircle,
  faRecycle,
  faTable,
  faEquals,
} from "@fortawesome/free-solid-svg-icons"

import { faObjectUngroup } from "@fortawesome/free-regular-svg-icons"

const AboutPage = () => {
  const [redirect, setRedirect] = useState(false)

  return (
    <Section size="medium">
      {redirect && <ClientSideRedirect to="/studio/flow/new" />}
      <Container>
        <Content className="has-text-centered">
          <Heading size={2}>Build modular spreadsheets</Heading>
        </Content>
      </Container>

      <Section>
        <Container>
          <Columns centered={true}>
            <Columns.Column size="half">
              <Button
                fullwidth={true}
                color="primary"
                onClick={() => {
                  setRedirect(true)
                }}
              >
                Start Now
              </Button>
            </Columns.Column>
          </Columns>
        </Container>
      </Section>

      <hr />

      <Section>
        <Container>
          <Columns centered={true}>
            <Columns.Column>
              <Heading size={2}>How It Works</Heading>
            </Columns.Column>
          </Columns>
          <br />
          <br />
          <Columns centered={true}>
            <Columns.Column>
              <Columns className="is-vcentered">
                <Columns.Column>
                  <Content className="has-text-centered">
                    <span className="icon has-text-primary">
                      <FontAwesomeIcon icon={faStream} size="5x" />
                    </span>
                  </Content>
                </Columns.Column>
                <Columns.Column size="three-quarters">
                  <Content>
                    <Heading size={5}>Create a Flow</Heading>
                    <p>
                      A Flow is like a spreadsheet that is broken out into its
                      constitute parts: tables of values, functions, and results
                      that are chanined together one after the other.
                    </p>
                  </Content>
                </Columns.Column>
              </Columns>
            </Columns.Column>
            <br />
            <Columns.Column>
              <Columns className="is-vcentered">
                <Columns.Column>
                  <Content className="has-text-centered">
                    <span className="icon has-text-primary">
                      <FontAwesomeIcon icon={faStopwatch} size="5x" />
                    </span>
                  </Content>
                </Columns.Column>
                <Columns.Column size="three-quarters">
                  <Content>
                    <Heading size={5}>Instantly See Results</Heading>
                    <p>
                      Just like a spreadsheet, your calculations are run as soon
                      as you press enter, so you can quickly iterate on your
                      financial model.
                    </p>
                  </Content>
                </Columns.Column>
              </Columns>
            </Columns.Column>
          </Columns>
          <br />
          <Columns centered={true}>
            <Columns.Column>
              <Columns className="is-vcentered">
                <Columns.Column>
                  <Content className="has-text-centered">
                    <span className="icon has-text-primary">
                      <FontAwesomeIcon icon={faPlusCircle} size="5x" />
                    </span>
                  </Content>
                </Columns.Column>
                <Columns.Column size="three-quarters">
                  <Content>
                    <Heading size={5}>Add Subflows</Heading>
                    <p>
                      Subflows work just like a Function, they let you see the
                      input and the output at a high level. You can always
                      expand a Subflow to exactly what it does.
                    </p>
                  </Content>
                </Columns.Column>
              </Columns>
            </Columns.Column>
            <br />
            <Columns.Column>
              <Columns className="is-vcentered">
                <Columns.Column>
                  <Content className="has-text-centered">
                    <span className="icon has-text-primary">
                      <FontAwesomeIcon icon={faRecycle} size="5x" />
                    </span>
                  </Content>
                </Columns.Column>
                <Columns.Column size="three-quarters">
                  <Content>
                    <Heading size={5}>Reuse Your Flows</Heading>
                    <p>
                      Other Flows can be called just like a Function from within
                      a Flow. Subflows can also be exported as a standalone Flow
                      if you want to use them in another model.
                    </p>
                  </Content>
                </Columns.Column>
              </Columns>
            </Columns.Column>
          </Columns>
        </Container>
      </Section>

      <hr />

      <Section>
        <Container>
          <Columns centered={true}>
            <Columns.Column>
              <Heading size={2}>Better than a Spreadsheet</Heading>
            </Columns.Column>
          </Columns>
          <br />
          <br />
          <Columns centered={true}>
            <Columns.Column>
              <Content className="has-text-centered">
                <span className="icon has-text-primary">
                  <FontAwesomeIcon icon={faTable} size="5x" />
                </span>
                <Heading size={5}>Tables &amp; Cells</Heading>
              </Content>
              <Content>
                <p>
                  Tables are the foundation of data in TableFlow, instead of
                  cells like in traditional spreadsheets. Each Table is like a
                  mini-database that can hold rows of numbers, text, and even
                  other Tables.
                </p>
                <br />
              </Content>
            </Columns.Column>
            <Columns.Column>
              <Content className="has-text-centered">
                <span className="icon has-text-primary">
                  <FontAwesomeIcon icon={faEquals} size="5x" />
                </span>
                <Heading size={5}>Flows &amp; Functions</Heading>
              </Content>
              <Content>
                <p>
                  All of your favorite functions from Excel are here. If not,
                  you don't need to wait for some programmer to build you a
                  macro. All Flows are functions! So you can call a Flow just
                  like you would a function.
                </p>
                <br />
              </Content>
            </Columns.Column>
            <Columns.Column>
              <Content className="has-text-centered">
                <span className="icon has-text-primary">
                  <FontAwesomeIcon icon={faObjectUngroup} size="5x" />
                </span>
                <Heading size={5}>Subflows &amp; Worksheets</Heading>
              </Content>
              <Content>
                <p>
                  Subflows are a much imporved version of a worksheet. They let
                  you easily separate out your work. A Subflow can be exported
                  to a Function, just like a regular Flow can.
                </p>
                <br />
              </Content>
            </Columns.Column>
          </Columns>
        </Container>
      </Section>
    </Section>
  )
}

export default AboutPage
