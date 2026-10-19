// GeoD Agent chat-ui.tsx MarkdownContent port (MIT), ecd93b7f.
// Native Global cards retain file/navigation/remote-asset authority.
import React, { memo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { CheckSquare, Square } from 'lucide-react';
const components={a:({children})=><span>{children}</span>,img:({alt})=><span>{alt}</span>,input:({checked})=>{const Icon=checked?CheckSquare:Square;return <Icon size={14} aria-label={checked?'Checked':'Unchecked'}/>;}};
const MarkdownContent=memo(function MarkdownContent({text}){return <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>{text}</ReactMarkdown>;});
export function MessageText({text='',streaming=false}) {return <div className="bui-message-text" data-streaming={streaming||undefined}><MarkdownContent text={text}/></div>;}
