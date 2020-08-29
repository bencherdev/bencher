import React from "react"
import { Form } from "react-bulma-components"

const Select = (props: {
  name: string
  disabled: boolean
  column: number
  selected: string
  config: any
  handleSelect: Function
}) => {
  return (
    <Form.Field>
      <Form.Control fullwidth={true}>
        <Form.Select
          name={props.name}
          disabled={props.disabled}
          value={props.selected}
          onChange={(event: any) => props.handleSelect(event, props.column)}
          className="is-fullwidth"
        >
          {props.config?.options?.map((option: any) => {
            return (
              <option key={option.value} value={option.value}>
                {option.option}
              </option>
            )
          })}
        </Form.Select>
      </Form.Control>
    </Form.Field>
  )
}

export default Select
