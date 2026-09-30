const pages = new Set(['Explore', 'Workspace', 'My Data', 'Tasks', 'Settings']);
const validId = value => /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value || '');

export function normalizeNavigationHash(hash) {
  const [name, query = ''] = hash.replace(/^#/, '').split('?');
  let page;
  try { page = decodeURIComponent(name); } catch { page = 'Explore'; }
  if (page === 'Recipes') page = 'My Data';
  if (!pages.has(page)) page = 'Explore';
  const params = new URLSearchParams(query);
  const saved = new URLSearchParams();
  if (['Explore', 'My Data', 'Workspace'].includes(page) && validId(params.get('project'))) saved.set('project', params.get('project'));
  else if (page === 'My Data' && params.get('view') === 'files') saved.set('view', 'files');
  if (page === 'Workspace' && validId(params.get('file'))) saved.set('file', params.get('file'));
  return '#' + encodeURIComponent(page) + (saved.size ? '?' + saved : '');
}
