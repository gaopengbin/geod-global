import { createHash } from 'node:crypto';
import { lstat, readFile } from 'node:fs/promises';
import { join } from 'node:path';

export const IMAGE_LIMITS = Object.freeze({ perTurn:3, perSession:6, bytes:5*1024*1024, sessionBytes:15*1024*1024 });
export const validImageId = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
export function validImage(value) {
  return value && Object.keys(value).every(key => ['id','name','mimeType','bytes','width','height'].includes(key))
    && validImageId(value.id) && typeof value.name === 'string' && value.name.trim() && [...value.name].length<=80 && !/[\u0000-\u001f\u007f/\\]/.test(value.name)
    && value.mimeType==='image/png' && Number.isInteger(value.bytes) && value.bytes>0 && value.bytes<=IMAGE_LIMITS.bytes
    && [value.width,value.height].every(n => Number.isInteger(n) && n>0 && n<=1024);
}
export function sameImage(left,right) {
  return validImage(left) && validImage(right)
    && ['id','name','mimeType','bytes','width','height'].every(key=>left[key]===right[key]);
}
export async function readImage(home, id) {
  if (!validImageId(id)) throw new Error('Invalid Agent image reference.');
  const directory=join(home,'images'), metadataPath=join(directory,`${id}.json`), path=join(directory,`${id}.png`);
  const directoryInfo=await lstat(directory);
  if (!directoryInfo.isDirectory() || directoryInfo.isSymbolicLink()) throw new Error('Invalid Agent image storage.');
  const metadataInfo=await lstat(metadataPath);
  if (!metadataInfo.isFile() || metadataInfo.isSymbolicLink() || metadataInfo.size>1024) throw new Error('Invalid Agent image metadata.');
  const record=JSON.parse(await readFile(metadataPath,'utf8'));
  if (record.version!==1 || Object.keys(record).some(key=>!['version','image'].includes(key)) || !validImage(record.image) || record.image.id!==id) throw new Error('Invalid Agent image metadata.');
  const info=await lstat(path);
  if (!info.isFile() || info.isSymbolicLink() || info.size!==record.image.bytes) throw new Error('Agent image changed or is missing.');
  const bytes=await readFile(path);
  if (bytes.length!==info.size || createHash('sha256').update(bytes).digest('hex')!==id || bytes.length<33
    || !bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])) || bytes.toString('ascii',12,16)!=='IHDR'
    || bytes.readUInt32BE(16)!==record.image.width || bytes.readUInt32BE(20)!==record.image.height) throw new Error('Agent image changed or is missing.');
  return { image:record.image, path };
}
export function sessionImages(entries) {
  const unique=new Map();
  for (const entry of entries) if (entry.images!==undefined) {
    if (entry.type!=='user' || !Array.isArray(entry.images) || entry.images.length>IMAGE_LIMITS.perTurn || entry.images.some(image=>!validImage(image))) throw new Error('Invalid Agent image history.');
    for (const image of entry.images) {
      if (unique.has(image.id) && !sameImage(unique.get(image.id),image)) throw new Error('Agent image history changed.');
      unique.set(image.id,image);
    }
  }
  if (unique.size>IMAGE_LIMITS.perSession || [...unique.values()].reduce((sum,image)=>sum+image.bytes,0)>IMAGE_LIMITS.sessionBytes) throw new Error('This conversation reached its image limit. Start a new conversation.');
  return [...unique.values()];
}
