import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

// The pinned server may echo user image data in thread history events. Model
// output and private native IPC remain bounded separately.
const FRAME_LIMIT = 32_000_000;
export const CODEX_VERSION = '0.159.2';

export function safeToolFailure(error) {
  // Only known native validation messages become guidance. Never forward a
  // provider body, path, URL, secret, or arbitrary source text to the model.
  switch (error?.message) {
    case 'Answer the pending decision card before preparing or executing dependent plans.':
    case 'Answer the pending decision card before confirming the download task.':
      return 'A pending human decision card blocks dependent planning or execution. This is not a missing tool, permission failure or exhausted budget. Read the current geod_execution_policy decision state. If it is an old missing-polygon fallback, read the same requested native boundary; verified matching geometry can supersede that obsolete card without inventing a human answer. Otherwise stop dependent calls and wait for the human to answer the actual card. Final task confirmation remains separate.';
    case 'Agent turn operation budget reached.':
      return 'The bounded native operation budget for this turn is exhausted. Stop further calls and summarize the actual blocker and retained successful references. This is not a permission failure or evidence of missing source data. Do not ask for a new conversation; the human can continue this same saved task.';
    case 'Revise the persisted task goal for the latest crop request before preparing its project.':
      return 'The saved goal still belongs to an earlier requirement. Call geod_goal_define for the complete revised crop task and final processed deliveries, preserving unchanged imagery/source/area constraints and the actual answered crop choice. Then prepare the project review. Input tile downloads do not by themselves satisfy the cropped delivery.';
    case 'Choose the crop area through a decision card before preparing the project.':
      return 'The crop extent is unresolved. First read the requested administrative candidate and call geod_boundary_read with its actual boundarySource. A search envelope does not establish that the polygon is absent. Pass the actual returned native boundary to project_plan. Only if no supported source polygon can be read, call geod_request_decision with question id crop_area and options rectangle (crop to the resolved outer rectangle) and boundary (provide/select a verified native polygon). Wait for an actual human answer; a recommendation is not consent. Do not silently create a rectangular crop.';
    case 'The requested boundary crop needs a verified native polygon before preparing the project.':
      return 'Preserve the boundary-crop requirement. A rectangular project cannot satisfy it. Read the actual administrative candidate boundarySource through geod_boundary_read, or use the actual attached native geometry with useAttachedPolygon=true. Do not ask the human to upload a boundary supplied by a native source. If the boundary request fails, distinguish source/network access from absent data; never invent vertices or silently use a bbox.';
    case 'Boundary changed, expired or belongs to another conversation. Read its source boundary again.':
      return 'This boundary reference cannot be used. Read the same selected source boundary again with geod_boundary_read, then pass its newly returned exact reference. Keep the requested area; do not invent geometry or substitute a rectangle.';
    case 'Read a fresh Census city candidate before reading its boundary.':
    case 'Read a fresh administrative candidate before reading its boundary.':
      return 'Read the same requested place/administrative candidate again and use its actual boundarySource. This is a stale or missing lookup reference, not evidence that no polygon exists.';
    case 'Administrative boundary lookup timed out.':
      return 'The source boundary request timed out. Preserve the requested administrative polygon, stop repeating the source call this turn, and explain the service failure. A timeout is not proof that the boundary is absent; reference ADM0/ADM1 polygons remain available offline.';
    case 'Administrative source changed. Search the area again before selecting a boundary.':
      return 'The boundary source no longer matches the selected cached dataset. Refresh the same administrative search and use its new exact boundarySource; do not mix versions or silently change the area.';
    case 'City boundary exceeds the supported native polygon limits; a rectangle is not an exact replacement.':
      return 'The city polygon cannot be used within the supported native geometry limits. The source boundary exists; explain the limitation and ask a selectable choice for a supported extent or another verified boundary. Never call a rectangular crop an exact administrative mask.';
    case 'Scene search is stale or belongs to another conversation. Search again.':
      return 'This search receipt cannot be used for a new plan. Repeat geod_scene_search in this conversation with the same requested source, area, dates and filters, then use the newly returned searchId and actual scene IDs. This read-only refresh does not authorize execution. Do not change the requested place or ask the human to start a new conversation.';
    case 'Agent plan expired. Create a new plan before confirming.':
    case 'Agent plan expired. Create a new plan.':
      return 'This review has expired. Prepare a fresh native review with the same effective requirements and fresh search or source preflight as required. Preserve the original request and wait for human confirmation of the new card; never execute the expired plan.';
    case 'Define the complete goal once for an actual human task request.':
      return 'Keep the saved goal and every required output. An automatic continuation cannot replace it; only a new human task request can revise the goal.';
    case 'Each required delivery needs its own native reference. Combine duplicate requirements instead of counting the same result twice.':
      return 'This native result is already bound to another required delivery. Do not count the same file twice or silently remove a distinct requirement. Use each actual final delivery plan.';
    case 'Use a completed, exhausted native metadata read.':
      return 'Metadata completion requires an actual completed native read with all required pages exhausted. Use its returned goalReadEntryId, not a fabricated entry or incomplete first page.';
    case 'Choose the matching current delivery plan, not a superseded or different output.':
    case 'Use a native plan recorded in this conversation.':
      return 'Use the actual current native delivery plan from this conversation. Superseded, unknown or different-kind plans cannot prove the required output.';
    case 'Choose an available required goal output.':
      return 'This required output is unavailable or unknown. Keep unsupported requirements visible and explain the missing capability; do not label the whole goal complete.';
    case 'This conversation requires confirmation. Ask the user to confirm in chat or enable automatic execution.':
      return 'Native permission requires human confirmation. Ask the human to say 确认执行 or 确认全部方案 in chat, or enable automatic execution. Do not repeat execution tool calls.';
    case 'Task retry or cancellation requires a fresh human request.':
    case 'Ask the human to request retry or cancellation explicitly.':
    case 'This task action was already requested in this turn. Read its current status.':
    case 'A background task needs attention. Wait for a new human request before starting more work.':
      return 'The native task action is not allowed for this turn. Read the actual status and explain the issue. Only a new human request can authorize retry or cancellation; do not repeat the action or start more tasks.';
    case 'Choose a task belonging to this conversation\'s plan.':
      return 'The task must belong to a plan prepared in this conversation. Read its exact native plan and job identifiers; never use another conversation or a global job guess.';
    case 'STAC datetime must be RFC3339 or an RFC3339 interval':
      return 'The datetime filter requires full RFC3339 timestamps with time and timezone. Convert the user dates to UTC start/end timestamps; calendar dates alone are invalid.';
    case 'STAC datetime interval is reversed':
      return 'The datetime interval ends before it starts. Keep the requested user dates in ascending order.';
    case 'Agent result exceeds 32 KiB; request fewer records or one job':
      return 'This read exceeds the bounded result size. Use the provided paging tool and a smaller limit; follow its continuation to read every declaration.';
    case 'Place lookup timed out.':
    case 'Place lookup is temporarily unreachable.':
    case 'Place response interrupted.':
      return 'The native place lookup exhausted its bounded provider attempts because the location services could not be reached. This is a network or service failure, not a model-permission error or an empty match. Do not retry alternate spellings in this turn or substitute an unrelated map area. Keep the requested place and explain the temporary failure briefly; the human can ask to retry.';
    case 'GeoD tool timed out.':
      return 'The native operation exceeded its time limit. Preserve the requested place and source, stop repeating this operation in this turn, and explain the timeout briefly. Do not infer missing location coverage or a model-permission problem.';
    case 'GeoD tool is not permitted.':
      return 'This tool call does not match the active conversation, turn or declared native tools. Stop repeating the call; this is not an exhausted operation budget and does not establish that the requested data source or place is unavailable.';
    case 'Agent record directory was redirected.':
    case 'Managed storage root changed':
    case 'Geographic lookup storage failed for this turn. Stop geographic retries.':
      return 'The native storage check refused saving the lookup or plan record. This is a local persistence failure, not an empty place match, network outage or model permission error. Stop repeated lookups and catalog searches in this turn. Preserve the requested city and do not substitute a same-named state, unrelated map area or ask the human for coordinates. Explain the storage issue briefly.';
    case 'Resolve the requested city extent before imagery search. Do not substitute a state or current map.':
      return 'The requested city has no successful native extent for this turn, or this search uses a different area. Do not search a same-named state or unrelated current map. Resolve the actual city first; if the native location lookup failed, explain that specific failure briefly without asking for coordinates or repeating catalog requests.';
    case 'Administrative lookup timed out.':
    case 'Administrative lookup is temporarily unreachable.':
      return 'The administrative boundary service is temporarily unreachable. This is a network or service failure, not an empty match or model-permission failure. Bundled ADM0 and ADM1 searches remain available offline. A city gazetteer lookup is a separate bounded fallback; do not retry this failing boundary service with spelling variants or invent coordinates.';
    case 'Administrative source has no dataset for this country or level.':
      return 'The boundary source does not publish this country or administrative level. Read available geod_region_levels, preserve the requested country and administrative type, and use the separate city gazetteer if appropriate. Do not substitute a province for a city or describe missing source coverage as a permission error.';
    case 'Administrative source denied the request.':
      return 'The boundary source denied this request. Do not retry or bypass the restriction; preserve the requested place and explain the refusal briefly.';
    case 'Administrative source returned invalid boundary data.':
    case 'Administrative dataset exceeds the bounded download limit.':
      return 'The boundary source did not return a valid bounded geographic dataset. Do not guess coordinates or claim the area was not found. Preserve the requested place; the separate city gazetteer may provide a supported actual extent.';
    case 'Choose a country before querying detailed administrative levels.':
    case 'Choose a valid ISO country code.':
    case 'Choose a known ISO country code; use a three-letter code for a territory absent from the reference layer.':
      return 'Resolve the requested country with geod_region_search, then use its ISO countryCode and geod_region_levels. Ask a short country clarification only when genuinely ambiguous; never ask the human to know technical ISO codes or level numbers.';
    case 'Place service denied the request.':
      return 'The public location service denied this request. Do not retry, bypass its restriction or guess an extent. Preserve the named place and explain the service refusal briefly.';
    case 'Place response is invalid JSON.':
    case 'Place service returned an invalid geographic document.':
    case 'Place service returned an invalid candidate list.':
    case 'Place response exceeds 1 MiB.':
      return 'The location services did not return valid bounded geographic data. Do not guess coordinates or repeat requests with alternate spellings; explain the service failure briefly and preserve the requested place.';
    default:
      return 'The native GeoD operation failed without a public error category. Stop repeating this operation in this turn. Preserve the requested place and source; this failure is not evidence of missing place coverage or a model-permission problem. Explain the tool failure briefly without guessing its cause.';
  }
}

export async function launchCodex({ executable, home, cwd, bridgeUrl, token, model, onEvent, callTool }) {
  await mkdir(home, { recursive: true }); await mkdir(cwd, { recursive: true });
  const quoted = value => JSON.stringify(value);
  await writeFile(join(home, 'config.toml'), `model = ${quoted(model)}\nmodel_provider = "geod"\nmodel_context_window = 32768\nmodel_auto_compact_token_limit = 26000\nmodel_auto_compact_token_limit_scope = "body_after_prefix"\nweb_search = "disabled"\napproval_policy = "never"\nsandbox_mode = "read-only"\nproject_doc_max_bytes = 0\n[model_providers.geod]\nname = "GeoD AI SDK text connection"\nbase_url = ${quoted(bridgeUrl)}\nwire_api = "responses"\nenv_key = "GEOD_BRIDGE_TOKEN"\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n[features]\nmulti_agent = false\nshell_tool = false\nshell_snapshot = false\ncode_mode_host = false\nplugins = false\nremote_plugin = false\nplugin_sharing = false\napps = false\nview_image = false\nimage_generation = false\n[windows]\nsandbox = "unelevated"\n`, { mode: 0o600 });
  // No inherited Codex settings, cloud credentials or user model tokens. The
  // only credential seen by Codex is this process-local loopback bridge token.
  const env = { CODEX_HOME: home, HOME: home, GEOD_BRIDGE_TOKEN: token };
  for (const name of ['SystemRoot', 'SYSTEMROOT', 'WINDIR', 'TEMP', 'TMP']) if (process.env[name]) env[name] = process.env[name];
  const child = spawn(executable, ['app-server', '--stdio', '-c', 'features.goals=true'], { cwd, env, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  let serial = 0, buffer = '', closed = false;
  const pending = new Map();
  const send = value => { if (closed || !child.stdin.writable) throw new Error('Agent runtime is not running.'); child.stdin.write(`${JSON.stringify(value)}\n`); };
  const fail = () => {
    if (closed) return; closed = true;
    for (const entry of pending.values()) { clearTimeout(entry.timer); entry.reject(new Error('Agent runtime stopped.')); }
    pending.clear(); onEvent({ method: 'geod/runtimeStopped', params: {} });
  };
  child.once('error', fail); child.once('exit', fail);
  child.stdin.on('error', fail);
  // Drain diagnostics without persisting them: stderr can contain provider
  // payloads and platform paths. Public errors come from bounded status codes.
  child.stderr.on('data', bytes => { if (process.env.GEOD_AGENT_TEST_TRACE === '1') process.stderr.write(String(bytes).replaceAll(token, '[redacted]')); });
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', chunk => {
    buffer += chunk;
    if (Buffer.byteLength(buffer) > FRAME_LIMIT) { child.kill(); fail(); return; }
    let index;
    while ((index = buffer.indexOf('\n')) !== -1) {
      const line = buffer.slice(0, index); buffer = buffer.slice(index + 1);
      let value; try { value = JSON.parse(line); } catch { child.kill(); fail(); return; }
      if ('id' in value && !value.method) {
        const entry = pending.get(value.id); if (!entry) continue;
        clearTimeout(entry.timer); pending.delete(value.id);
        if (value.error) {
          if (process.env.GEOD_AGENT_TEST_TRACE === '1') process.stderr.write(JSON.stringify(value.error).replaceAll(token, '[redacted]')+'\n');
          entry.reject(new Error('Agent runtime request failed.'));
        } else entry.resolve(value.result);
      } else if ('id' in value && value.method) {
        if (value.method === 'item/tool/call') {
          Promise.resolve().then(() => callTool(value.params)).then(result => {
            if (!closed) send({ id: value.id, result: { success: true, contentItems: [{ type: 'inputText', text: JSON.stringify(result) }] } });
          }, error => { if (!closed) send({ id: value.id, result: { success: false, contentItems: [{ type: 'inputText', text: safeToolFailure(error) }] } }); });
        } else {
          // Approval, command/process execution, filesystem and auth requests
          // have no delegate in this first, read-only stage.
          send({ id: value.id, error: { code: -32601, message: 'This Agent stage allows only registered read-only GeoD tools.' } });
        }
      } else if (value.method) onEvent(value);
    }
  });
  const rpc = (method, params) => new Promise((resolve, reject) => {
    const id = ++serial;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error('Agent runtime request timed out.')); }, 25_000);
    pending.set(id, { resolve, reject, timer });
    try { send({ id, method, params }); } catch (error) { clearTimeout(timer); pending.delete(id); reject(error); }
  });
  try {
    await rpc('initialize', { clientInfo: { name: 'geod_global', title: 'GeoD Global', version: CODEX_VERSION }, capabilities: { experimentalApi: true } });
    send({ method: 'initialized' });
  } catch (error) { child.kill(); fail(); throw error; }
  return { rpc, async close() {
    if (closed) return;
    await new Promise(resolve => {
      const timer = setTimeout(() => { child.kill(); resolve(); }, 1500);
      child.once('exit', () => { clearTimeout(timer); resolve(); }); child.stdin.end();
    }); fail();
  } };
}
