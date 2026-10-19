import {desktopAvailable} from './runtime-client.js';

export const officialErrors={
 sign_in_required:'Sign in to use official services.',
 pro_required:'This official service requires Pro. Local tools remain available.',
 service_not_deployed:'The official aggregation service is not deployed yet.',
 identity_unavailable:'Your account could not be verified. Retry when the account service is available.',
 checkout_not_available:'Payments are not configured. No payment has been started.',
 sandbox_invitation_required:'Sandbox checkout requires an invitation from the service operator.',
 credits_required:'Not enough service credits. Existing local results remain available.',
 budget_limit:'The confirmed monitoring budget has been reached.',
 quote_changed:'The task changed. Prepare a new review.',
 quote_expired:'This review expired. Prepare it again.',
 not_found:'This task could not be found in your account.',
 monitor_limit:'The maximum number of monitored areas has been reached.',
 monitor_paused:'This monitor is paused. Resume it before checking.',
 check_in_progress:'A check is already running for this area.',
 rate_limited:'Too many requests. Please wait a moment and retry.',
 area_too_large:'Choose an area spanning no more than 30 degrees in each direction.',
 invalid_geometry:'Choose a valid polygon or rectangle.',
 area_too_complex:'This boundary has too many vertices for the monitoring service.',
 invalid_request:'Check the task fields and try again.',
 request_too_large:'This task is too large. Choose a simpler boundary.',
 subscription_already_active:'Your Pro term is already active. Recharge service credits separately.',
 checkout_pending:'A Pro checkout is pending. Restore purchases before starting another.',
 account_service_paused:'Official services are paused for this account. Contact support.',
 service_unavailable:'Official service is unavailable.',
};
const operations=new Set(['capabilities','account','monitors','quoteMonitor','acceptMonitor','controlMonitor','runMonitor','notifications','readNotifications','checkout','orders','restorePurchases','refundRequest','quotes']);
export async function officialRequest(operation,payload={}){
 if(!operations.has(operation))throw Error('Invalid official service request.');
 if(!desktopAvailable())throw Error('Open the desktop app to use official services.');
 let value;
 try{value=await window.__TAURI__.core.invoke('official_request',{operation,payload});}
 catch{throw Error('Official service is unavailable.');}
 if(!value||value.error){throw Error(officialErrors[value?.error]||'Official service is unavailable.');}
 if(value.product!=='xyz.laogao.geod.global'||value.version!==1)throw Error('Official service response could not be verified.');
 return value;
}

export function rectangleGeometry(bounds){
 const [west,south,east,north]=bounds;
 return {type:'Polygon',coordinates:[[[west,south],[east,south],[east,north],[west,north],[west,south]]]};
}
