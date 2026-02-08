import {
  BareItem,
  ByteSequence,
  Dictionary,
  InnerList,
  Item,
  List,
  Parameters,
} from './types';

import { Token } from './token';

import { isAscii, isInnerList, isValidKeyStr } from './util';

export class SerializeError extends Error {}

export function serializeList(input: List): string {

  return input.map(value => {

    if (isInnerList(value)) {
      return serializeInnerList(value);
    } else {
      return serializeItem(value);
    }

  }).join(', ');

}

export function serializeDictionary(input: Dictionary): string {

  return Array.from(
    input.entries()
  ).map(([key, value]) => {

    let out = serializeKey(key);
    if (value[0]===true) {
      out += serializeParameters(value[1]);
    } else {
      out += '=';
      if (isInnerList(value)) {
        out += serializeInnerList(value);
      } else {
        out += serializeItem(value);
      }
    }
    return out;

  }).join(', ');

}

export function serializeItem(input: Item): string {

  return serializeBareItem(input[0]) + serializeParameters(input[1]);

}

function serializeInnerList(input: InnerList): string {

  return `(${input[0].map(value => serializeItem(value)).join(' ')})${serializeParameters(input[1])}`;

}


function serializeBareItem(input: BareItem): string {
  if (typeof input === 'number') {
    if (Number.isInteger(input)) {
      return serializeInteger(input);
    }
    return serializeDecimal(input);
  }
  if (typeof input === 'string') {
    return serializeString(input);
  }
  if (input instanceof Token) {
    return serializeToken(input);
  }
  if (input instanceof ByteSequence) {
    return serializeByteSequence(input);
  }
  if (typeof input === 'boolean') {
    return serializeBoolean(input);
  }
  throw new SerializeError(`Cannot serialize values of type ${typeof input}`);
}

function serializeInteger(input: number): string {

  if (input < -999_999_999_999_999 || input > 999_999_999_999_999) {
    throw new SerializeError('Structured headers can only encode integers in the range range of -999,999,999,999,999 to 999,999,999,999,999 inclusive');
  }
  return input.toString();
}

function serializeDecimal(input: number): string {
  const out = input.toFixed(3).replace(/0+$/,'');
  const signifantDigits = out.split('.')[0].replace('-','').length;

  if (signifantDigits > 12) {
    throw new SerializeError('Fractional numbers are not allowed to have more than 12 significant digits before the decimal point');
  }
  return out;
}

function serializeString(input: string): string {
  if (!isAscii(input)) {
    throw new SerializeError('Only ASCII strings may be serialized');
  }
  return `"${input.replace(/("|\\)/g, (v) => '\\' + v)}"`;
}

function serializeBoolean(input: boolean): string {
  return input ? '?1' : '?0';
}

function serializeByteSequence(input: ByteSequence): string {
  return `:${input.toBase64()}:`;
}

function serializeToken(input: Token): string {
  return input.toString();
}

function serializeParameters(input: Parameters): string {

  return Array.from(input).map(([key, value]) => {

    let out = ';' + serializeKey(key);
    if (value!==true) {
      out+='=' + serializeBareItem(value);
    }
    return out;

  }).join('');

}

function serializeKey(input: string): string {

  if (!isValidKeyStr(input)) {
    throw new SerializeError('Keys in dictionaries must only contain lowercase letter, numbers, _-*. and must start with a letter or *');
  }
  return input;

}
