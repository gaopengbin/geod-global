// Pure format declarations shared by the attachment service and renderer.
export const OFFICE_TYPES=Object.freeze({docx:'application/vnd.openxmlformats-officedocument.wordprocessingml.document',xlsx:'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',pptx:'application/vnd.openxmlformats-officedocument.presentationml.presentation'});
export const officeExtension=document=>Object.keys(OFFICE_TYPES).find(extension=>OFFICE_TYPES[extension]===document?.mimeType);
export const AUDIO_TYPES=Object.freeze({wav:'audio/wav',mp3:'audio/mpeg',flac:'audio/flac',ogg:'audio/ogg'});
export const audioExtension=document=>Object.keys(AUDIO_TYPES).find(extension=>AUDIO_TYPES[extension]===document?.mimeType);
export const VIDEO_TYPES=Object.freeze({mp4:'video/mp4',webm:'video/webm'});
export const videoExtension=document=>Object.keys(VIDEO_TYPES).find(extension=>VIDEO_TYPES[extension]===document?.mimeType);
export function videoSignature(bytes,extension){
  return extension==='mp4'?bytes.length>=16 && String.fromCharCode(...bytes.slice(4,8))==='ftyp'
    :extension==='webm'?[0x1a,0x45,0xdf,0xa3].every((byte,index)=>bytes[index]===byte):false;
}
// Signature checks complement the native decoder and content hash. They never
// replace native ingestion or allow an arbitrary URL/path to become a file.
export function audioSignature(bytes,extension){
  const starts=text=>[...text].every((character,index)=>bytes[index]===character.charCodeAt(0));
  return extension==='wav'?starts('RIFF') && String.fromCharCode(...bytes.slice(8,12))==='WAVE'
    :extension==='mp3'?starts('ID3') || bytes.length>1 && bytes[0]===0xff && (bytes[1]&0xe0)===0xe0
    :extension==='flac'?starts('fLaC'):extension==='ogg'?starts('OggS'):false;
}
