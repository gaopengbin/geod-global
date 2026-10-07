import React from 'react';

// Model text never creates raw HTML, remote images, executable links or
// artifact actions. Native cards own workspace navigation.
function inline(text) {
  return text.split(/(\*\*[^*\n]+\*\*|`[^`\n]+`)/g).map((part,index) => part.startsWith('**') && part.endsWith('**')
    ? <strong key={index}>{part.slice(2,-2)}</strong> : part.startsWith('`') && part.endsWith('`')
      ? <code key={index}>{part.slice(1,-1)}</code> : part);
}
export function MessageText({ text, streaming = false }) {
  const lines = text.split('\n'), blocks = [];
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index];
    const start = index;
    if (!line.trim()) continue;
    if (line.startsWith('```')) {
      const code = [];
      while (++index < lines.length && !lines[index].startsWith('```')) code.push(lines[index]);
      blocks.push(<pre key={start}><code>{code.join('\n')}</code></pre>); continue;
    }
    const item = line.match(/^\s*(?:[-*]|\d+\.)\s+(.+)/);
    if (item) {
      const ordered = /^\s*\d+\./.test(line), items = [item[1]];
      while (index + 1 < lines.length) {
        const next = lines[index + 1].match(ordered ? /^\s*\d+\.\s+(.+)/ : /^\s*[-*]\s+(.+)/);
        if (!next) break; items.push(next[1]); index++;
      }
      const List = ordered ? 'ol' : 'ul'; blocks.push(<List key={start}>{items.map((value,i) => <li key={i}>{inline(value)}</li>)}</List>); continue;
    }
    if (/^#{1,3}\s+/.test(line)) blocks.push(<p className="bui-message-heading" key={index}><strong>{inline(line.replace(/^#{1,3}\s+/,''))}</strong></p>);
    else blocks.push(<p key={index}>{inline(line)}</p>);
  }
  return <div className="bui-message-text" data-streaming={streaming || undefined}>{blocks}</div>;
}
