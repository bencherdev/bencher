import React from "react"

import BarChart from "./BarChart"

const Chart = (props: {
  id: string
  value: any
  disabled: boolean
  handleVariable: Function
}) => {
  return <BarChart data={props?.value?.rows} />
}

export default Chart
