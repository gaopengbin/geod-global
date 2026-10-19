import React, { lazy, Suspense, useContext, useEffect, useLayoutEffect, useId, useRef, useState } from 'react';
import {sceneKind} from '../../agent/scenes.mjs';
import {sceneSuggestions} from './agent-scenes.js';
import { ArrowUpRight, AudioLines, Bot, Check, CheckCircle2, ChevronDown, CircleAlert, Copy, Crop, Download, File, FileText, Film, FolderOpen, HardDrive, Headphones, History, ImageOff, KeyRound, Layers, ListChecks, ListTodo, Map as MapIcon, MessageSquare, PanelLeftClose, PanelRightClose, PlugZap, Plus, Search, Settings, Settings2, ShieldAlert, ShieldCheck, Shrink, Square, Trash2, X } from 'lucide-react';
import { InfoDetails, Badge, Button, Disclosure, Input, MessageScroller, Modal, Popover, Progress, PromptInput, Select, Spinner, Switch } from './ui/index.jsx';
import { useI18n } from './i18n.jsx';
import { desktopAvailable, formatBytes, runtimeRequest } from './runtime-client.js';
import { Compass, MoreHorizontal } from 'lucide-react';
import { SourcePickerDialog } from './source-picker.jsx';
import { agentRequest, subscribeAgentUpdates } from './agent-client.js';
import { transcriptTokenUsage } from './agent-usage.js';
import { TurnTokenUsage } from './ui/agent-token-usage.jsx';
import { AgentDecisionCard } from './agent-decision-card.jsx';
import { AgentGoalCard } from './agent-goal-card.jsx';
import { conversationDeliveries } from './agent-deliveries.js';
import { toolObservations } from './agent-tool-records.js';
import { transcriptRows, activeWorkId, workEntryNeedsAttention } from './agent-chat-work.js';
import {downloadWork,downloadTaskSummary} from './agent-download-tasks.js';
import {AgentDownloadTasks} from './agent-download-tasks.jsx';
import { ChatMessage, MessageTyping, WorkRecords, WorkRecord, WorkDetailsSection, LiveStatus, TranscriptStage } from './ui/agent-chat.jsx';
import { FileThumbnail } from './file-thumbnail.jsx';
import { RuntimeContext } from './runtime-context.js';
import { localRasterKeys } from './reflectance.js';
import {ConversationMonitorReviews} from './official-ui.jsx';
import { decisionReviewIssue } from '../../agent/decisions.mjs';
import {officeExtension,audioExtension,videoExtension} from '../../agent/file-types.mjs';
import { MODEL_PROVIDERS, MODEL_PROTOCOLS, groupedConnections, providerLabel, validProviderProtocol } from './agent-model-registry.js';
import { MIN_CONTEXT_WINDOW, MAX_CONTEXT_WINDOW, defaultContextWindow, modelContextPolicy } from '../../agent/model-context.mjs';
const AgentPdfPreview=lazy(()=>import('./agent-pdf-preview.jsx'));
const AgentAudioPreview=lazy(()=>import('./agent-audio-preview.jsx'));
const AgentVideoPreview=lazy(()=>import('./agent-video-preview.jsx'));
import { ArtifactPackageButton } from './artifact-ui.jsx';
import {DeliveryReceipt} from './acquisition-ui.jsx';
import './agent-panel.css';
const TOOLS = { geod_wcs_connections:"Read connected coverage services",geod_wcs_coverages:"Read coverage catalog",geod_wcs_description:"Read coverage definition",geod_wcs_plan:"Read saved coverage request",geod_wcs_describe:"Discover coverage definition",geod_wcs_prepare:"Prepare coverage grid",geod_wcs_project_plan:"Prepare coverage project",geod_wcs_download_plan:"Prepare coverage downloads",geod_wcs_inspect:"Inspect coverage raster",geod_wcs_pixel:"Read coverage pixel", geod_vector_extract_plan:'Prepare vector extraction', geod_feature_services:'Read connected vector services', geod_feature_collections:'Read vector collections', geod_vectors_list:'Read local vector files', geod_vector_inspect:'Verify vector file', geod_vector_features:'Read vector features', geod_vector_node:'Read vector attributes and geometry', geod_stac_connections:'Read connected raster sources', geod_stac_catalog:'Read source catalog', geod_stac_snapshot:'Read original item metadata', geod_stac_assets:'Read original asset declarations', geod_stac_inspect:'Inspect custom raster', geod_stac_pixel:'Read custom raster pixel', geod_stac_search:'Search connected raster source', geod_stac_project_plan:'Prepare custom source project', geod_stac_download_plan:'Prepare custom source downloads', geod_scientific_rgb_plan:'Prepare scientific RGB', geod_rgb_inspect:'Inspect scientific RGB', geod_rgb_pixel:'Read original RGB pixel', geod_recipe_review_plan:'Prepare recipe review', geod_sources_list:'Check data source capabilities', geod_health:'Check workspace', geod_jobs_list:'Read tasks', geod_job_status:'Check task status', geod_projects_list:'Read projects', geod_project_get:'Read project details', geod_raster_inspect:'Inspect raster file', geod_raster_pixel:'Read original pixel', geod_recipes_list:'Read processing recipes', geod_recipe_plan:'Check processing plan', geod_place_search:'Find requested place', geod_workspace_context:'Read current map area', geod_scene_search:'Search imagery', geod_download_plan:'Prepare download plan', geod_clip_plan:'Prepare crop plan', geod_plan_status:'Check plan status', geod_project_plan:'Prepare project selection', geod_project_download_plan:'Prepare project downloads', geod_project_mosaic_plan:'Prepare project processing' };

Object.assign(TOOLS,{geod_request_decision:'Choose task options',geod_region_search:'Find administrative area',geod_region_levels:'Read administrative levels',geod_execution_policy:'Read execution permission',geod_plan_execute:'Execute native plan',geod_job_control:'Control native task',geod_workspace_open:'Open result on map'});
Object.assign(TOOLS,{geod_goal_define:'Save task goal',geod_goal_bind:'Track required output',geod_goal_check:'Verify complete goal'});
Object.assign(TOOLS,{geod_boundary_read:'Read administrative boundary'});
Object.assign(TOOLS,{geod_official_services:'Check official service entitlement',geod_monitor_list:'Read monitored areas',geod_monitor_notifications:'Read imagery updates',geod_monitor_quote:'Prepare monitoring review'});
Object.assign(TOOLS,{geod_scene_coverage:'Check entire area coverage',geod_scene_search_more:'Continue imagery search',geod_catalog_selection:'Read selected imagery'});
Object.assign(TOOLS,{geod_gscloud_products:'Read GSCloud products',geod_gscloud_search:'Search GSCloud originals',geod_gscloud_detail:'Read GSCloud details',geod_gscloud_download_plan:'Prepare GSCloud download',geod_gscloud_archive_inspect:'Verify GSCloud archive'});
Object.assign(TOOLS,{geod_dataset_catalog:'Read public data products',geod_dataset_resolve:'Find published product files',geod_dataset_connect:'Check and archive product sources'});
const TOOL_FAILURES={'record-storage':'Could not save this query. Local storage needs attention.',
  'argument-schema':'Tool parameters do not match; no plan was created.',
  'pending-decision':'Answer the pending choices before preparing this task.',
  'city-scope':'The requested city has not been located. Imagery search was stopped.',
  'stale-search':'This search has expired. Refresh the search before preparing the new plan.',
  'expired-plan':'This plan has expired. Prepare a fresh review before confirming.',
  'crop-area':'Choose the crop extent before preparing this plan.',
  'task-goal':'Update the task requirements before preparing the new plan.',
  'source-network':'The data service could not be reached. Try again after the connection recovers.'};
function ModelDialog({ model, registry, onSaved, onClose }) {
  const { t } = useI18n();
  const formId=useId();
  const blank = () => ({provider:'openai',label:'',protocol:MODEL_PROVIDERS[0].protocol,baseUrl:MODEL_PROVIDERS[0].baseUrl,model:'',apiKey:'',contextWindow:''});
  const fromConnection = connection => ({provider:connection.provider || 'custom',label:connection.label,protocol:connection.protocol,baseUrl:connection.baseUrl,model:connection.model,apiKey:'',contextWindow:connection.contextWindow == null ? '' : String(connection.contextWindow)});
  const [editing, setEditing] = useState(model?.id ?? 'new');
  const [form, setForm] = useState(model?.id ? fromConnection(model) : blank());
  const [operation, setOperation] = useState(''), [error, setError] = useState(''), [removing, setRemoving] = useState(false);
  const [testResult, setTestResult] = useState(null);
  const busy = Boolean(operation), formRef = useRef(null);
  const saving = useRef(false);
  const requestForm = () => ({...form,contextWindow:form.contextWindow === '' ? null : Number(form.contextWindow)});
  const contextDefault = defaultContextWindow(form);
  const contextSize = form.contextWindow === '' ? contextDefault.contextWindow : Number(form.contextWindow);
  const contextValid = Number.isSafeInteger(contextSize) && contextSize >= MIN_CONTEXT_WINDOW && contextSize <= MAX_CONTEXT_WINDOW;
  const contextPolicy = contextValid ? modelContextPolicy({...form,contextWindow:contextSize}) : null;
  const contextFormat = size => size.toLocaleString('en-US');
  const update = (key, value) => { setTestResult(null); setError(''); setForm(form => ({ ...form, [key]:value })); };
  const choose = id => {
    setEditing(id); setError(''); setTestResult(null); setRemoving(false);
    setForm(id === 'new' ? blank() : fromConnection(registry.connections.find(connection => connection.id === id)));
  };
  const saved = async request => {
    if (saving.current) return;
    saving.current = true; setOperation('save'); setError('');
    try { const result = await agentRequest('saveModel', { request }); setForm(current => ({...current,apiKey:''})); onSaved(result); onClose(); }
    catch (error) { setError(String(error.message || error)); }
    finally { saving.current = false; setOperation(''); }
  };
  const test = async () => {
    if (saving.current || !formRef.current?.reportValidity()) return;
    saving.current = true; setOperation('test'); setError(''); setTestResult(null);
    try { setTestResult(await agentRequest('testModel',{request:{...requestForm(),...(editing !== 'new' ? {id:editing} : {})}})); }
    catch (error) { setError(String(error.message || error)); }
    finally { saving.current = false; setOperation(''); }
  };
  const submit = async event => {
    event.preventDefault(); if (!formRef.current?.reportValidity()) return; await saved({...requestForm(),action:'save',...(editing !== 'new' ? {id:editing} : {})});
  };
  return <Modal title={t('Agent model connection')} description={t('Connect a model that supports text and tool calls.')} closeLabel={t('Close')} onClose={onClose} closeDisabled={busy} className="agent-model-dialog" footer={<div className="agent-model-footer">
      <div className="agent-model-test"><Button icon={operation==='test'?undefined:PlugZap} disabled={busy || removing} onClick={test}>{operation==='test' && <Spinner size={15}/>} {t(operation==='test'?'Testing…':'Test connection')}</Button>
        <div className="agent-model-test-copy" data-status={testResult?.status} role={testResult?.status==='failed'?'alert':'status'}>
          {testResult ? <><strong>{testResult.status==='passed'?<CheckCircle2 size={15}/>:<CircleAlert size={15}/>} {t(testResult.status==='passed'?'Connection test passed':testResult.text?'Text connected; tool test incomplete':'Connection test failed')}</strong><small>{testResult.status==='passed' ? t('Text and tool calls · {time} ms',{time:testResult.latencyMs}) : t(testResult.message)}</small></> : <small>{t(operation==='test'?'Checking model response and tool calls…':'Sends two fixed test requests. No chat or map data; model fees may apply.')}</small>}
        </div>
      </div>
      <div className="agent-model-actions">{editing !== 'new' && <Button className="agent-model-remove" disabled={busy} onClick={() => removing ? saved({action:'delete',id:editing}) : setRemoving(true)}>{t(removing ? 'Confirm removal' : 'Remove connection')}</Button>}<Button disabled={busy} onClick={onClose}>{t('Cancel')}</Button><Button primary type="submit" form={formId} disabled={busy || removing}>{operation==='save' && <Spinner size={15}/>} {t('Save connection')}</Button></div>
    </div>}>
    <form ref={formRef} id={formId} className="agent-model-form" onSubmit={submit}>
      {registry?.connections.length > 0 && <label className="agent-connection-field">{t('Saved connection')}<Select descriptionLayout="stacked" aria-label={t('Saved connection')} value={editing} displayValue={editing==='new'?t('Add model connection'):`${form.model} · ${form.label}`} onChange={event => choose(event.target.value)} disabled={busy}>
        <option value="new">{t('Add model connection')}</option>{groupedConnections(registry.connections).map(provider => <optgroup key={provider.id} label={t(provider.label)}>{provider.connections.map(connection => <option key={connection.id} value={connection.id} data-description={connection.label}>{connection.model}</option>)}</optgroup>)}
      </Select></label>}
      <label>{t('Provider')}<Select aria-label={t('Agent model provider')} value={form.provider} onChange={event => { const provider=MODEL_PROVIDERS.find(provider=>provider.id===event.target.value); setTestResult(null); setError(''); setForm(current=>({...current,provider:provider.id,protocol:provider.protocol,baseUrl:provider.baseUrl,apiKey:''})); }} disabled={busy}>{MODEL_PROVIDERS.map(provider=><option key={provider.id} value={provider.id}>{t(provider.label)}</option>)}</Select></label>
      <label>{t('Protocol')}<Select value={form.protocol} aria-label={t('Agent model protocol')} disabled={busy || !['custom','openai'].includes(form.provider)} onChange={event=>{const protocol=event.target.value;if(validProviderProtocol(form.provider,protocol)){setTestResult(null);setError('');setForm(current=>({...current,protocol,apiKey:''}));}}}>{MODEL_PROTOCOLS.filter(protocol=>validProviderProtocol(form.provider,protocol.id)).map(protocol=><option key={protocol.id} value={protocol.id}>{t(protocol.label)}</option>)}</Select></label>
      <label>{t('Connection name')}<Input autoFocus value={form.label} disabled={busy} onChange={event => update('label', event.target.value)} maxLength={80} required placeholder={t('For example: My model gateway')}/></label>
      <label>{t('Model ID')}<Input value={form.model} disabled={busy} onChange={event => update('model', event.target.value)} maxLength={160} required placeholder={t('Use the exact ID from your provider')} autoComplete="off" spellCheck={false}/></label>
      <label className="agent-endpoint-field">{t('API endpoint')}<Input value={form.baseUrl} disabled={busy} onChange={event => update('baseUrl', event.target.value)} maxLength={1000} required placeholder="https://api.example.com/v1" autoComplete="off" spellCheck={false}/></label>
      <div className="agent-context-field">
        <label>{t('Context size (tokens)')}<Input type="number" min={MIN_CONTEXT_WINDOW} max={MAX_CONTEXT_WINDOW} step={1} value={form.contextWindow} disabled={busy} onChange={event => update('contextWindow', event.target.value)} placeholder={t('Automatic · {size}',{size:contextFormat(contextDefault.contextWindow)})} aria-describedby={`${formId}-context-help`} autoComplete="off"/></label>
        <div id={`${formId}-context-help`} className="agent-context-help">
          {contextPolicy && <p>{t('Effective context: {size} tokens · organizes at {threshold}',{size:contextFormat(contextSize),threshold:contextFormat(contextPolicy.autoCompactTokenLimit)})}</p>}
          <p>{t(form.contextWindow === '' && !contextDefault.known ? 'Model capacity is unknown. Automatic uses 32,768 tokens; enter your provider’s limit to use a larger context.' : 'Leave blank for automatic. Use the limit supported by your model or gateway. Space is reserved for model output.')}</p>
        </div>
      </div>
      <label className="agent-key-field">{t('API key')}<Input type="password" value={form.apiKey} disabled={busy} onChange={event => update('apiKey', event.target.value)} maxLength={4096} required={editing === 'new'} autoComplete="new-password" placeholder={t(editing !== 'new' ? 'Leave blank to keep the saved key' : 'Stored in Windows Credential Manager')}/></label>
      <p className="agent-model-help">{t('Each connection keeps its own key. Enter a new key when changing the endpoint or provider. Removing a connection keeps its conversation history.')}</p>
      <InfoDetails closeLabel={t('Close')} className="agent-model-help" summary={t('Connection details')}><p>{t('Your questions and requested tool results are sent to this model connection. Source URLs, local paths and account credentials are excluded from tool results.')}</p><p>{t(form.protocol==='openai-responses' ? 'OpenAI Responses keeps encrypted reasoning in private recovery records with server-side response storage disabled. Images and tools still depend on your model. Existing Chat Completions connections keep their protocol and history.' : 'Adapter capabilities: text, selected images, function tools, plaintext reasoning and native context organization. Provider signatures are saved for recovery. OpenAI encrypted reasoning is not supported. Image and tool support depend on your selected model.')}</p><p>{t('The connection test checks text and a harmless tool call. It does not verify file reading or geographic workflows, and does not save changes.')}</p></InfoDetails>
      {error && <p role="alert" className="agent-error">{t(error)}</p>}
      {removing && <p className="agent-model-help">{t('Remove this connection and its saved key? Conversation history is kept.')}</p>}
    </form>
  </Modal>;
}
function PlanRevisionDialog({ draft, onSave, onClose }) {
  const { t, date } = useI18n();
  const [form,setForm]=useState(()=>({...draft.parameters,itemIds:[...new Set(draft.parameters.itemIds??[])],bounds:draft.parameters.bounds?.map(String)}));
  const [busy,setBusy]=useState(false),[error,setError]=useState('');const saving=useRef(false);
  const fields=draft.fields;
  const update=(key,value)=>setForm(current=>({...current,[key]:value}));
  const items=[...new Map((fields.items??[]).map(item=>[item.id,item])).values()];
  const originalAssets=items.length>0&&items.every(item=>item.label);
  const submit=async event=>{
    event.preventDefault();if(saving.current)return;
    const revision={kind:draft.kind};
    if(fields.name)revision.name=form.name;
    if(fields.bounds)revision.bounds=form.bounds.map(Number);
    if(['clip','project','vector'].includes(draft.kind))revision.keepPolygon=Boolean(form.keepPolygon);
    if(fields.items)revision.itemIds=form.itemIds;
    if(fields.assetKeys)revision.assetKey=form.assetKey;
    if(fields.qualityPolicies)revision.qualityPolicy=form.qualityPolicy;
    if(fields.snow)revision.excludeSnow=form.excludeSnow;
    saving.current=true;setBusy(true);setError('');
    try{await onSave(revision);onClose();}catch(error){setError(String(error.message||error));}
    finally{saving.current=false;setBusy(false);}
  };
  const qualityLabels={cloud_free:'Landsat standard QA',cloud_free_conservative:'Landsat conservative QA',clear:'MODIS clear state',clear_best:'MODIS clear + best QC',good:'VI good observations',usable:'VI good + marginal observations'};
  return <Modal title={t('Edit review parameters')} description={t('Save changes to check the source again and create a new review. Confirm that review separately before running.')} onClose={onClose} closeDisabled={busy} closeLabel={t('Close')} className="agent-revision-dialog">
    <form className="agent-revision-form" onSubmit={submit}>
      {fields.name && <label>{t('Output name')}<Input value={form.name} onChange={event=>update('name',event.target.value)} maxLength={80} required disabled={busy}/></label>}
      {fields.bounds && <fieldset className="agent-revision-bounds"><legend>{t('Area coordinates')} · {draft.boundsCrs}</legend>{['Minimum X','Minimum Y','Maximum X','Maximum Y'].map((label,index)=><label key={label}>{t(label)}<Input type="number" step="any" value={form.bounds[index]} required disabled={busy} onChange={event=>update('bounds',form.bounds.map((value,i)=>i===index?event.target.value:value))}/></label>)}</fieldset>}
      {fields.polygon && <label className="agent-revision-toggle"><span>{t('Keep the saved polygon boundary')}</span><Switch aria-label={t('Keep the saved polygon boundary')} checked={form.keepPolygon} onCheckedChange={value=>update('keepPolygon',value)} disabled={busy}/></label>}
      {fields.items && <fieldset className="agent-revision-items"><legend>{t(originalAssets?'Files included in this review':'Scenes included in this review')}</legend>{items.map(item=><label className="agent-revision-toggle" key={item.id}><span title={item.label || item.id}>{item.label || item.id}<small>{date(item.date)}{item.locked&&` · ${t('Already in project')}`}</small></span><Switch aria-label={item.label || item.id} checked={form.itemIds.includes(item.id)} disabled={busy||item.locked} onCheckedChange={checked=>update('itemIds',checked?[...form.itemIds,item.id]:form.itemIds.filter(id=>id!==item.id))}/></label>)}</fieldset>}
      {fields.assetKeys && <label>{t('Source asset')}<Select aria-label={t('Source asset')} value={form.assetKey} onChange={event=>update('assetKey',event.target.value)} disabled={busy}>{fields.assetKeys.map(key=><option key={key} value={key}>{key.toUpperCase()}</option>)}</Select></label>}
      {fields.qualityPolicies && <label>{t('Quality policy')}<Select aria-label={t('Quality policy')} value={form.qualityPolicy} onChange={event=>update('qualityPolicy',event.target.value)} disabled={busy}>{fields.qualityPolicies.map(policy=><option key={policy} value={policy}>{t(qualityLabels[policy])}</option>)}</Select></label>}
      {fields.snow && <label className="agent-revision-toggle"><span>{t('Exclude snow')}</span><Switch aria-label={t('Exclude snow')} checked={form.excludeSnow} onCheckedChange={value=>update('excludeSnow',value)} disabled={busy}/></label>}
      {error&&<p role="alert" className="agent-error">{t(error)}</p>}
      <div className="agent-model-actions"><Button onClick={onClose} disabled={busy}>{t('Cancel')}</Button><Button type="submit" primary disabled={busy||fields.items&&!form.itemIds.length}>{busy&&<Spinner size={15}/>} {t('Save and review again')}</Button></div>
    </form>
  </Modal>;
}
function OutputActions({ jobId, projectId, packageKind, archive=false }) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false), [error, setError] = useState('');
  const reveal = async () => {
    if (busy) return;
    setBusy(true); setError('');
    try { await runtimeRequest('reveal', { id:jobId }); }
    catch (error) { setError(String(error.message || error)); }
    finally { setBusy(false); }
  };
  return <div className="agent-output-actions">
    {!archive&&<Button asChild size="icon" variant="secondary" tooltip={t('Open in workspace')}><a aria-label={t('Open in workspace')} href={`#Workspace?file=${encodeURIComponent(jobId)}${projectId ? `&project=${encodeURIComponent(projectId)}` : ''}`}><Layers size={15} aria-hidden="true"/></a></Button>}
    {desktopAvailable() && <Button size="icon" variant="secondary" icon={FolderOpen} tooltip={t('Show in folder')} aria-label={t('Show in folder')} onClick={reveal} disabled={busy}/>}
    {packageKind && <ArtifactPackageButton compact job={{id:jobId,kind:packageKind}}/>}
    {error && <p role="alert" className="agent-error">{t(error)}</p>}
  </div>;
}
function TaskSummary({ context, scope }) {
  const {t}=useI18n();
  return <div className="agent-task-summary">
    {(context?.requestText||context?.choices?.length>0)&&<section><h4>{t('Your task')}</h4><div className="agent-task-context" tabIndex={0} role="region" aria-label={t('Task requirements')}>
      {context.requestText&&<p>{context.requestText}</p>}
      {context.choices?.length>0&&<div><strong>{t('Agreed choices')}</strong>{context.choices.map(choice=><p key={`${choice.decisionId}:${choice.questionId}`}><span>{choice.prompt}</span><b>{choice.answer}</b></p>)}</div>}
    </div></section>}
    <p>{t(scope)}</p>
  </div>;
}
function ConversationResults({ items, onOpenResult, onOpenProject, onShowPlan }) {
  const {t,number,locale}=useI18n(), runtime=useContext(RuntimeContext);
  const main=items.filter(item=>!item.intermediate), originals=items.filter(item=>item.intermediate);
  const card=item=><article className="agent-result-card" key={item.key} aria-label={item.label}>
    {runtime&&item.kind==='raster'&&localRasterKeys.includes(item.assetKey)&&/^[a-f0-9]{64}$/.test(item.sha256??'')&&<FileThumbnail job={{...item,title:item.label}}/>}
    <div className="agent-result-title"><span>{item.kind==='project'?<FolderOpen size={16}/>:item.kind==='vector'?<Layers size={16}/>:<FileText size={16}/>}</span><strong>{t(item.label)}</strong><CheckCircle2 size={14} aria-label={t('Output ready')}/></div>
    {item.source&&<p className="agent-result-source">{t(item.source)}</p>}
    <p className="agent-result-facts">{item.kind==='project'?t('Project saved'):item.kind==='vector'?t('Verified vector file'):item.format??t('Verified local file')}
      {Number.isFinite(item.bytes)&&` · ${formatBytes(item.bytes,locale)}`}
      {item.width&&item.height&&<span>{number(item.width)} × {number(item.height)} px</span>}
      {Number.isInteger(item.count)&&<span>{t('{count} features',{count:number(item.count)})}</span>}
    </p>
    <div className="agent-result-actions">
      {item.kind==='project'?<Button size="sm" variant="secondary" icon={FolderOpen} onClick={()=>onOpenProject(item.id)}>{t('Open project')}</Button>:<>
        {onOpenResult&&<Button size="sm" variant="secondary" icon={MapIcon} onClick={()=>onOpenResult({kind:item.kind,id:item.id,verified:true})}>{t('Preview result')}</Button>}
        {item.kind==='vector'?<Button asChild size="icon" variant="secondary" tooltip={t('Open in workspace')}><a href={`#Workspace?vector=${item.id}`} aria-label={t('Open in workspace')}><Layers size={15}/></a></Button>:<OutputActions archive={item.kind==='archive'} jobId={item.id} projectId={item.projectId} packageKind={item.packageKind}/>}
      </>}
      {item.planId&&<Button size="icon" variant="quiet" icon={ListTodo} aria-label={t('View result task')} tooltip={t('View result task')} onClick={()=>onShowPlan(item.planId)}/>}
    </div>
  </article>;
  return <section className="agent-results" aria-label={t('Conversation results')}>
    <div className="agent-results-heading"><strong>{t('Ready files and projects')}</strong><Badge tone="green">{number(main.length||originals.length)}</Badge></div>
    {items.length===0&&<p className="agent-results-empty">{t('Completed files will appear here. Pending plans stay in the conversation.')}</p>}
    <div className="agent-results-list">{main.map(card)}</div>
    {originals.length>0&&<Disclosure className="agent-result-originals" summary={t('Intermediate source files · {count}',{count:number(originals.length)})}><div className="agent-results-list">{originals.map(card)}</div></Disclosure>}
  </section>;
}
function PlanCard({ plan, disabled, confirming, onConfirm, onEdit, onPreview, onOpenProject, onOpenTasks, taskContext, reviewIssue, suppressOutputs=false }) {
  const { t, date, number, locale } = useI18n();
  if (plan.status === 'unavailable') return <p className="agent-error">{t('This plan is unavailable. Ask the Agent to create a new plan.')}</p>;
  if (plan.kind === 'vector') return <VectorPlanCard plan={plan} taskContext={taskContext} reviewIssue={reviewIssue} disabled={disabled} confirming={confirming} onConfirm={onConfirm} onEdit={onEdit} suppressOutputs={suppressOutputs}/>;
   const authorizationRequired=plan.authorization && (!plan.authorization.downloadEnabled || Date.parse(plan.authorization.expiresAt)<=Date.now());
   const Icon = {clip:Crop,download:Download,project:FolderOpen,mosaic:Layers,rgb:Layers}[plan.kind];
  const title = {clip:'Crop plan',download:'Complete download task',project:'Project selection',mosaic:'Project processing',rgb:'Scientific RGB'}[plan.kind];
  const confirmation = {clip:'Confirm crop',download:'Confirm download',project:plan.project?.mode === 'append' ? 'Confirm selection' : 'Confirm project',mosaic:'Confirm processing',rgb:'Confirm RGB'}[plan.kind];
  const qualityLabels = {cloud_free:'Landsat standard QA',cloud_free_conservative:'Landsat conservative QA',clear:'MODIS clear state',clear_best:'MODIS clear + best QC',good:'VI good observations',usable:'VI good + marginal observations'};
   const coverage=Boolean(plan.files[0].coveragePlanId);
   const countsScenes=plan.kind==='project'&&!plan.files[0].originalAssetKey&&!coverage;
  const execution=plan.status==='submitted' && plan.kind!=='project' && plan.jobs.length>0;
  const complete=execution && plan.jobs.every(job=>job.status==='succeeded' && job.settled);
  const areaBlocked=plan.areaCoverage && plan.areaCoverage.status!=='complete';
  const attention=execution && plan.jobs.some(job=>job.status==='failed');
  const active=execution && plan.jobs.some(job=>['queued','running'].includes(job.status));
  const finishing=execution && plan.jobs.some(job=>job.status==='succeeded' && !job.settled);
  const planLabel=plan.status==='pending'?'Awaiting confirmation':plan.status==='expired'?'Expired':plan.status==='superseded'?'Replaced by revised review'
    :plan.kind==='project'?'Project saved':complete?'Execution complete':attention?'Needs attention':active?'Tasks in progress':finishing?'Finalizing output':execution?'Tasks stopped':'Submitted';
  return <section className="agent-plan" aria-label={t(title)} data-plan-id={plan.planId} tabIndex={-1}>
    <div className="agent-plan-heading"><span><Icon size={16}/><strong>{t(title)}</strong></span><Badge tone={attention?'warning':plan.status==='pending'||active||finishing||complete?'blue':'neutral'}>{t(planLabel)}</Badge></div>
    {plan.project && <p className="agent-plan-project" title={plan.project.name}><FolderOpen size={14}/><strong>{plan.project.name}</strong></p>}
    <p className="agent-plan-source">{t(plan.source)}</p>
     <div className="agent-plan-facts"><span>{plan.kind === 'rgb' ? t('3 original bands · RGB') : <>{t(countsScenes ? plan.files.length === 1 ? '1 scene' : '{count} scenes' : plan.files.length === 1 ? '1 file' : '{count} files', {count:number(plan.files.length)})}{plan.kind !== 'project' && ` · ${coverage ? t('Coverage subset') : plan.files[0].assetKey === 'stac_asset' ? t('Original raster') : plan.archive || plan.authorization ? plan.format : plan.files[0].assetKey.toUpperCase()}`}</>}</span>{plan.kind !== 'project' && <strong>{plan.expectedBytes === null ? coverage && plan.files.length>1 ? t('Native grids') : Number.isInteger(plan.files[0].width) && Number.isInteger(plan.files[0].height) ? `${number(plan.files[0].width)} × ${number(plan.files[0].height)} px` : t('Size available after download') : formatBytes(plan.expectedBytes,locale)}</strong>}</div>
     {plan.authorization && plan.status!=='submitted' && <div className="agent-plan-facts"><Badge tone={authorizationRequired ? 'warning' : 'neutral'}>{t(authorizationRequired ? 'Authorization required' : 'Authorization saved')}</Badge><Button asChild size="icon" variant="quiet" tooltip={t(plan.authorization.provider==='nasa-earthdata' ? 'Manage NASA Earthdata authorization' : 'Manage Copernicus authorization')}><a href={`#Settings?account=${plan.authorization.provider}`} aria-label={t(plan.authorization.provider==='nasa-earthdata' ? 'Manage NASA Earthdata authorization' : 'Manage Copernicus authorization')}><KeyRound size={15} aria-hidden="true"/></a></Button></div>}
    {plan.processing && <div className="agent-plan-processing"><span>{t('Original values: {type}',{type:plan.processing.dataType})}</span><span>{t(plan.processing.quality ? qualityLabels[plan.processing.quality.policy] : 'No quality screening')}{plan.processing.quality?.excludeSnow && ` · ${t('Exclude snow')}`}</span></div>}
    {plan.start && <p className="agent-plan-dates">{plan.kind==='download' && `${t('Requested time interval')}: `}{date(plan.start)} – {date(plan.end)}</p>}
    <div className="agent-review-details"><InfoDetails summary={t('Area and files')} closeLabel={t('Close')}><div className="agent-plan-details">
      {taskContext&&<TaskSummary context={taskContext} scope={plan.kind==='download'?'This step downloads the listed files. Later processing is reviewed separately.':'Saves project metadata; no files are downloaded.'}/>}
      <p>{!plan.boundsCrs || plan.boundsCrs === 'EPSG:4326' ? 'WGS 84' : plan.boundsCrs} · {plan.bounds.map(value => number(value,{maximumFractionDigits:4})).join(', ')}</p>
       <div className="agent-plan-files-list" tabIndex={0} role="region" aria-label={t('Task file list')}>{plan.files.map(file => <div key={file.referenceId || `${file.itemId}:${file.assetKey}`}><p title={file.itemId}>{file.originalAssetKey ? `${file.originalAssetKey} · ` : `${file.assetKey.toUpperCase()} · `}{file.itemId}{file.bytes !== null && <small>{formatBytes(file.bytes,locale)}</small>}</p>{file.coveragePlanId && <><p>{file.crs} · {number(file.width)} × {number(file.height)} px</p><p>{t('Aligned coverage area')}: {file.alignedBounds.map(value=>number(value,{maximumFractionDigits:6})).join(', ')}</p></>}</div>)}</div>
      {plan.processing && <><p>{t('Uncompressed samples')}: {formatBytes(plan.processing.rawBytes,locale)}</p>{plan.processing.requiredDiskBytes !== null && <p>{t('Required workspace space')}: {formatBytes(plan.processing.requiredDiskBytes,locale)}</p>}{plan.processing.quality?.coupled && <p>{t('All channels use the same selected observation.')}</p>}</>}
       {coverage && plan.kind==='download' && <p>{t('Encoded size available after download; maximum 512 MiB per file.')}</p>}
       {coverage && plan.notes.map(note=><p key={note}>{t(note)}</p>)}
         <p>{t(plan.kind === 'project' ? 'Saves project metadata; no files are downloaded.' : plan.archive || plan.authorization ? 'Output: managed workspace files' : 'Output: managed workspace files · GeoTIFF')}{plan.authorization && ` · ${plan.format}`}</p>
      {plan.polygon&&<p>{t('Includes the saved polygon boundary.')}</p>}
      {plan.areaCoverage&&<><p>{plan.areaCoverage.status==='unknown'?t('Search again to prepare a verified complete area review.'):t('Catalog footprint coverage: {percent}',{percent:number(plan.areaCoverage.coveredFraction,{style:'percent',maximumFractionDigits:2})})}</p><p>{t('Footprint coverage does not establish valid pixels or cloud-free coverage.')}</p></>}
      {!coverage&&plan.notes.map(note=><p key={note}>{t(note)}</p>)}
    </div></InfoDetails><span>{t(plan.kind==='project'?'Saves project metadata; no files are downloaded.':coverage?'Service-generated subset · native grid · no resampling':plan.kind==='download'?'Downloads retain complete source tiles; the search area does not crop originals.':'Output: managed workspace files')}</span></div>
    {plan.areaCoverage&&<div className="agent-review-coverage" role="status"><span>{t(plan.areaCoverage.target==='polygon'?'Administrative / selected polygon':'Selected rectangle')} · {t(plan.areaCoverage.status==='unknown'?'Area coverage not verified':areaBlocked?'Area coverage incomplete':'Entire area covered')}</span>{areaBlocked&&<span>{t('Complete the coverage before downloading')}</span>}</div>}
    {plan.status==='pending' && reviewIssue && <p className="agent-plan-note" role="status">{t(reviewIssue)}</p>}
    <div className="agent-plan-actions">{['download','project'].includes(plan.kind)&&!coverage&&!plan.archive&&!plan.files[0].originalAssetKey&&plan.status!=='superseded'&&<Button size="icon" variant="secondary" icon={MapIcon} aria-label={t('Map preview')} tooltip={t('Map preview')} disabled={disabled||!onPreview} onClick={()=>onPreview(plan)}/>}
    {['pending','expired'].includes(plan.status)&&<>{!plan.archive&&<Button size="icon" variant="secondary" icon={Settings2} tooltip={t('Edit review parameters')} aria-label={t('Edit review parameters')} disabled={disabled} onClick={()=>onEdit(plan)}/>} {plan.status === 'pending' && <Button primary icon={confirming ? undefined : Check} disabled={disabled || confirming || Boolean(authorizationRequired) || Boolean(reviewIssue) || Boolean(areaBlocked)} onClick={() => onConfirm(plan)}>{confirming && <Spinner size={14}/>} {t(confirming ? 'Submitting…' : confirmation)}</Button>}</>}</div>
    {!suppressOutputs&&plan.status==='submitted'&&(plan.kind==='project'||plan.kind==='download')&&<DeliveryReceipt compact project={plan.project} jobs={plan.jobs} jobIds={plan.jobs.map(job=>job.id)} submitted={plan.kind==='download'} onOpenProject={onOpenProject}/>}
    {plan.status === 'expired' && <p className="agent-plan-note">{t('Edit the parameters to create a fresh review.')}</p>}
    {plan.jobs.length > 0 && <><div className="agent-plan-job-summary" role="status"><strong>{t('Task progress')}</strong><span>{t('{completed}/{total} tasks complete',{completed:number(plan.jobs.filter(job=>job.status==='succeeded'&&job.settled).length),total:number(plan.jobs.length)})}</span></div><div className="agent-plan-jobs" tabIndex={0} role="region" aria-label={t('Task progress list')}>{plan.jobs.map(job => {
      const ready = job.status === 'succeeded' && job.settled;
      const status=ready?'Output ready':({queued:'Queued task',running:'Running task',succeeded:'Finalizing output',failed:'Failed task',cancelled:'Cancelled task',interrupted:'Interrupted task'})[job.status];
      const measurable=Number.isFinite(job.totalBytes) && job.totalBytes>0;
      return <div key={job.id}><div className="agent-plan-job-heading"><Button size="icon" variant="secondary" icon={ListTodo} title={job.title} tooltip={t('Open task')} aria-label={t('Open task')} onClick={() => onOpenTasks(job.id)}/>
        <div className="agent-plan-job-label"><strong title={job.title}>{job.title}</strong><span>{t(status)}</span></div>
        {ready && !suppressOutputs && <OutputActions archive={plan.archive===true} jobId={job.id} projectId={plan.project?.committed || plan.project?.mode === 'existing' ? plan.project.id : undefined} packageKind={plan.kind === 'rgb' ? 'raster_rgb' : plan.kind === 'clip' ? 'raster_clip' : undefined}/>}
        </div>
        {['queued','running'].includes(job.status) && <Progress value={measurable?job.bytesDownloaded:null} max={measurable?job.totalBytes:100} aria-label={t(plan.kind==='download'?'Download progress':'Processing progress')}/>}
        {plan.kind === 'download' || ready ? <small>{formatBytes(job.bytesDownloaded,locale)}{job.totalBytes ? ` / ${formatBytes(job.totalBytes,locale)}` : ''}</small>
          : ['queued','running'].includes(job.status) && job.totalBytes ? <small>{number(Math.min(100,job.bytesDownloaded / job.totalBytes * 100),{maximumFractionDigits:0})}%</small> : null}</div>;
    })}</div></>}
  </section>;
}
function VectorPlanCard({plan,disabled,confirming,onConfirm,onEdit,taskContext,reviewIssue,suppressOutputs=false}) {
  const {t,number,locale}=useI18n(), review=plan.vectorReview, result=plan.vector;
  return <section className="agent-plan" aria-label={t('Vector extraction')} data-plan-id={plan.planId} tabIndex={-1}>
    <div className="agent-plan-heading"><span><Layers size={16}/><strong>{t('Vector extraction')}</strong></span><Badge tone={plan.status==='pending'?'blue':'neutral'}>{t(plan.status==='pending'?'Awaiting confirmation':plan.status==='submitted'?'File saved':plan.status==='expired'?'Expired':'Replaced by revised review')}</Badge></div>
    <p className="agent-plan-source">{plan.source} · {review.protocol}</p>
    <div className="agent-plan-facts"><span title={review.name}>{review.name}</span><strong>{result ? t('{count} features',{count:number(result.featureCount)}) : t('Count available after extraction')}</strong></div>
    <InfoDetails summary={t('Area and collection')} closeLabel={t('Close')}><div className="agent-plan-details">{taskContext&&<TaskSummary context={taskContext} scope="This step saves the selected vector data. Later analysis is reviewed separately."/>}<p>WGS 84 · {plan.bounds.map(value=>number(value,{maximumFractionDigits:4})).join(', ')}</p><p>{review.collectionTitle} · {review.collectionId}</p>{review.responseFormat && <p>{t('Original response format')}: {review.responseFormat}</p>}<p>{t('Output: managed vector file and original service provenance')}</p>{plan.polygon&&<p>{t('Includes the saved polygon boundary.')}</p>}{plan.notes.map(note=><p key={note}>{t(note)}</p>)}</div></InfoDetails>
    {plan.status==='pending' && reviewIssue && <p className="agent-plan-note" role="status">{t(reviewIssue)}</p>}
    {['pending','expired'].includes(plan.status) && <div className="agent-plan-actions"><Button size="icon" variant="secondary" icon={Settings2} tooltip={t('Edit review parameters')} aria-label={t('Edit review parameters')} disabled={disabled} onClick={()=>onEdit(plan)}/>{plan.status==='pending' && <Button primary icon={confirming?undefined:Check} disabled={disabled || Boolean(confirming) || Boolean(reviewIssue)} onClick={()=>onConfirm(plan)}>{confirming && <Spinner size={14}/>} {t(confirming?'Extracting…':'Confirm extraction')}</Button>}</div>}
    {result && !suppressOutputs && <div className="agent-plan-jobs"><div className="agent-plan-job-heading"><Button asChild size="icon" variant="secondary" tooltip={t('Open in workspace')}><a href={`#Workspace?vector=${result.id}`} aria-label={t('Open in workspace')} title={result.name}><Layers size={16} aria-hidden={true}/></a></Button><span>{t('Verified vector file')} · {formatBytes(result.bytes,locale)}</span></div></div>}
    {plan.status==='expired' && <p className="agent-plan-note">{t('Edit the parameters to create a fresh review.')}</p>}
  </section>;
}
function SourceAccounts({ summary }) {
  const { t, date } = useI18n();
  const accounts = summary?.kind === 'sources' ? summary.accounts : ['nasa-earthdata','copernicus'].map(provider => ({provider}));
  const labels = {'not-connected':'Not connected',saved:'Authorization saved',connected:'Connected',expired:'Authorization expired','storage-error':'Storage unavailable',unsupported:'Unavailable on this device',unavailable:'Status unavailable'};
  return <div className="agent-account-actions">
    {summary?.kind === 'sources' && <small>{t('Account status at {time}',{time:date(summary.checkedAt,{hour:'2-digit',minute:'2-digit',timeZoneName:'short'})})}</small>}
    {accounts.map(account => {
      const nasa = account.provider === 'nasa-earthdata';
      const status = ['saved','connected'].includes(account.status) && account.expiresAt && Date.parse(account.expiresAt) <= Date.now() ? 'expired' : account.status;
      const action = t(nasa ? 'Manage NASA Earthdata authorization' : 'Manage Copernicus authorization');
      return <div key={account.provider}><span>{nasa ? 'NASA Earthdata' : 'Copernicus'}{status && <Badge tone={status === 'connected' ? 'accent' : status === 'expired' ? 'warning' : 'neutral'} title={t('Authorization state only; original product access is not verified.')}>{t(labels[status])}</Badge>}</span>
        <Button asChild size="icon" variant="quiet" tooltip={action}><a href={`#Settings?account=${account.provider}`} aria-label={action}><KeyRound size={15} aria-hidden="true"/></a></Button></div>;
    })}
  </div>;
}
function ToolEntry({ entry, plans = [], showPlans = true, active=false, onInspect, disabled, confirming, onConfirm, onEdit, onPreview, onOpenProject, onOpenTasks, onDecisionAnswer, answering, decisionPending, decisionEntries, suppressOutputs=false }) {
  const { t, number } = useI18n();
  const tool = useRef(null), openVersion = useRef(0);
  useEffect(() => () => { openVersion.current++; }, []);
  if(entry.decision)return <AgentDecisionCard decision={entry.decision} disabled={disabled} submitting={answering===entry.decision.id} onAnswer={onDecisionAnswer}/>;
  const reveal = open => {
    const version = ++openVersion.current;
    if (!open) return;
    requestAnimationFrame(async () => {
      const element = tool.current;
      if (!element?.isConnected) return;
      // Wait for the shared disclosure's height animation, excluding spinners.
      await Promise.all((element.getAnimations?.({subtree:true}) || [])
        .filter(animation => Number.isFinite(animation.effect?.getComputedTiming().endTime))
        .map(animation => animation.finished.catch(() => {})));
      if (version !== openVersion.current || !element.isConnected) return;
      const content = element.querySelector('.agent-tool-results'), viewport = element.closest('.agent-conversation');
      if (!content || !viewport) return;
      const body = content.getBoundingClientRect(), frame = viewport.getBoundingClientRect();
      // Small opened results stay above the composer. Long results keep their
      // heading in place so opening them does not skip the beginning.
      if (body.height > viewport.clientHeight - 24) return;
      const delta = body.bottom - (frame.top + viewport.clientTop + viewport.clientHeight - 12);
      if (delta > 0) viewport.scrollBy?.({top:Math.ceil(delta),behavior:'smooth'});
    });
  };
  const running=active&&entry.status==='running';
  const {outcome,observations,record}=toolObservations(entry,plans,{t,number});
  const detail=entry.status==='completed'?outcome:
    t(running?'Performing native task':TOOL_FAILURES[entry.failureCode]||'Tool stopped or could not finish.');
  const reviewPlans = showPlans ? entry.references.filter(reference => reference.kind === 'plan').map(reference => plans.find(plan => plan.planId === reference.id)).filter(Boolean) : [];
  const hasDetails=Boolean(record||observations.length||entry.references.some(ref=>ref.kind!=='plan')||entry.name==='geod_sources_list'||entry.status!=='completed');
  const body=<div className="agent-tool-results">
      {entry.status==='completed'&&observations.length>0&&<dl className="agent-tool-facts">{observations.map(({label,value},index)=><div key={`${label}:${index}`}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>}
      {outcome&&observations.length===0&&<p>{outcome}</p>}
      {record&&<div className="agent-tool-receipt" aria-label={t('Tool details')}>
        <div className="agent-tool-receipt-heading"><code>{entry.name}</code><small>{t('Recorded at {time}',{time:record.startedAt.slice(11,19)+' UTC'})}</small></div>
        <WorkDetailsSection label="Input summary" content={JSON.stringify(record.input,null,2)}/>
        {record.result!==undefined&&<WorkDetailsSection label="Result excerpt" content={JSON.stringify(record.result,null,2)}/>}
        {record.limited&&<small>{t('Showing a limited result excerpt; it is not the complete dataset.')}</small>}
      </div>}
      {entry.references.filter(reference => reference.kind !== 'plan').map(reference => <div key={`${reference.kind}:${reference.id}`}>{reference.kind === 'vector' ? <Button asChild size="row" variant="quiet" ><a href={`#Workspace?vector=${reference.id}`} title={reference.label}><Layers size={16} aria-hidden={true}/>{reference.label === 'Vector file' ? t('Vector file') : reference.label || t('Open in workspace')}</a></Button> : <Button size="row" variant="quiet" icon={reference.kind === 'project' ? FolderOpen : ListTodo}
      onClick={() => reference.kind === 'project' ? onOpenProject(reference.id) : onOpenTasks(reference.id)} title={reference.label}>{reference.label || t(reference.kind === 'project' ? 'Open project' : 'Open task')}</Button>}
      {!suppressOutputs && reference.kind === 'job' && (entry.summary?.kind === 'file' || entry.summary?.kind === 'job' && entry.summary.status === 'succeeded' && entry.summary.settled) && <OutputActions jobId={reference.id}/>}</div>)}
      {entry.name === 'geod_sources_list' && entry.status === 'completed' && <SourceAccounts summary={entry.summary}/>}
      </div>;
  return <div ref={tool}><WorkRecord className="agent-tool" title={t(TOOLS[entry.name] || 'Read workspace data')} status={entry.status} active={active} onInspect={onInspect} hasDetails={hasDetails} detail={detail} duration={record?.durationMs===undefined?undefined:number(record.durationMs/1000,{maximumFractionDigits:1})+'s'} onOpenChange={reveal}>{body}</WorkRecord>
    {reviewPlans.map(plan => <PlanCard key={plan.planId} plan={plan} suppressOutputs={suppressOutputs} taskContext={entry.taskContext} reviewIssue={decisionReviewIssue(decisionEntries??[entry],plan.planId)} disabled={disabled || decisionPending} confirming={confirming === plan.planId} onConfirm={onConfirm} onEdit={onEdit} onPreview={onPreview} onOpenProject={onOpenProject} onOpenTasks={onOpenTasks}/>)}</div>;
}
function AgentMessage({ entry, streaming, progress = false, active=false, footer }) {
  return <ChatMessage entry={entry} streaming={streaming} progress={progress} active={active} footer={footer}>
    {entry.images?.length > 0 && <div className="agent-images">{entry.images.map(image => <AgentImage key={image.id} image={image}/>)}</div>}
    {entry.documents?.length > 0 && <div className="agent-documents">{entry.documents.map(document => <AgentDocument key={document.id} document={document}/>)}</div>}
  </ChatMessage>;
}
function AgentTyping() {const {t}=useI18n();return <MessageTyping label={t('Waiting for response…')}/>;}
function ComposerPermission({ mode, disabled, onChange }) {
  const { t } = useI18n(), [open, setOpen] = useState(false);
  const full = mode === 'full-access';
  return <Popover open={open} onOpenChange={setOpen} align="start" side="top" className="composer-permission-menu"
    trigger={<Button data-prompt-control="permission" variant="quiet" className={`composer-permission-trigger${full ? ' full-access' : ''}`}
      aria-label={t('Execution mode')} title={t(full ? 'Full access' : 'Confirm each plan')} disabled={disabled} aria-expanded={open}>
      {full ? <ShieldCheck size={16} aria-hidden="true"/> : <ShieldAlert size={16} aria-hidden="true"/>}<span>{t(full ? 'Full access' : 'Confirm each plan')}</span><ChevronDown size={14} aria-hidden="true"/>
    </Button>}>
    <strong>{t('Workspace permission')}</strong>
    {[['confirm-each','Confirm each plan','Confirm plans in chat before running.',ShieldCheck],['full-access','Full access','Run validated plans for your requested task automatically.',ShieldAlert]].map(([value,label,description,Icon]) =>
      <Button key={value} variant="quiet" size="row" className="composer-permission-option" aria-pressed={mode === value} onClick={() => { setOpen(false); onChange(value); }}>
        <Icon size={17} aria-hidden="true"/><span><b>{t(label)}</b><small>{t(description)}</small></span>
      </Button>)}
  </Popover>;
}
function ComposerContext({ state, usage, model, disabled, onOrganize, children }) {
  const { t } = useI18n();
  const used = state?.usedTokens, limit = state?.windowTokens;
  const known = Number.isSafeInteger(used), hasLimit = Number.isSafeInteger(limit) && limit > 0;
  const percent = known && hasLimit ? Math.min(100, used / limit * 100) : 0, length = 2 * Math.PI * 8;
  const tokens = number => number.toLocaleString();
  return <div className="context-usage-root" data-prompt-control="context"><Popover align="end" side="top" className="context-usage-panel"
    trigger={<Button variant="quiet" size="icon" className="context-usage-trigger" aria-label={t('Context usage')} tooltip={t('Context usage')}>
      <svg className="context-usage-ring" viewBox="0 0 24 24" aria-hidden="true"><circle className="context-usage-ring-track" cx="12" cy="12" r="8" fill="none" strokeWidth="2.5"/>
        <circle className={`context-usage-ring-progress${percent >= 90 ? ' is-near-limit' : ''}`} cx="12" cy="12" r="8" fill="none" strokeWidth="2.5" strokeLinecap="round" strokeDasharray={length} strokeDashoffset={length * (1 - percent / 100)}/></svg>
    </Button>}>
    <div className="context-usage-heading"><strong>{t('Context usage')}</strong><span>{t('This conversation')}</span></div>
    <div className="context-usage-number"><strong>{known && hasLimit ? `${percent.toFixed(1)}%` : '—'}</strong><span>{known ? `${tokens(used)} / ${hasLimit ? tokens(limit) : t('Unknown')} token` : t('Waiting for the first model request')}</span></div>
    <div className="context-usage-track" {...(known && hasLimit ? {role:'meter','aria-label':t('Context usage'),'aria-valuemin':0,'aria-valuemax':limit,'aria-valuenow':Math.min(used,limit)} : {})}><span style={{width:`${percent}%`}}/></div>
    <div className="context-usage-details"><div><span>{t('Model')}</span><strong>{model?.model || t('No model connected')}</strong></div>
      <div><span>{t('Context organized')}</span><strong>{state?.count ?? 0}</strong></div></div>
    <p className="context-usage-note">{t('Usage comes from the latest native model request. Unknown limits are not estimated.')}</p>
    {usage && <><div className="context-usage-heading"><strong>{t('Recorded model usage')}</strong><span>{t('{count} requests',{count:usage.requests})}</span></div>
      <div className="context-usage-details">{[['Cumulative input','input'],['Cached input (included)','cachedInput'],['Cumulative output','output'],['Reasoning output (included)','reasoningOutput'],['Cumulative total','total']].map(([label,key])=><div key={key}><span>{t(label)}</span><strong>{Number.isSafeInteger(usage.totals?.[key])?tokens(usage.totals[key]):t('Unknown')}</strong></div>)}</div>
      <p className="context-usage-note">{t('Recorded requests only. Cached input and reasoning are included in totals. Token counts are not a bill; missing provider details stay unknown.')}</p></>}
    <Button variant="quiet" size="row" icon={Shrink} disabled={disabled} onClick={onOrganize}>{t('Organize context')}</Button>
    {children}
  </Popover></div>;
}
function AgentImage({ image }) {
  const { t }=useI18n(); const [preview,setPreview]=useState(null),[failed,setFailed]=useState(false),[open,setOpen]=useState(false);
  useEffect(()=>{
    let current=true; setPreview(null);setFailed(false);setOpen(false);
    agentRequest('imagePreview',{id:image.id}).then(value=>{
      if(value.image.id!==image.id || value.image.bytes!==image.bytes) throw Error('Image reference changed.');
      if(current)setPreview(value.dataUrl);
    }).catch(()=>{if(current)setFailed(true);});
    return ()=>{current=false;};
  },[image.id,image.bytes]);
  return <><figure className="agent-image" title={image.name}>{preview?<Button variant="quiet" className="agent-image-open" aria-label={t('Preview image {name}',{name:image.name})} onClick={()=>setOpen(true)}><img src={preview} alt={image.name} width={image.width} height={image.height}/></Button>:<span className="agent-image-placeholder" role={failed?'img':undefined} aria-label={failed?t('Image unavailable'):undefined}>{failed?<ImageOff size={18}/>:<Spinner size={16}/>}</span>}<figcaption>{image.name}</figcaption></figure>
    {open && <Modal title={image.name} description={t('Image preview')} closeLabel={t('Close')} onClose={()=>setOpen(false)}><img className="agent-image-full" src={preview} alt={image.name} width={image.width} height={image.height}/></Modal>}</>;
}
function AgentDocument({ document }) {
  const {t}=useI18n(),[open,setOpen]=useState(false),[preview,setPreview]=useState(null),[failed,setFailed]=useState(false);
  useEffect(()=>{
    if(!open)return;
    let current=true;setPreview(null);setFailed(false);
    agentRequest('documentPreview',{id:document.id}).then(value=>{
      if(['id','name','bytes','characters','pages','mimeType'].some(key=>value.document[key]!==document[key]))throw Error('Document reference changed.');
      if(current)setPreview(value);
    }).catch(()=>{if(current)setFailed(true);});return()=>{current=false;};
  },[open,document.id,document.name,document.bytes,document.characters,document.pages,document.mimeType]);
  return <><Button variant="quiet" className="agent-document" icon={videoExtension(document)?Film:audioExtension(document)?Headphones:FileText} aria-label={t('Preview file {name}',{name:document.name})} title={document.name} onClick={()=>setOpen(true)}><span><strong>{document.name}</strong><small>{formatBytes(document.bytes)}</small></span></Button>
    {open && <Modal wide={document.mimeType==='application/pdf' || Boolean(officeExtension(document)||videoExtension(document))} title={document.name} description={t(document.mimeType==='application/pdf'?'PDF preview':officeExtension(document)?'Office content preview':audioExtension(document)?'Audio preview':videoExtension(document)?'Video preview':'Text file preview')} closeLabel={t('Close')} onClose={()=>setOpen(false)}>{failed?<p role="alert" className="agent-error">{t('Document unavailable')}</p>:preview!==null?document.mimeType==='application/pdf'?<Suspense fallback={<Spinner size={16}/>}><AgentPdfPreview preview={preview}/></Suspense>:audioExtension(document)?<Suspense fallback={<Spinner size={16}/>}><AgentAudioPreview preview={preview}/></Suspense>:videoExtension(document)?<Suspense fallback={<Spinner size={16}/>}><AgentVideoPreview preview={preview}/></Suspense>:<>{officeExtension(document) && <p className="agent-file-note">{t('Content preview shows text and stored cell values. Layout and images are not shown; formulas are not calculated.')}</p>}<pre className="agent-document-text">{preview.text}</pre></>:<p role="status" className="agent-loading"><Spinner size={16}/>{t('Reading file…')}</p>}</Modal>}</>;
}
const readAttachmentFile=file=>new Promise((resolve,reject)=>{
  const reader=new FileReader(); reader.onload=()=>resolve(String(reader.result).split(',')[1]);
  reader.onerror=()=>reject(Error('File could not be read.'));reader.readAsDataURL(file);
});
function AttachmentStorageDialog({ protectedAttachments, onClose }) {
  const { t }=useI18n(),[storage,setStorage]=useState(null),[busy,setBusy]=useState(false),[error,setError]=useState('');
  const current=useRef(true),pending=useRef(false);
  useEffect(()=>{
    current.current=true;
    agentRequest('attachmentStorage',{protectedImages:protectedAttachments.images,protectedDocuments:protectedAttachments.documents,cleanup:false}).then(value=>{if(current.current)setStorage(value);}).catch(error=>{if(current.current)setError(String(error.message||error));});
    return ()=>{current.current=false;};
  },[protectedAttachments]);
  const unused=storage?(storage.images.unusedCount+storage.documents.unusedCount):0;
  const clean=async()=>{
    if(pending.current || !unused)return;
    pending.current=true;setBusy(true);setError('');
    try{const value=await agentRequest('attachmentStorage',{protectedImages:protectedAttachments.images,protectedDocuments:protectedAttachments.documents,cleanup:true});if(current.current)setStorage(value);}
    catch(error){if(current.current)setError(String(error.message||error));}
    finally{pending.current=false;if(current.current)setBusy(false);}
  };
  const removed=storage?(storage.images.removedBytes+storage.documents.removedBytes):0;
  return <Modal title={t('Attachment storage')} description={t('Local copies used by Agent conversations.')} closeLabel={t('Close')} onClose={onClose} closeDisabled={busy}
    footer={<div className="agent-model-actions"><Button onClick={onClose} disabled={busy}>{t('Close')}</Button><Button primary icon={busy?undefined:Trash2} disabled={busy || !unused} onClick={clean}>{busy && <Spinner size={15}/>} {t(busy?'Cleaning attachments…':'Clean unused copies')}</Button></div>}>
    {storage?<><div className="agent-storage-sections">{[['images','Images',storage.images.imageCount],['documents','Documents',storage.documents.documentCount]].map(([key,label,count])=><section key={key}><h3>{t(label)}<Badge>{count}</Badge></h3><div className="agent-storage-usage"><strong>{formatBytes(storage[key].usedBytes)}</strong><span>{t('of {size}',{size:formatBytes(storage[key].limitBytes)})}</span></div><Progress value={Math.min(100,storage[key].usedBytes/storage[key].limitBytes*100)} aria-label={t('{name} storage usage',{name:t(label)})}/></section>)}</div>
      <dl className="agent-storage-details"><div><dt>{t('Unused copies')}</dt><dd>{t('{count} files · {size}',{count:unused,size:formatBytes(storage.images.unusedBytes+storage.documents.unusedBytes)})}</dd></div></dl>
      {removed>0 && <p className="agent-storage-result" role="status"><Check size={16}/>{t('Freed {size}',{size:formatBytes(removed)})}</p>}</>:!error && <p role="status" className="agent-loading"><Spinner size={16}/>{t('Checking attachment storage…')}</p>}
    <p className="agent-model-help">{t('Conversation attachments and your current draft are kept. Cleanup removes unused local copies; source files and workspace data stay in place.')}</p>
    {error && <p role="alert" className="agent-error">{t(error)}</p>}
  </Modal>;
}
export function AgentPanel({ onClose, onOpenProject, onOpenTasks, onOpenResult, onPreviewPlan, closeRef, context = null, scene = null, datasetId, draftStore, sessionViewStore, preferredSessionId, onSessionTargetChange, onContinueInConversation, variant = 'sidebar', onChooseSource, onOpenSourceEntry }) {
  const { t } = useI18n();
  const [snapshot, setData] = useState(null), [actionError, setError] = useState(''), [snapshotError, setSnapshotError] = useState(''), [input, setInput] = useState(()=>draftStore?.current.get(scene?.kind)?.input??'');
  const sessionViews=useRef(new Map());
  if(sessionViewStore)sessionViews.current=sessionViewStore.current;
  if(snapshot?.selected)sessionViews.current.set(snapshot.selected.id,{selected:snapshot.selected,plans:snapshot.plans,execution:snapshot.execution});
  const error=actionError||snapshotError;
  const [modelOpen, setModelOpen] = useState(false), [historyPreference, setHistoryOpen] = useState(null), [fresh, setFresh] = useState(!scene && variant === 'home');
  const [sourcesOpen,setSourcesOpen]=useState(false),[moreOpen,setMoreOpen]=useState(false);
  const sourceTrigger=useRef(null),moreAction=useRef(null);
  const chooseMore=action=>{moreAction.current=action;setMoreOpen(false);};
  const finishMore=()=>{const action=moreAction.current;moreAction.current=null;if(action)queueMicrotask(action);};
  const [sending, setSending] = useState(false), [closing, setClosing] = useState(false), [switching, setSwitching] = useState(false),[organizing,setOrganizing]=useState(false);
  const [confirming, setConfirming] = useState(null), confirmationRef = useRef(false);
  const [answering,setAnswering]=useState(null),answeringRef=useRef(false);
  const [revisionDraft,setRevisionDraft]=useState(null),[revisionBusy,setRevisionBusy]=useState(false),revisionRef=useRef(false);
  const panelElement=useRef(null),goalToggle=useRef(null),historyToggle=useRef(null),resultsToggle=useRef(null);
  const [panelWidth,setPanelWidth]=useState(0),[goalPreference,setGoalPreference]=useState(null),[resultsPreference,setResultsPreference]=useState(null),[workReveal,setWorkReveal]=useState(null);
  const previewReference=useRef(null);
  const [images,setImages]=useState(()=>draftStore?.current.get(scene?.kind)?.images??[]),[attaching,setAttaching]=useState(false),fileInput=useRef(null),attachRef=useRef(false);
  const [documents,setDocuments]=useState(()=>draftStore?.current.get(scene?.kind)?.documents??[]),[storageAttachments,setStorageAttachments]=useState(null);
  const mounted = useRef(true), epoch = useRef(0), scrolling = useRef(null), nearBottom = useRef(true), sendingRef = useRef(false);
  const surface=scene?.kind??'legacy',drafts=useRef(new Map()),previousSurface=useRef(surface),draftValues=useRef(null),restoringTarget=useRef(null);
  if(draftStore)drafts.current=draftStore.current;
  if(previousSurface.current===surface)draftValues.current={input,images,documents};
  const target=scene ? preferredSessionId===undefined ? snapshot?.sessions.find(value=>sceneKind(value)===surface&&value.compatible!==false)?.id??null : preferredSessionId : null;
  // Keep the last native transcript for this entrance visible while another
  // conversation owns the single model runtime. Cached views are read only;
  // actions wait until the native selected conversation has been restored.
  const cachedView=snapshot&&scene&&!fresh&&target&&snapshot.selected?.id!==target?sessionViews.current.get(target):null;
  const data=cachedView?{...snapshot,...cachedView}:snapshot;
  const awaitingTarget=Boolean(scene&&!fresh&&target&&snapshot?.selected?.id!==target);
  const session = scene ? !fresh&&data?.selected?.id===target?data.selected:null : fresh && !data?.busy ? null : data?.selected;
  useLayoutEffect(()=>{
    if(previousSurface.current===surface)return;
    drafts.current.set(previousSurface.current,draftValues.current);
    const next=drafts.current.get(surface);previousSurface.current=surface;restoringTarget.current=null;
    setInput(next?.input??'');setImages(next?.images??[]);setDocuments(next?.documents??[]);setFresh(false);setError('');
    setHistoryOpen(null);setRevisionDraft(null);setWorkReveal(null);
  },[surface]);
  useEffect(()=>()=>{drafts.current.set(previousSurface.current,draftValues.current);},[]);
  useEffect(()=>{
    if(!scene || !snapshot?.runtimeAvailable || fresh || !target || snapshot.selected?.id===target || snapshot.busy || sending)return;
    const key=surface+target;if(restoringTarget.current===key)return;
    restoringTarget.current=key;const ticket=++epoch.current;setSwitching(true);
    agentRequest('select',{id:target}).then(value=>{if(mounted.current&&ticket===epoch.current)setData(value);})
      .catch(error=>{if(mounted.current&&ticket===epoch.current)setError(String(error.message||error));})
      .finally(()=>{if(mounted.current&&ticket===epoch.current)setSwitching(false);});
  },[surface,target,snapshot?.selected?.id,snapshot?.busy,snapshot?.runtimeAvailable,fresh,sending]);
  const responseBusy=Boolean(sending||organizing||data?.busy&&session&&session.id===(data.activity?.sessionId??data.selected?.id));
  const elsewhereBusy=Boolean(scene&&data?.busy&&!responseBusy);
  const visibleSessions=scene?.kind==='workspace'?data?.sessions.filter(value=>sceneKind(value)==='workspace'||value.id===target):data?.sessions;
  useEffect(()=>{
    const panel=panelElement.current;if(!panel)return;
    const measure=()=>setPanelWidth(panel.getBoundingClientRect().width);
    measure();if(typeof ResizeObserver==='undefined')return;
    const observer=new ResizeObserver(measure);observer.observe(panel);return ()=>observer.disconnect();
  },[]);
  const historyOpen=historyPreference??(variant==='home'&&panelWidth>=860&&Boolean(visibleSessions?.length));
  const wideHistoryLayout=panelWidth>=860;
  const wideGoalLayout=panelWidth>=(historyOpen?1050:760);
  useEffect(()=>{setGoalPreference(null);setResultsPreference(null);},[session?.id,session?.goal?.id]);
  useEffect(()=>setWorkReveal(null),[session?.id]);
  const goalOpen=Boolean(session?.goal&&(goalPreference??(wideGoalLayout&&session.goal.status!=='complete')));
  const hideGoal=(restoreFocus=true)=>{setGoalPreference(false);if(restoreFocus)goalToggle.current?.focus();};
  useEffect(()=>{
    const opened=previewReference.current;
    if(!opened)return;
    const current=opened.kind?conversationDeliveries(session,data?.plans).some(item=>item.kind===opened.kind&&item.id===opened.id):data?.plans?.find(plan=>plan.planId===opened.planId);
    if(session?.id!==opened.sessionId || !current || !opened.kind&&(current.status==='superseded' || current.planHash!==opened.planHash)) {
      previewReference.current=null;onPreviewPlan?.(null);
    }
  },[session?.id,data?.plans,onPreviewPlan]);
  useEffect(()=>()=>onPreviewPlan?.(null),[onPreviewPlan]);
  const previewTask=onPreviewPlan ? plan=>{
    if(!session)return;
    previewReference.current={sessionId:session.id,planId:plan.planId,planHash:plan.planHash};
    onPreviewPlan({sessionId:session.id,plan});
  } : undefined;
  const previewResult=onOpenResult?view=>{
    if(!session)return;
    previewReference.current={sessionId:session.id,kind:view.kind,id:view.id};
    if(view.kind==='archive'){setResultsPreference(false);hideGoal(false);}
    onOpenResult(view);
  }:undefined;
  const executionMode=(fresh?data?.execution?.defaultMode:data?.execution?.mode)??'confirm-each';
  const workflowStatus=session?.workflow?.status;
  const viewed=useRef(new Set());
  useEffect(()=>{
    const view=session?.workspaceView;
    if(cachedView||!view||view.acknowledged||viewed.current.has(view.requestId)||!onOpenResult)return;
    viewed.current.add(view.requestId);previewResult(view);const ticket=epoch.current;
    agentRequest('acknowledgeView',{sessionId:session.id,requestId:view.requestId}).then(value=>{if(mounted.current&&ticket===epoch.current)setData(value);}).catch(error=>{if(mounted.current&&ticket===epoch.current)setError(String(error.message||error));});
  },[session?.id,session?.workspaceView?.requestId,session?.workspaceView?.acknowledged,cachedView,onOpenResult]);
  const compatibilityReason=data?.sessions.find(value=>value.id===session?.id)?.compatibilityReason;
  const busy = Boolean(data?.busy || sending || organizing || confirming || switching || revisionBusy || awaitingTarget), compatible = !session || data?.sessions.find(value => value.id === session.id)?.compatible !== false;
  const deliveries=conversationDeliveries(session,data?.plans);
  const resultsOpen=Boolean(deliveries.length&&(resultsPreference??wideGoalLayout));
  const officeCompatible = data?.model?.protocol==='openai-responses' || ![...documents,...(session?.entries??[]).flatMap(entry=>entry.documents??[])].some(officeExtension);
  const audioCompatible = data?.model?.protocol==='google-generative-ai' || ![...documents,...(session?.entries??[]).flatMap(entry=>entry.documents??[])].some(audioExtension);
  const videoCompatible = data?.model?.protocol==='google-generative-ai' || ![...documents,...(session?.entries??[]).flatMap(entry=>entry.documents??[])].some(videoExtension);
  useEffect(() => {
    if (!desktopAvailable()) {
      setData({runtimeAvailable:false,configured:false,busy:false,sessions:[],selected:null});
      return;
    }
    mounted.current = true; let timer, wakeTimer, unlisten, stopped = false, loading = false, dirty = false, eventDriven = false;
    const poll = async () => {
      if (stopped) return;
      if (loading) { dirty = true; return; }
      clearTimeout(timer); clearTimeout(wakeTimer); wakeTimer = undefined;
      loading = true; dirty = false;
      const ticket = epoch.current;
      let delay = 2500;
      try {
        const value = await agentRequest('snapshot');
        if (mounted.current && ticket === epoch.current) {setData(value);setSnapshotError('');}
        delay = value.busy && !eventDriven ? 125 : 1500;
      } catch (error) {
        if (mounted.current && ticket === epoch.current) setSnapshotError(String(error.message || error));
      } finally {
        loading = false;
        if (!stopped && desktopAvailable()) timer = setTimeout(poll, dirty ? 16 : delay);
      }
    };
    subscribeAgentUpdates(() => {
      if (stopped) return;
      if (loading) { dirty = true; return; }
      clearTimeout(timer);
      wakeTimer ??= setTimeout(poll, 16);
    }).then(dispose => {
      if (stopped) { dispose(); return; }
      unlisten = dispose; eventDriven = Boolean(window.__TAURI__?.event?.listen);
    }).catch(() => { /* The bounded snapshot poll also supports older runtimes. */ });
    poll();
    return () => { stopped = true; mounted.current = false; epoch.current++; clearTimeout(timer); clearTimeout(wakeTimer); unlisten?.(); };
  }, []);
  const apply = value => { epoch.current++; if (mounted.current) { setData(value); setError('');setSnapshotError(''); } };
  const modelSaved = value => {
    const before=data?.model, after=value.model;
    apply(value);
    if (before?.id !== after?.id || before?.protocol !== after?.protocol || before?.baseUrl !== after?.baseUrl || before?.model !== after?.model || before?.provider !== after?.provider) { setFresh(true);onSessionTargetChange?.(null);setImages([]);setDocuments([]); nearBottom.current=true; }
  };
  const switchModel = async id => {
    if (busy || attaching || id === data?.registry?.selectedId) return;
    epoch.current++; setSwitching(true); setError('');
    try { modelSaved(await agentRequest('saveModel',{request:{action:'select',id}})); }
    catch (error) { if (mounted.current) setError(String(error.message || error)); }
    finally { if (mounted.current) setSwitching(false); }
  };
  const send = async event => {
    event?.preventDefault(); if (sendingRef.current || busy || attaching || !officeCompatible || !audioCompatible || !videoCompatible || !input.trim() && !images.length && !documents.length || !data?.configured || !compatible) return;
    epoch.current++; sendingRef.current = true; setSending(true); setError(''); nearBottom.current = true;
    try { const value = await agentRequest('send', { sessionId:session?.id ?? null, text:input.trim(), context, ...(scene?{scene}:{}),...(images.length?{images:images.map(image=>image.id)}:{}),...(documents.length?{documents:documents.map(document=>document.id)}:{}) }); if (mounted.current) { apply(value);onSessionTargetChange?.(value.selected?.id??null);setInput(''); setImages([]);setDocuments([]); setFresh(false); } }
    catch (error) { if (mounted.current) setError(String(error.message || error)); }
    finally { sendingRef.current = false; if (mounted.current) setSending(false); }
  };
  const attach=async event=>{
    const files=Array.from(event.target.files??[]);event.target.value='';
    if(!files.length || busy || attaching || attachRef.current || !data?.configured || !compatible)return;
    if(files.length+images.length+documents.length>3){setError('Choose up to 3 attachments per message.');return;}
    const isImage=file=>['image/png','image/jpeg','image/webp'].includes(file.type) || /\.(png|jpe?g|webp)$/i.test(file.name);
    if(files.some(file=>!file.size || (/\.(mp4|webm)$/i.test(file.name)?file.size>8*1024*1024:isImage(file) || /\.(pdf|docx|xlsx|pptx|wav|mp3|flac|ogg)$/i.test(file.name)?file.size>2*1024*1024:!(/\.(txt|md|csv|json|geojson)$/i.test(file.name)) || file.size>64*1024))){
      setError('Choose video up to 8 MiB, images, PDF, Office or audio up to 2 MiB, or UTF-8 text up to 64 KiB.');return;
    }
    attachRef.current=true;setAttaching(true);setError('');
    try{
      const addedImages=[],addedDocuments=[];
      for(const file of files)(isImage(file)?addedImages:addedDocuments).push(await agentRequest(isImage(file)?'attachImage':'attachDocument',{name:file.name,encoded:await readAttachmentFile(file)}));
      if(mounted.current){setImages(previous=>[...new Map([...previous,...addedImages].map(image=>[image.id,image])).values()]);setDocuments(previous=>[...new Map([...previous,...addedDocuments].map(document=>[document.id,document])).values()]);}
    }catch(error){if(mounted.current)setError(String(error.message||error));}
    finally{attachRef.current=false;if(mounted.current)setAttaching(false);}
  };
  const confirm = async plan => {
    if (busy || confirmationRef.current || !session || plan.status !== 'pending') return;
    confirmationRef.current = true; setConfirming(plan.planId); epoch.current++; setError('');
    try { apply(await agentRequest('approvePlan', { sessionId:session.id, planId:plan.planId, planHash:plan.planHash })); }
    catch (error) { if (mounted.current) setError(String(error.message || error)); }
    finally { confirmationRef.current = false; if (mounted.current) setConfirming(null); }
  };
  const answerDecision=async(decisionId,answers)=>{
    if(busy || answeringRef.current || !session || !compatible)return;
    answeringRef.current=true;setAnswering(decisionId);epoch.current++;setError('');nearBottom.current=true;
    try{apply(await agentRequest('send',{sessionId:session.id,text:'',context,...(scene?{scene}:{}),decisionAnswer:{decisionId,answers}}));}
    catch(error){if(mounted.current)setError(String(error.message||error));}
    finally{answeringRef.current=false;if(mounted.current)setAnswering(null);}
  };
  const editPlan=async plan=>{
    if(busy||revisionRef.current||!session||!compatible||!['pending','expired'].includes(plan.status))return;
    revisionRef.current=true;setRevisionBusy(true);epoch.current++;setError('');
    try{const draft=await agentRequest('revisionDraft',{sessionId:session.id,planId:plan.planId,planHash:plan.planHash});if(mounted.current)setRevisionDraft({...draft,sessionId:session.id});}
    catch(error){if(mounted.current)setError(String(error.message||error));}
    finally{revisionRef.current=false;if(mounted.current)setRevisionBusy(false);}
  };
  const revisePlan=async revision=>{
    if(revisionRef.current||!revisionDraft||data?.busy)throw Error('Wait for the Agent response to finish before editing.');
    revisionRef.current=true;setRevisionBusy(true);epoch.current++;
    try{apply(await agentRequest('revisePlan',{sessionId:revisionDraft.sessionId,planId:revisionDraft.planId,planHash:revisionDraft.planHash,revision}));nearBottom.current=true;}
    finally{revisionRef.current=false;if(mounted.current)setRevisionBusy(false);}
  };
  const stop = async () => {
    epoch.current++;
    try { apply(await agentRequest('interrupt')); } catch (error) { if (mounted.current) setError(String(error.message || error)); }
  };
  const changeExecution=async mode=>{
    if(busy||attaching||!compatible)return;
    epoch.current++;setSwitching(true);setError('');
    try{apply(await agentRequest('executionMode',{sessionId:session?.id??null,mode}));}
    catch(error){if(mounted.current)setError(String(error.message||error));}
    finally{if(mounted.current)setSwitching(false);}
  };
  const controlGoal=async action=>{
    if(!session?.goal||switching||!compatible||action!=='pause'&&busy)return;
    epoch.current++;setSwitching(true);setError('');
    try{apply(await agentRequest('goalControl',{sessionId:session.id,action}));}
    catch(error){if(mounted.current)setError(String(error.message||error));}
    finally{if(mounted.current)setSwitching(false);}
  };
  const organize=async()=>{
    if(busy || attaching || !session?.threadId || !compatible || !data?.configured)return;
    epoch.current++;setOrganizing(true);setError('');
    try{apply(await agentRequest('compact',{sessionId:session.id}));}
    catch(error){if(mounted.current)setError(String(error.message||error));}
    finally{if(mounted.current)setOrganizing(false);}
  };
  const close = async () => {
    setClosing(true); epoch.current++;
    try { onClose(); }
    catch (error) { if (mounted.current) { setError(String(error.message || error)); setClosing(false); } }
  };
  useEffect(() => { if (closeRef) closeRef.current = close; return () => { if (closeRef) closeRef.current = null; }; });
  const choose = async id => {
    if(busy||attaching)return;
    epoch.current++;
    setSwitching(true);
    try { apply(await agentRequest('select', { id }));onSessionTargetChange?.(id);setFresh(false); setImages([]);setDocuments([]); if(!wideHistoryLayout)setHistoryOpen(false); nearBottom.current = true; }
    catch (error) { setError(String(error.message || error)); }
    finally { if(mounted.current)setSwitching(false); }
  };
  const newConversation=async()=>{
    if(busy||attaching)return;
    epoch.current++;setSwitching(true);setError('');
    try{apply(await agentRequest('select',{id:null}));onSessionTargetChange?.(null);setFresh(true);setInput('');setImages([]);setDocuments([]);nearBottom.current=true;}
    catch(error){if(mounted.current)setError(String(error.message||error));}
    finally{if(mounted.current)setSwitching(false);}
  };
  const executionActivity=data?.activity===undefined?undefined:data.activity?.sessionId===session?.id?data.activity:null;
  const rows = transcriptRows(session?.entries,executionActivity,{status:session?.status,error:session?.error,workflow:session?.workflow,goal:session?.goal,plans:data?.plans});
  const liveEntry = session?.entries.at(-1);
  const liveTool=session?.entries.findLast(entry=>entry.type==='tool'&&entry.status==='running');
  const goalActivity=busy&&liveTool?t(TOOLS[liveTool.name]??'Performing native task'):undefined;
  const showGoalPlan=planId=>{
    if(!wideGoalLayout){hideGoal(false);setResultsPreference(false);}
    const reveal=()=>{
      const viewport=scrolling.current;
      const card=viewport&&[...viewport.querySelectorAll('[data-plan-id]')].find(element=>element.dataset.planId===planId);
      if(!card)return false;
      nearBottom.current=false;
      const top=card.getBoundingClientRect().top-viewport.getBoundingClientRect().top+viewport.scrollTop-12;
      viewport.scrollTo({top,behavior:window.matchMedia?.('(prefers-reduced-motion: reduce)').matches?'instant':'smooth'});
      card.focus({preventScroll:true});return true;
    };
    if(!reveal()){
      const work=rows.find(row=>row.type==='work'&&row.entries.some(entry=>entry.type==='reviews'&&entry.references.some(ref=>ref.id===planId)));
      if(work){setWorkReveal(value=>({id:work.id,planId,request:(value?.request??0)+1}));requestAnimationFrame(()=>requestAnimationFrame(reveal));}
    }
  };
  const attachmentAction = action => {
    if (action === 'audio-settings') { setModelOpen(true); return; }
    const input = fileInput.current;
    if (!input) return;
    input.accept = {image:'image/png,image/jpeg,image/webp', document:'.pdf,.docx,.xlsx,.pptx,.txt,.md,.csv,.json,.geojson,.mp4,.webm', audio:'.wav,.mp3,.flac,.ogg', data:'.geojson,.json'}[action];
    input.click();
  };
  const homeEmpty = variant === 'home' && !session?.entries.length && !responseBusy && !awaitingTarget;
  const activity=t(session?.contextState?.status==='organizing'?'Organizing context…':liveTool?TOOLS[liveTool.name]??'Performing native task':liveEntry?.type==='assistant'&&liveEntry.status==='running'?'Writing response…':'Waiting for response…');
  const composer = <div className="agent-composer-area">
      {elsewhereBusy&&<p className="agent-other-activity" role="status">{t('Another conversation is running.')}<Button variant="quiet" size="xs" onClick={()=>onContinueInConversation?.(snapshot.activity?.sessionId??snapshot.selected?.id)}>{t('View running task')}</Button></p>}
      {error && <p className="agent-error" role="alert">{t(error)}</p>}
      {session && !compatible && <p className="agent-error">{t(compatibilityReason==='tool-set'?'This conversation needs updated tools. Reopen the assistant to continue.':'This conversation belongs to a different model connection. Start a new conversation.')}</p>}
      {!session?.goal&&['waiting','ready','paused','failed','completed'].includes(workflowStatus) && <div className="agent-composer-status" role="status">{['waiting','ready'].includes(workflowStatus) && <Spinner size={12}/>}
        <span>{t(workflowStatus === 'waiting' || workflowStatus === 'ready' ? 'Waiting for background tasks' : workflowStatus === 'paused' ? 'Continuation paused' : workflowStatus === 'failed' ? 'Tasks need attention' : 'Workflow complete')}</span>
        {['waiting','ready'].includes(workflowStatus) && !busy && <Button variant="quiet" size="icon" icon={Square} aria-label={t('Pause continuation')} tooltip={t('Pause continuation')} onClick={stop}/>}</div>}
      {attaching && <p className="agent-loading" role="status"><Spinner size={12}/>{t('Preparing attachments…')}</p>}
      {!data?.busy && organizing && <p className="agent-loading" role="status"><Spinner size={12}/>{t('Organizing context…')}</p>}
      {images.length>0 && <div className="agent-images agent-draft-images">{images.map(image=><div key={image.id}><AgentImage image={image}/><Button variant="quiet" size="icon" icon={X} aria-label={t('Remove image {name}',{name:image.name})} tooltip={t('Remove image')} onClick={()=>setImages(previous=>previous.filter(item=>item.id!==image.id))} disabled={busy || attaching}/></div>)}</div>}
      {documents.length>0 && <div className="agent-documents agent-draft-documents">{documents.map(document=><div key={document.id}><AgentDocument document={document}/><Button variant="quiet" size="icon" icon={X} aria-label={t('Remove file {name}',{name:document.name})} tooltip={t('Remove file')} onClick={()=>setDocuments(previous=>previous.filter(item=>item.id!==document.id))} disabled={busy || attaching}/></div>)}</div>}
      <PromptInput minRows={homeEmpty ? 4 : 2} value={input} onValueChange={setInput} onSubmit={() => send()} onStop={stop} loading={scene?responseBusy:busy}
        disabled={!data?.configured || !compatible || closing} canSubmit={!busy && !attaching && officeCompatible && audioCompatible && videoCompatible && Boolean(input.trim() || images.length || documents.length)}
        stopDisabled={Boolean(confirming) || revisionBusy || switching || session?.status === 'stopping'}
        placeholder={t('Describe your task, add images, data links or a vector boundary')} label={t('Message GeoD Agent')}
        addLabel={t('Attach files')} sendLabel={t('Send message')} stopLabel={t('Stop response')}
        actionsDisabled={attaching || images.length + documents.length >= 3}
        actions={[
          {value:'image',label:t('Add images'),description:t('Screenshots, photos or charts'),icon:Plus},
          {value:'document',label:t('Add documents or scans'),description:t('PDF, Office, text or scanned documents'),icon:File},
          {value:'audio',label:t('Add audio'),description:t('Audio files for a Google native model'),icon:AudioLines,disabled:data?.model?.protocol !== 'google-generative-ai'},
          {value:'audio-settings',label:t('Audio model settings'),icon:Settings},
          {value:'data',label:t('Add data boundary'),description:t('GeoJSON boundary or vector data'),icon:MapIcon},
        ]} onAction={attachmentAction}
        toolbarContent={<div className="composer-controls">
          <ComposerPermission mode={executionMode} disabled={busy || attaching || !data?.configured || !compatible || closing} onChange={changeExecution}/>
          <ComposerContext state={session?.contextState} usage={session?.usage} model={data?.model} disabled={busy || attaching || !session?.threadId || !compatible || !data?.configured} onOrganize={organize}>
            <InfoDetails closeLabel={t('Close')} summary={t('What is sent?')}><p>{t('Messages, chosen attachments, map context and requested tool results are sent to your model. Native permissions control execution; attachments stay saved locally.')}</p><p>{t('PDF reading depends on your selected model and endpoint. The original PDF is sent as a file; it is not converted to text.')}</p><p>{t('Office file reading depends on the model and endpoint. Embedded images or spreadsheet rows may be omitted by its file reader; use PDF for charts and visual layout.')}</p><p>{t('Organizing context sends this conversation to your model to create a shorter working summary. Visible history and native plans remain saved; selected attachments are restored when you continue.')}</p>
              <p>{t('Audio reading depends on the selected Google model and endpoint. The original file is sent without transcription or conversion. Clips are limited to 2 MiB and 10 minutes.')}</p><p>{t('Video reading depends on the selected Google model and endpoint. Original MP4 or WebM files are sent without conversion, up to 8 MiB and 10 minutes. Local playback depends on this device.')}</p></InfoDetails>
          </ComposerContext>
        </div>}
        modelPicker={<div className="prompt-input-model" data-prompt-control="model">{data?.registry?.connections.length > 0 ? <Select className="agent-model-select prompt-input-model-trigger" contentClassName="agent-model-menu" optionIcons={{__manage_models__:Bot}} aria-label={t('Agent model selection')} value={data.registry.selectedId ?? ''} displayValue={data.model?.model ?? t('No model connected')} title={data.model ? `${t(providerLabel(data.model.provider))} · ${data.model.model} · ${data.model.label}` : undefined} disabled={busy || attaching || closing} onChange={event => event.target.value === '__manage_models__' ? setModelOpen(true) : switchModel(event.target.value)}>
          {data.registry.connections.map(connection=><option key={connection.id} value={connection.id}>{connection.label} · {connection.model}</option>)}
          <option value="__manage_models__">{t('Models and connections…')}</option>
        </Select> : <Button variant="quiet" className="prompt-input-model-trigger" onClick={() => setModelOpen(true)} disabled={busy || attaching || closing || !desktopAvailable()} aria-label={t('Choose model')}><span>{data?.model?.model || t('Choose model')}</span><ChevronDown size={15} aria-hidden="true"/></Button>}</div>}
      >
        {!officeCompatible && <p className='agent-file-note' role='status'>{t('Office files require an OpenAI Responses connection.')}</p>}
        {!audioCompatible && <p className='agent-file-note' role='status'>{t('Audio files require a Google native connection.')}</p>}
        {!videoCompatible && <p className='agent-file-note' role='status'>{t('Video files require a Google native connection.')}</p>}
        <Input ref={fileInput} type="file" accept="image/png,image/jpeg,image/webp,.pdf,.docx,.xlsx,.pptx,.wav,.mp3,.flac,.ogg,.mp4,.webm,.txt,.md,.csv,.json,.geojson" multiple hidden style={{display:'none'}} onChange={attach}/>
      </PromptInput>
    </div>;
  const liveWork=activeWorkId(rows,scene?responseBusy:Boolean(data?.busy),executionActivity);
  const usageByRow=transcriptTokenUsage(rows,session?.usage,responseBusy||liveWork?session?.entries.findLast(entry=>entry.type==='user')?.id:undefined);
  const writing=responseBusy&&liveEntry?.type==='assistant'&&liveEntry.status==='running';
  const openTaskProject=id=>onOpenProject?.(id,...(scene?[session?.id]:[])),openTaskJobs=id=>onOpenTasks?.(id,...(scene?[session?.id]:[]));
  const nativeProps={plans:data?.plans,suppressOutputs:resultsOpen,disabled:busy||!compatible||Boolean(answering)||Boolean(confirming),decisionEntries:session?.entries,decisionPending:session?.entries.some(entry=>entry.decision?.status==='pending'),confirming,onConfirm:confirm,onEdit:editPlan,onPreview:previewTask,onOpenProject:openTaskProject,onOpenTasks:openTaskJobs,answering,onDecisionAnswer:answerDecision};
  const nativeEntryActive=entry=>Boolean(data?.busy&&(executionActivity===undefined||executionActivity?.kind==='turn'&&entry.turnId===executionActivity.turnId));
  const renderWork=row=>{
    const downloads=downloadWork(row.entries,data?.plans),summary=downloadTaskSummary(downloads.plans);
    const revealed=workReveal?.id===row.id?workReveal:undefined;
    return <WorkRecords key={`${session?.id}-${row.id}`} id={row.id} turnId={row.requestId} entries={downloads.entries} footer={usageByRow.has(row.id)?<TurnTokenUsage usage={usageByRow.get(row.id)}/>:null} active={row.id===liveWork} writing={writing} activity={row.backgroundRunning&&!responseBusy?t('Waiting for background tasks'):activity} completed={Boolean(row.finalId)&&!row.entries.some(entry=>['failed','interrupted'].includes(entry.status))} expandRequest={revealed?.request} keepVisible={entry=>entry.type==='reviews'&&!row.finalId||workEntryNeedsAttention(entry,data?.plans)} renderEntry={(entry,_,inspect)=>entry.type==='assistant'?<AgentMessage key={entry.id} entry={entry} progress active={nativeEntryActive(entry)} streaming={nativeEntryActive(entry)&&entry.status==='running'}/>:renderRow(entry,nativeEntryActive(entry),inspect)}
      keepAttachmentsVisible={!row.finalId||summary.unfinished}
      attachments={downloads.plans.length>0&&<AgentDownloadTasks plans={downloads.plans} onOpenTasks={openTaskJobs} expandRequest={revealed?.request} focusPlanId={revealed?.planId} renderPlan={plan=><PlanCard plan={plan} {...nativeProps} taskContext={row.entries.find(entry=>entry.type==='reviews'&&entry.references.some(ref=>ref.id===plan.planId))?.entry.taskContext}/>}/>}/>;
  };
  const renderRow=(row,active=false,onInspect)=>row.type==='work'?renderWork(row):
    row.type==='reviews'?row.references.map(ref=>{const plan=data?.plans?.find(value=>value.planId===ref.id);return plan?<PlanCard key={plan.planId} plan={plan} {...nativeProps} disabled={nativeProps.disabled||nativeProps.decisionPending} taskContext={row.entry.taskContext} reviewIssue={decisionReviewIssue(session?.entries??[],plan.planId)} confirming={confirming===plan.planId}/>:null;}):
    row.decision?<AgentDecisionCard key={row.id} decision={row.decision} disabled={nativeProps.disabled} submitting={answering===row.decision.id} onAnswer={answerDecision}/>:
    row.type==='tool'?<ToolEntry key={row.id} entry={row} showPlans={false} active={active} onInspect={onInspect} {...nativeProps}/>:
    <AgentMessage key={row.id} entry={row} footer={usageByRow.has(row.id)?<TurnTokenUsage usage={usageByRow.get(row.id)}/>:null} active={Boolean(data?.busy&&row.id===liveEntry?.id)} streaming={Boolean(data?.busy&&row.type==='assistant'&&row.status==='running')}/>;
  const closeRight=()=>{setResultsPreference(false);hideGoal(false);(resultsOpen?resultsToggle:goalToggle).current?.focus();};
  return <aside ref={panelElement} className={`agent-panel ${variant === 'home' ? 'agent-home' : ''}`} data-start={homeEmpty || undefined} id="geod-agent-panel" aria-label={t('GeoD Agent')}>
    <header className="agent-panel-header"><div className="agent-header-title"><MessageSquare size={17}/><strong>{scene?.kind==='workspace'?t('Workspace assistant'):'Agent'}</strong>{scene&&!homeEmpty&&<span className="agent-scene-context" role="status" title={t(scene.kind==='conversation'?'The task uses this conversation and its confirmed plans.':'The assistant uses the current page and your selection.')}>{t(scene.kind==='conversation'?'Independent task':scene.page)}{scene.page==='Explore'&&context?.catalog?` · ${t('{count} scenes',{count:context.catalog.filteredCount})}`:''}</span>}{['waiting','ready'].includes(workflowStatus)&&<Badge tone="blue">{t('In progress')}</Badge>}</div><div className="agent-header-actions">
      {scene?.kind==='workspace'&&session&&<Button variant="quiet" size="icon" icon={ArrowUpRight} aria-label={t('Continue in conversation')} tooltip={t('Continue in conversation')} onClick={()=>onContinueInConversation?.(session.id)}/>}
      {session?.goal&&<Button ref={goalToggle} className="agent-goal-toggle" data-goal-status={session.goal.status} variant="quiet" size="icon" icon={ListChecks} aria-label={t(goalOpen?'Hide plan progress':'Show plan progress')} tooltip={t(goalOpen?'Hide plan progress':'Show plan progress')} aria-controls="geod-agent-goal-sidebar" aria-expanded={goalOpen} onClick={()=>setGoalPreference(!goalOpen)}/>}
      {deliveries.length>0&&<Button ref={resultsToggle} className="agent-results-toggle" variant="quiet" size="icon" icon={Layers} aria-label={t(resultsOpen?'Hide conversation results':'Show conversation results')} tooltip={t(resultsOpen?'Hide conversation results':'Show conversation results')} aria-controls="geod-agent-results-sidebar" aria-expanded={resultsOpen} onClick={()=>{setResultsPreference(!resultsOpen);if(!wideGoalLayout)setHistoryOpen(false);}}/>}
      <Button ref={historyToggle} variant="quiet" size="icon" icon={History} aria-label={t('Conversation history')} tooltip={t('Conversation history')} aria-controls="geod-agent-history" aria-expanded={historyOpen} onClick={() => {setHistoryOpen(!historyOpen);if(!wideHistoryLayout){setResultsPreference(false);setGoalPreference(false);}}} disabled={!data?.runtimeAvailable || busy || attaching}/>
      <Button variant="quiet" size="icon" icon={Plus} aria-label={t('New conversation')} tooltip={t('New conversation')} onClick={newConversation} disabled={busy || attaching}/>
      <Popover open={moreOpen} onOpenChange={setMoreOpen} onCloseAutoFocus={finishMore} className="agent-more-menu" aria-label={t('Conversation options')} trigger={<Button variant="quiet" size="icon" icon={MoreHorizontal} aria-label={t('Conversation options')} tooltip={t('Conversation options')}/>}>
        <Button variant="quiet" size="row" icon={Settings2} onClick={()=>chooseMore(()=>setModelOpen(true))} disabled={busy || attaching || !desktopAvailable()}>{t('Agent model connection')}</Button>
        <Button variant="quiet" size="row" icon={Shrink} onClick={()=>chooseMore(organize)} disabled={busy || attaching || !session?.threadId || !compatible || !data?.configured}>{t('Organize context')}</Button>
        <Button variant="quiet" size="row" icon={HardDrive} onClick={()=>chooseMore(()=>setStorageAttachments({images:images.map(image=>image.id),documents:documents.map(document=>document.id)}))} disabled={busy || attaching}>{t('Attachment storage')}</Button>
      </Popover>
      {variant !== 'home' && <Button variant="quiet" size="icon" icon={X} aria-label={t('Close Agent')} tooltip={t('Close Agent')} onClick={close} disabled={closing}/>}
    </div></header>
    <div className="agent-workspace-layout">
    {historyOpen && <aside className="agent-history" id="geod-agent-history" aria-label={t('Conversation list')} data-overlay={!wideHistoryLayout||undefined} onKeyDown={event=>{if(event.key==='Escape'){event.stopPropagation();setHistoryOpen(false);historyToggle.current?.focus();}}}>
      <div className="agent-history-heading"><strong>{t('Conversations')}</strong><Button variant="quiet" size="icon" icon={PanelLeftClose} aria-label={t('Hide conversation list')} tooltip={t('Hide conversation list')} onClick={()=>{setHistoryOpen(false);historyToggle.current?.focus();}}/></div>
      <Button variant="secondary" size="sm" icon={Plus} onClick={newConversation} disabled={busy||attaching}>{t('New conversation')}</Button>
      <div className="agent-history-list" role="region" tabIndex={0} aria-label={t('Saved conversations')}>
        {!data&&<p role="status"><Spinner size={14}/>{t('Loading conversations…')}</p>}
        {visibleSessions?.length===0&&<p className="agent-history-empty">{t('Your conversations will appear here.')}</p>}
        {visibleSessions?.map(value => {
          const modelLabel=value.modelId?`${t(providerLabel(value.modelProvider))} · ${value.modelLabel} · ${value.modelId}`:t(value.status==='running'?'In progress':'Conversation');
          return <Button key={value.id} variant="quiet" size="row" className="agent-history-item" selected={!fresh && value.id === session?.id} title={value.title} disabled={busy||attaching} onClick={() => choose(value.id)}>{value.goal?.status==='complete'?<CheckCircle2 size={14}/>:<MessageSquare size={14}/>}<span><strong>{value.title}</strong><small title={modelLabel}>{modelLabel}</small></span></Button>;
        })}
      </div>
    </aside>}
    <div className="agent-workspace-main">
    {homeEmpty ? <div className="agent-home-start">
      <div className="agent-home-hero">
        <div className="agent-home-heading"><h1>{t('What geographic data do you need?')}</h1><p>{t('Describe a place, time and purpose. AI helps you find, download and process data.')}</p></div>
        {composer}
        <div className="agent-home-status">
          {!data && !error && <p role="status"><Spinner size={14}/>{t('Loading conversations…')}</p>}
          {data && !data.runtimeAvailable && <p>{t(desktopAvailable() ? 'Agent runtime is not ready' : 'Browser preview. Use the desktop app to chat with Agent.')}</p>}
          {data?.runtimeAvailable && !data.configured && <><span>{t('Connect a model to chat, or explore data directly.')}</span><Button size="sm" icon={Settings2} onClick={() => setModelOpen(true)}>{t('Connect model')}</Button></>}
        </div>
        <div className="agent-home-suggestions" aria-label={t('Example requests')}>
          {['Find the latest satellite imagery of New York City.', 'Find elevation data for Berlin.', 'Show my saved projects and download tasks.'].map(prompt => <Button key={prompt} variant="quiet" size="sm" disabled={busy} onClick={() => setInput(t(prompt))}>{t(prompt)}</Button>)}
        </div>
        <nav className="agent-home-shortcuts" aria-label={t('Workspace shortcuts')}>
          <Button ref={sourceTrigger} variant="secondary" onClick={()=>setSourcesOpen(true)} aria-haspopup="dialog"><Compass size={19}/><span><strong>{t('Explore data')}</strong><small>{t('Choose a source and search the map')}</small></span></Button>
          <Button asChild variant="secondary"><a href="#My%20Data"><FolderOpen size={19}/><span><strong>{t('My Data')}</strong><small>{t('Open saved projects and results')}</small></span></a></Button>
          <Button asChild variant="secondary"><a href="#Tasks"><ListTodo size={19}/><span><strong>{t('Tasks')}</strong><small>{t('Check downloads and retry failures')}</small></span></a></Button>
        </nav>
      </div>
    </div> : <>
    <div className="agent-chat-layout">
    <div className="agent-chat-main">
    <MessageScroller className="agent-conversation" viewportRef={scrolling} followRef={nearBottom} revision={data?.revision} conversationId={session?.id ?? 'draft'} followKey={(session?.entries??[]).filter(entry=>entry.type==='user').map(entry=>entry.id).join('|')} busy={scene?responseBusy:Boolean(data?.busy)} label={t('Conversation messages')} jumpLabel={t('Back to latest message')}>
      {(!data||awaitingTarget&&!session) && !error && <p className="agent-loading" role="status"><Spinner size={16}/>{t('Loading conversations…')}</p>}
      {data && !data.runtimeAvailable && <div className="agent-empty"><MessageSquare size={24}/><h2>{t('Agent runtime is not ready')}</h2><p>{t('The optional Agent runtime needs setup. Your data tools remain available.')}</p></div>}
      {data?.runtimeAvailable && !data.configured && !session?.entries.length && <div className="agent-empty"><MessageSquare size={24}/><h2>{t('Connect your model')}</h2><p>{t('Search imagery and prepare download or crop plans. Confirm each plan before it runs.')}</p><Button primary icon={Settings2} onClick={() => setModelOpen(true)}>{t('Connect model')}</Button></div>}
      {data?.configured && !awaitingTarget && !session?.entries.length && <div className="agent-empty"><img src="./brand/geod-symbol.png" width="28" height="28" alt=""/><h2>{t('Ask about your workspace')}</h2><p>{t('Describe a task. I can search, create projects, download, process and inspect results.')}</p><div className="agent-suggestions">
        {(scene?sceneSuggestions(scene,datasetId).map(prompt=>[prompt,Search]):[['Find SCL imagery for the current map area, download it into a project, then crop it to the map area.',Search],['Show my saved projects.',FolderOpen],['Which tasks need attention?',ListTodo]]).map(([prompt, Icon]) => <Button key={prompt} size="row" variant="quiet" icon={Icon} onClick={() => setInput(t(prompt))}>{t(prompt)}</Button>)}
      </div></div>}
      <TranscriptStage conversationId={session?.id??'draft'}>{rows.map(row=>renderRow(row))}
        {responseBusy&&!liveWork&&!writing&&<LiveStatus activity={activity}/>}
      </TranscriptStage>{session?.error && <p className="agent-error" role="alert">{t(session.error)}</p>}
      <ConversationMonitorReviews conversationId={session?.id}/>
    </MessageScroller>
    {composer}
    </div>
    {(goalOpen||resultsOpen)&&<aside className="agent-goal-sidebar agent-delivery-sidebar" id="geod-agent-results-sidebar" aria-label={t(goalOpen?'Plan sidebar':'Conversation results sidebar')} data-overlay={!wideGoalLayout||undefined} onKeyDown={event=>{if(event.key==='Escape'){event.stopPropagation();closeRight();}}}>
      {resultsOpen&&<div className="agent-delivery-heading"><Layers size={16}/><strong>{t('Results')}</strong><Button variant="quiet" size="icon" icon={PanelRightClose} aria-label={t('Close results panel')} tooltip={t('Close results panel')} onClick={()=>{setResultsPreference(false);resultsToggle.current?.focus();}}/></div>}
      <div className="agent-delivery-scroll" tabIndex={0} role="region" aria-label={t('Results and plan details')}>
      {resultsOpen&&<ConversationResults items={deliveries} onOpenResult={previewResult} onOpenProject={openTaskProject} onShowPlan={showGoalPlan}/>}
      {goalOpen&&<div id="geod-agent-goal-sidebar"><AgentGoalCard key={session.goal.id??session.id} inSidebar={!resultsOpen} onHide={()=>hideGoal()} goal={session.goal} plans={data?.plans} currentActivity={goalActivity} busy={busy} controlling={switching||!compatible} onControl={controlGoal} onShowPlan={showGoalPlan}/></div>}
      </div>
    </aside>}
    </div>
    </>}
    </div>
    </div>
    {sourcesOpen && <SourcePickerDialog returnFocusRef={sourceTrigger} onChoose={onChooseSource} onOpenEntry={onOpenSourceEntry} onClose={()=>setSourcesOpen(false)}/>}
    {modelOpen && <ModelDialog model={data?.model} registry={data?.registry} onSaved={modelSaved} onClose={() => setModelOpen(false)}/>}
    {revisionDraft && <PlanRevisionDialog draft={revisionDraft} onSave={revisePlan} onClose={()=>setRevisionDraft(null)}/>}
    {storageAttachments && <AttachmentStorageDialog protectedAttachments={storageAttachments} onClose={()=>setStorageAttachments(null)}/>}
  </aside>;
}
