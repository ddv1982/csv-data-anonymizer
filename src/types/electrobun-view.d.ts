declare module 'electrobun/view' {
  type RequestMethod<Definition> = Definition extends { params: infer Params; response: infer Response }
    ? undefined extends Params
      ? (params?: Params) => Promise<Response>
      : (params: Params) => Promise<Response>
    : never

  type WebviewRpc<Schema> = Schema extends { bun: { requests: infer Requests } }
    ? {
        request: {
          [Method in keyof Requests]: RequestMethod<Requests[Method]>
        }
      }
    : never

  export class Electroview<T = unknown> {
    constructor(config: { rpc: T })
    static defineRPC<Schema>(config: unknown): WebviewRpc<Schema>
  }
}
