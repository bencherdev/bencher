import React from "react"
import { Form, Icon } from "react-bulma-components"

const Select = (props: any) => {
  return (
    <Form.Field>
      <Form.Control fullwidth={true}>
        <Form.Select
          name={props.name}
          disabled={props.disabled}
          value={props.selected}
          onChange={(event: any) => props.handleType(event, props.column)}
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
