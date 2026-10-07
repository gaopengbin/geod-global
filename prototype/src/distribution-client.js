import { desktopAvailable } from './runtime-client.js';
import { RELEASE_VERSION } from './release-policy.js';

export const emptyDistribution = () => ({schemaVersion:1,product:'xyz.laogao.geod.global',version:RELEASE_VERSION,development:true,updateConfigured:false,messagesConfigured:false,automaticChecks:true,notificationsEnabled:true,lastUpdateCheck:null,lastMessagesCheck:null,update:{state:'idle'},updateRead:false,busy:false,items:[],unreadCount:0});
const phases=['idle','checking','unconfigured','upToDate','available','downloading','verifying','ready','installing','error'];
export function validateDistribution(value) {
  const text=value=>value&&typeof value.en==='string'&&typeof value['zh-CN']==='string';
  if (!value || value.schemaVersion!==1 || value.product!=='xyz.laogao.geod.global' || typeof value.version!=='string'
    || ['development','updateConfigured','messagesConfigured','automaticChecks','notificationsEnabled','updateRead','busy'].some(key=>typeof value[key]!=='boolean')
    || !phases.includes(value.update?.state) || !Array.isArray(value.items) || value.items.length>100
    || value.items.some(item=>!item || typeof item.id!=='string' || !Number.isSafeInteger(item.revision) || item.revision<1
      || !text(item.title) || !text(item.body) || !['normal','important'].includes(item.priority)
      || !Number.isFinite(Date.parse(item.publishedAt)) || typeof item.read!=='boolean' || typeof item.seen!=='boolean'
      || item.action!==null && !['updates','sources'].includes(item.action))
    || new Set(value.items.map(item=>item.id)).size!==value.items.length
    || value.unreadCount!==value.items.filter(item=>!item.read).length) throw new Error('Notification response is invalid.');
  return value;
}
export async function distributionRequest(command, args={}) {
  if (!desktopAvailable()) {
    if(command==='distribution_snapshot')return emptyDistribution();
    throw new Error('Updates and notifications are available in the desktop app.');
  }
  const value=await window.__TAURI__.core.invoke(command,args);
  return ['distribution_snapshot','distribution_preferences','notifications_refresh','notifications_read'].includes(command)?validateDistribution(value):value;
}
