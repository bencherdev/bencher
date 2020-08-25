import React from "react"
import { Card, Content, Box, Button, Icon } from "react-bulma-components"

import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { faChartBar, faCog } from "@fortawesome/free-solid-svg-icons"

import Chart from "../variables/charts/Chart"

const ChartElement = (props: {
  id: string
  config: { name: string }
  table: any
  handleElement: Function
  handleVariable: Function
  getVariable: Function
}) => {
  return (
    <Card.Content>
      <Content>
        <Box>
          <Chart
            id={props.table?.id}
            value={props.table?.value}
            disabled={false}
            handleVariable={props.handleVariable}
          />
          <Button
            color="primary"
            outlined={true}
            size="small"
            fullwidth={true}
            title="Settings"
            onClick={(event: any) => {
              event.preventDefault()
              // props.handleElement()
              console.log("TODO edit settings")
            }}
          >
            <Icon className="primary">
              <FontAwesomeIcon icon={faCog} size="1x" />
            </Icon>
            <span>Settings</span>
          </Button>
        </Box>
      </Content>
    </Card.Content>
  )
}

export default ChartElement
