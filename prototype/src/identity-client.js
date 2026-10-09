import {desktopAvailable} from './runtime-client.js';
export const signedOutIdentity=()=>({configured:false,ready:false,status:'signed-out',user:null,providers:[],busy:false,remembered:false,error:null});
export function validateIdentity(v){
 if(!v||typeof v.configured!=='boolean'||typeof v.ready!=='boolean'||typeof v.busy!=='boolean'||typeof v.remembered!=='boolean'||
 !['signed-out','waiting','signed-in','expired','unavailable'].includes(v.status)||!Array.isArray(v.providers)||v.providers.length>3||
 v.providers.some(p=>!['email','google','github'].includes(p.id)||typeof p.available!=='boolean')||new Set(v.providers.map(p=>p.id)).size!==v.providers.length||
 (v.status==='signed-in')!==Boolean(v.user)||v.busy!==(v.status==='waiting')||
 Object.keys(v).some(k=>/token|secret|verifier|csrf|authorizeUrl/i.test(k))||
 v.user&&(Object.keys(v.user).some(k=>!['id','provider','name','email','emailVerified','avatar','expiresAt'].includes(k))||typeof v.user.id!=='string'||!/^[a-f0-9]{32}$/i.test(v.user.id)||typeof v.user.name!=='string'||v.user.name.length>500||!['email','google','github'].includes(v.user.provider)||!Number.isSafeInteger(v.user.expiresAt)||v.user.provider==='email'&&(!v.user.email||!v.user.emailVerified)||
 v.user.email!==null&&typeof v.user.email!=='string'||typeof v.user.emailVerified!=='boolean'||
 v.user.avatar!==null&&(typeof v.user.avatar!=='string'||!/^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/]+=*$/.test(v.user.avatar))))
 throw Error('Account status could not be verified.');
 return v;
}
const safeErrors=new Set([
 'Account status could not be verified.', 'Account service is unavailable. Try again.',
 'Invalid account login request.', 'Account sign-in is already pending.',
 'Sign out before signing in with another account.',
 'Secure account storage is unavailable. Clear the checkbox to sign in for this session.',
 'Secure account storage could not be cleared.',
 'The sign-in callback could not be verified. Start a new sign-in.',
 'Invalid desktop sign-in response.',
]);
export async function identityRequest(operation,payload={}){
 if(!desktopAvailable())throw Error('Open the desktop app to sign in.');
 const commands={snapshot:'identity_snapshot',begin:'identity_begin',cancel:'identity_cancel',logout:'identity_logout'};
 if(!commands[operation])throw Error('Unknown account operation.');
 try{return validateIdentity(await window.__TAURI__.core.invoke(commands[operation],payload));}
 catch(error){const message=typeof error==='string'?error:error?.message;throw Error(safeErrors.has(message)?message:'Account service is unavailable. Try again.');}
}
