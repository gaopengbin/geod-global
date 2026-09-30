// Windows extended-length prefixes are for filesystem APIs, not for people.
export function displayLocalPath(value) {
  if (typeof value !== 'string') return '';
  if (value.startsWith('\\\\?\\UNC\\')) return `\\\\${value.slice(8)}`;
  if (value.startsWith('\\\\?\\')) return value.slice(4);
  return value;
}
