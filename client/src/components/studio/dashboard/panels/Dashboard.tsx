import React from "react"

import { Columns, Box, Content, Heading } from "react-bulma-components"

const Dashboard = (props: { path: string }) => {
  return (
    <Columns>
      <Columns.Column size={12}>
        <Box>
          <Content className="has-text-centered">
            <Heading size={3}>Workflows</Heading>
          </Content>
        </Box>
      </Columns.Column>
      <Columns.Column>
        <Box>
          <Content className="has-text-centered">
            <Heading size={4}>Flows</Heading>
          </Content>
        </Box>
      </Columns.Column>
      <Columns.Column>
        <Box>
          <Content className="has-text-centered">
            <Heading size={4}>Tempaltes</Heading>
          </Content>
        </Box>
      </Columns.Column>
      <Columns.Column>
        <Box>
          <Content className="has-text-centered">
            <Heading size={4}>Contracts</Heading>
          </Content>
        </Box>
      </Columns.Column>
    </Columns>
  )
}

export default Dashboard
