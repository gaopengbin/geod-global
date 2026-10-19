export function vectorLicenseLabel(asset) {
  if(asset?.licenseUrl==='https://cdla.dev/permissive-2-0/')return 'CDLA Permissive 2.0';
  if(['https://opendatacommons.org/licenses/odbl/1-0/','https://www.openstreetmap.org/copyright'].includes(asset?.licenseUrl))return 'ODbL';
  return 'Publisher terms of use';
}
