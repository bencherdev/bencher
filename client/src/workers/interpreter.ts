import InterpreterWorker from "./interpreter.worker"

const interpreter = typeof window === "object" && new InterpreterWorker()

export default interpreter
