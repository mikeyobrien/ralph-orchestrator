import type * as _nodeTty from "node:tty";
import type * as _nodeStream from "node:stream";
declare function main(argvInput?: Array<string>, _optionalArg?: {
  readonly stdin?: _nodeStream.Readable | _nodeTty.ReadStream;
  readonly stderr: _nodeStream.Writable;
  readonly stdout: _nodeStream.Writable;
}): Promise<number>;
export default main;