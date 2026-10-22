// Local UI acceptance only. This entry is never included in the application build.
import React from 'react';
import {createRoot} from 'react-dom/client';
import {I18nProvider} from '../src/i18n.jsx';
import {DistributionProvider,UpdatesPanel} from '../src/distribution-ui.jsx';
import {emptyDistribution} from '../src/distribution-client.js';
import '../src/ui/foundation.css';
import '../src/styles.css';
document.documentElement.dataset.theme=new URLSearchParams(location.search).get('theme')||'dark';
const fixture={...emptyDistribution(),development:false,updateConfigured:true,version:'0.2.0',update:{state:'available',version:'0.2.1',notes:'改进地图预览与重试；补充主要流程失败诊断；新增签名更新与用户确认升级。'}};
let notify=()=>{};
window.__TAURI__={event:{listen:async(_,listener)=>{notify=listener;return()=>{};}},core:{invoke:async(command,args)=>{
  if(command==='update_download'){fixture.update={...fixture.update,state:'ready',verified:true,mode:'delta'};notify();}
  if(command==='update_install')throw new Error('界面验收：不会安装软件。实际安装由独立原生验收执行。');
  if(command==='distribution_preferences')Object.assign(fixture,args);
  return structuredClone(fixture);
}}};
localStorage.setItem('geod-global-locale','zh-CN');
createRoot(document.getElementById('root')).render(<I18nProvider><DistributionProvider><main style={{maxWidth:720,margin:'32px auto',padding:16}}><p>更新弹窗界面验收 · 模拟版本状态，不会下载或安装软件</p><UpdatesPanel/></main></DistributionProvider></I18nProvider>);
