// Display receipts are a bounded projection of native data, never an execution
// authority. Unknown fields, URLs, local paths and credentials stay out of UI.
const FIELDS = new Set(`query name title kind id provider product country countryCode countryName state county adminLevel level levelName boundaryType boundaryName boundaryID dataYear year units unitCount count total matchCount truncated cached checkedAt retrievedAt status settled bytesDownloaded totalBytes sha256 geometryType bounds boundsCrs center candidates levels availableLevels scenes sources projects jobs files assets itemId itemIds assetKey date endDate cloud start end maxCloud limit offset searchId projectId jobId planId boundary polygon useAttachedPolygon coverage coveredFraction target basis areaMethod sceneCount missingParts missingBounds unknownItemIds moreAvailable complete limitReached featureCount raster metadata width height bands crs epsg pixelSize dataType noData verified source provenance license format expectedBytes approvalRequired mode action available context catalog totalCount filteredCount selectedCount loadedCount visibleLoadedCount cloudMin cloudMax pages includesSavedProjectScenes activeDate textFilter sort dateCounts dateCountsTruncated attachedPolygon todayUtc scene page subject selectionScope selectedItemIds visibleItemIds focusedItemId selectionTruncated`.split(' '));
const unsafeText = /(?:https?:\/\/|file:\/\/|[a-z]:[\\/]|\\\\|(?:api[_-]?key|password|secret|access[_-]?token)\s*[:=])/i;
for (const field of ['dataset','datasetId','request','layer','month','unit','warnings','failures','sourceId','downloadsStarted','nominalResolution','valueInterpretation','version']) FIELDS.add(field);
for (const field of ['gscloud','productId','seriesId','products','items','presence','downloadStatus','pageSize','totalPages','hasMore','cloudCover','datetime','bbox','matchedProducts','directoryTotal','appliedFilter']) FIELDS.add(field);
for (const field of ['archive','members','crc32','rasterPreview','access','agentDownload','desktopOriginalDownload','requiresOwnAccount','archiveRasterPreview']) FIELDS.add(field);
function project(value, budget) {
  let limited = false, remaining = budget;
  function visit(value, depth) {
    if (depth > 5 || remaining < 24) { limited = true; return undefined; }
    if (value === null || typeof value === 'boolean' || typeof value === 'number' && Number.isFinite(value)) {
      remaining -= String(value).length + 2; return value;
    }
    if (typeof value === 'string') {
      if (unsafeText.test(value) || /[\u0000-\u001f\u007f]/.test(value)) { limited = true; return undefined; }
      let text = value.slice(0, Math.min(160, remaining - 4));
      while (JSON.stringify(text).length + 2 > remaining) text = text.slice(0, -1);
      limited ||= text.length < value.length; remaining -= JSON.stringify(text).length + 2; return text;
    }
    if (Array.isArray(value)) {
      limited ||= value.length > 8;
      return value.slice(0, 8).flatMap(item => { const next = visit(item, depth + 1); return next === undefined ? [] : [next]; });
    }
    if (value && typeof value === 'object') {
      const output = {};
      for (const [key, item] of Object.entries(value)) {
        if (!FIELDS.has(key)) { limited = true; continue; }
        if (remaining < key.length + 24) { limited = true; break; }
        remaining -= key.length + 4;
        const next = visit(item, depth + 1);
        if (next !== undefined) output[key] = next;
      }
      return output;
    }
  }
  return { value: visit(value, 0) ?? {}, limited };
}
export function startToolRecord(input, now = Date.now()) {
  const projected = project(input, 500);
  return { version: 1, startedAt: new Date(now).toISOString(), input: projected.value, limited: projected.limited };
}
export function finishToolRecord(record, result, now = Date.now()) {
  const projected = project(result, 1400);
  return { ...record, durationMs: Math.max(0, Math.min(3_600_000, now - Date.parse(record.startedAt))), result: projected.value, limited: record.limited || projected.limited };
}
export function validToolRecord(value) {
  if (!value || Object.keys(value).some(key => !['version','startedAt','input','result','durationMs','limited'].includes(key))
    || value.version !== 1 || typeof value.startedAt !== 'string' || value.startedAt.length > 64 || !Number.isFinite(Date.parse(value.startedAt))
    || typeof value.limited !== 'boolean' || !value.input || Array.isArray(value.input) || typeof value.input !== 'object'
    || value.durationMs !== undefined && (!Number.isSafeInteger(value.durationMs) || value.durationMs < 0 || value.durationMs > 3_600_000)
    || value.result !== undefined && (!value.result || Array.isArray(value.result) || typeof value.result !== 'object')
    || JSON.stringify(value).length > 2400) return false;
  function valid(data, depth = 0) {
    if (depth > 5) return false;
    if (data === null || typeof data === 'boolean' || typeof data === 'number' && Number.isFinite(data)) return true;
    if (typeof data === 'string') return data.length <= 160 && !unsafeText.test(data) && !/[\u0000-\u001f\u007f]/.test(data);
    if (Array.isArray(data)) return data.length <= 8 && data.every(item => valid(item, depth + 1));
    return data && typeof data === 'object' && Object.entries(data).every(([key, item]) => FIELDS.has(key) && valid(item, depth + 1));
  }
  return valid(value.input) && (value.result === undefined || valid(value.result));
}
