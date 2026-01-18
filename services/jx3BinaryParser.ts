export enum BinaryLuaType {
  Number = 0,
  Boolean = 1,
  String = 2,
  Nil = 3,
  Table = 4,
}

export interface Jx3BinaryHeader {
  sig: bigint;
  version: number;
  compress: boolean;
  hash: boolean;
  crc: number;
}

export interface Jx3BinaryReadOptions {
  encoding?: 'gbk' | 'utf8' | 'latin1';
  strict?: boolean;
}

const BINARY_SIG_FLAG = BigInt('0x206174614461754c');

export function isJx3Binary(buffer: ArrayBuffer): boolean {
  if (buffer.byteLength < 8) return false;
  const dataView = new DataView(buffer);
  const sig = dataView.getBigUint64(0, true);
  return sig === BINARY_SIG_FLAG;
}

export function readJx3BinaryHeader(buffer: ArrayBuffer): Jx3BinaryHeader {
  const dataView = new DataView(buffer);
  const sig = dataView.getBigUint64(0, true);
  const version = dataView.getInt32(8, true);
  const compress = dataView.getUint8(12) === 1;
  const hash = dataView.getUint8(13) === 1;
  const crc = dataView.getUint32(14, true);
  return { sig, version, compress, hash, crc };
}

export function decodeGBK(buffer: Uint8Array): string {
  try {
    const decoder = new TextDecoder('gbk', { fatal: false });
    return decoder.decode(buffer);
  } catch {
    return decodeGBKFallback(buffer);
  }
}

function decodeGBKFallback(buffer: Uint8Array): string {
  let result = '';
  for (let i = 0; i < buffer.length; i++) {
    const byte = buffer[i];
    if (byte < 0x80) {
      result += String.fromCharCode(byte);
    } else if (i + 1 < buffer.length) {
      const nextByte = buffer[i + 1];
      const code = (byte << 8) | nextByte;
      result += String.fromCharCode(code);
      i++;
    }
  }
  return result;
}

export function readJx3Binary(buffer: ArrayBuffer, options?: Jx3BinaryReadOptions): any {
  const { encoding = 'gbk', strict = false } = options || {};
  
  if (buffer.byteLength < 18) {
    throw new Error('Buffer too small to be a valid JX3 binary file');
  }

  const header = readJx3BinaryHeader(buffer);
  
  if (header.sig !== BINARY_SIG_FLAG && strict) {
    throw new Error(`Invalid signature: 0x${header.sig.toString(16)}`);
  }

  const payloadBuffer = buffer.slice(18);
  const result = readBinaryPayload(payloadBuffer, encoding);
  
  return result;
}

export function readBinaryPayload(buffer: ArrayBuffer, encoding: string): any {
  const dataView = new DataView(buffer);
  let offset = 0;
  
  return readValue(dataView, buffer, offset, encoding).value;
}

function readValue(dataView: DataView, buffer: ArrayBuffer, offset: number, encoding: string): { value: any; offset: number } {
  if (offset >= buffer.byteLength) {
    throw new Error('Unexpected end of buffer');
  }

  const type = dataView.getUint8(offset++) as BinaryLuaType;

  switch (type) {
    case BinaryLuaType.Number:
      const numValue = dataView.getFloat64(offset, true);
      return { value: numValue, offset: offset + 8 };

    case BinaryLuaType.Boolean:
      const boolValue = dataView.getUint8(offset++) === 1;
      return { value: boolValue, offset };

    case BinaryLuaType.String:
      const start = offset;
      while (offset < buffer.byteLength && dataView.getUint8(offset) !== 0) {
        offset++;
      }
      const strLength = offset - start;
      const strBuffer = new Uint8Array(buffer, start, strLength);
      let strValue;
      
      if (encoding === 'gbk') {
        strValue = decodeGBK(strBuffer);
      } else {
        strValue = new TextDecoder(encoding, { fatal: false }).decode(strBuffer);
      }
      
      return { value: strValue, offset: offset + 1 };

    case BinaryLuaType.Nil:
      return { value: null, offset };

    case BinaryLuaType.Table:
      const tableSize = dataView.getUint32(offset, true);
      offset += 4;
      const tableEnd = offset + tableSize;
      const table = new Map();
      
      while (offset < tableEnd) {
        const keyResult = readValue(dataView, buffer, offset, encoding);
        offset = keyResult.offset;
        const valueResult = readValue(dataView, buffer, offset, encoding);
        offset = valueResult.offset;
        table.set(keyResult.value, valueResult.value);
      }
      
      return { value: mapToObject(table), offset };

    default:
      throw new Error(`Unknown type: ${type}`);
  }
}

function mapToObject(map: Map<any, any>): any {
  const obj: any = {};
  let isArray = true;
  let maxIndex = 0;
  
  for (const key of map.keys()) {
    if (typeof key !== 'number' || !Number.isInteger(key) || key < 1) {
      isArray = false;
      break;
    }
    if (key > maxIndex) {
      maxIndex = key;
    }
  }
  
  if (isArray && map.size === maxIndex) {
    const arr: any[] = [];
    for (let i = 1; i <= maxIndex; i++) {
      arr.push(map.get(i));
    }
    return arr;
  }
  
  for (const [key, value] of map.entries()) {
    obj[key] = value;
  }
  
  return obj;
}