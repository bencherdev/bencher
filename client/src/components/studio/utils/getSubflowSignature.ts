function getSubflowSignature(subflow: any): any {
  if (!subflow) {
    return
  }

  let inputElementId = subflow.input
  let outputElementId = subflow.output
  if (!inputElementId || !outputElementId) {
    return
  }

  let inputElement = subflow.elements?.[inputElementId]
  let outputElement = subflow.elements?.[outputElementId]
  if (!inputElement || !outputElement) {
    return
  }

  let subflowInputs = inputElement.value?.inputs
  let subflowOutputs = outputElement.value?.outputs
  if (!subflowInputs || !subflowOutputs) {
    return
  }

  return {
    id: subflow.id,
    inputs: subflowInputs,
    outputs: subflowOutputs,
  }
}

export default getSubflowSignature
