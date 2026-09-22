const SERVICE = 'http://127.0.0.1:4318';

export function desktopAvailable() {
  return Boolean(window.__TAURI__?.core?.invoke);
}

export async function runtimeRequest(operation, payload, signal) {
  if (desktopAvailable()) {
    const commands = { health: 'health', list: 'list_jobs', create: 'create_job', cancel: 'cancel_job', retry: 'retry_job', reveal: 'reveal_job' };
    try {
      return await window.__TAURI__.core.invoke(commands[operation], operation === 'create' ? { request: payload } : payload || {});
    } catch (error) {
      throw new Error(typeof error === 'string' ? error : error?.message || 'The desktop task command failed.');
    }
  }
  const routes = { health: '/health', list: '/jobs', create: '/jobs', cancel: `/jobs/${encodeURIComponent(payload?.id)}/cancel`, retry: `/jobs/${encodeURIComponent(payload?.id)}/retry` };
  if (!routes[operation]) throw new Error('Open the desktop app to reveal local files.');
  const mutation = ['create', 'cancel', 'retry'].includes(operation);
  const response = await fetch(SERVICE + routes[operation], {
    method: mutation ? 'POST' : 'GET',
    headers: mutation ? { 'Content-Type': 'application/json', 'X-GeoD-Client': 'geod-global' } : {},
    body: operation === 'create' ? JSON.stringify(payload) : mutation ? '{}' : undefined,
    signal: signal || AbortSignal.timeout(10000),
  });
  const body = await response.json();
  if (!response.ok) throw new Error(typeof body.error === 'string' ? body.error : body.message || `Task service returned ${response.status}`);
  return body;
}

export function downloadableAssets(scene) {
  return ['scl', 'visual', 'thumbnail'].flatMap(key => {
    const asset = scene?.assets?.[key];
    if (!asset?.href) return [];
    try {
      const url = new URL(asset.href);
      if (url.protocol !== 'https:' || url.hostname !== 'sentinel-cogs.s3.us-west-2.amazonaws.com') return [];
    } catch { return []; }
    return [{ key, ...asset }];
  });
}

export function formatBytes(value) {
  if (!Number.isFinite(value) || value < 0) return 'Unknown size';
  if (value < 1024) return `${value} B`;
  const unit = value < 1048576 ? 'KiB' : value < 1073741824 ? 'MiB' : 'GiB';
  const scale = unit === 'KiB' ? 1024 : unit === 'MiB' ? 1048576 : 1073741824;
  return `${(value / scale).toFixed(1)} ${unit}`;
}
