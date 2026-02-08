declare namespace bPlistParser {
  type CallbackFunction<T = any> = (error: Error|null, result: [T]) => void
  export function parseFile<T = any>(fileNameOrBuffer: string|Buffer, callback?: CallbackFunction<T>): Promise<[T]>
}

export = bPlistParser
