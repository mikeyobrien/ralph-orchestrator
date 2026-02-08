import { isValidTokenStr } from './util';

export class Token {

  private value: string;
  constructor(value: string) {

    if (!isValidTokenStr(value)) {
      throw new TypeError('Invalid character in Token string. Tokens must start with *, A-Z and the rest of the string may only contain a-z, A-Z, 0-9, :/!#$%&\'*+-.^_`|~');
    }
    this.value = value;

  }

  toString(): string {

    return this.value;

  }

}
