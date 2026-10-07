export class ReportDecodeError extends Error {
  constructor(path: string, explanation: string) {
    super(`${path}: ${explanation}`)
    this.name = 'ReportDecodeError'
  }
}

export const objectAt = (value: unknown, path: string): Record<string, unknown> => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new ReportDecodeError(path, 'expected object')
  }
  return value as Record<string, unknown>
}

export const arrayAt = (value: unknown, path: string): unknown[] => {
  if (!Array.isArray(value)) throw new ReportDecodeError(path, 'expected array')
  return value
}

export const stringAt = (value: unknown, path: string): string => {
  if (typeof value !== 'string') throw new ReportDecodeError(path, 'expected string')
  return value
}

export const finiteAt = (value: unknown, path: string): number => {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new ReportDecodeError(path, 'expected finite number')
  }
  return value
}

export const integerAt = (value: unknown, path: string): number => {
  if (typeof value !== 'number' || !Number.isSafeInteger(value)) {
    throw new ReportDecodeError(path, 'expected integer')
  }
  return value
}

export const countAt = (value: unknown, path: string): number => {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new ReportDecodeError(path, 'expected nonnegative integer')
  }
  return value
}

export const nullableCountAt = (value: unknown, path: string): number | null => {
  if (value === null) return null
  try {
    return countAt(value, path)
  } catch {
    throw new ReportDecodeError(path, 'expected nonnegative integer or null')
  }
}

export const scoreAt = (value: unknown, path: string): number | null => {
  if (value === null) return null
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > 100) {
    throw new ReportDecodeError(path, 'expected integer from 0 to 100 or null')
  }
  return value
}

/**
 * An optional field whose value must be one of `words`, absent otherwise. The
 * words are the keys of a `Record<T, true>`, so a type with a new variant
 * cannot be listed here without naming it.
 */
export const optionalWordAt = <T extends string>(
  object: Record<string, unknown>,
  key: string,
  path: string,
  words: Readonly<Record<T, true>>,
): T | undefined => {
  if (!Object.prototype.hasOwnProperty.call(object, key)) return undefined
  const value = object[key]
  const accepted = Object.keys(words)
  if (typeof value !== 'string' || !accepted.includes(value)) {
    throw new ReportDecodeError(`${path}.${key}`, `expected one of ${accepted.join(', ')}`)
  }
  return value as T
}

export const nullableStringAt = (value: unknown, path: string): string | null => {
  if (value === null) return null
  return stringAt(value, path)
}

export const usableDateAt = (value: unknown, path: string): string => {
  const date = stringAt(value, path)
  if (Number.isNaN(Date.parse(date))) {
    throw new ReportDecodeError(path, 'expected usable date string')
  }
  return date
}
