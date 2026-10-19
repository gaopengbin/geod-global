import React from 'react';import {it,expect,vi,afterEach} from 'vitest';import {cleanup,render,screen} from '@testing-library/react';import {VectorWorkspace} from './vector-map.jsx';import official from './official-vector-native.fixture.json';
vi.mock('./i18n.jsx',()=>({useI18n:()=>({t:s=>s,number:String})}));
vi.mock('./vector-client.js',()=>({vectorRequest:vi.fn(async()=>({asset:official[0],geojson:{type:'FeatureCollection',features:[]}})),exportVector:vi.fn()}));
vi.mock('./ui/index.jsx',async load=>({...await load(),ResizableGroup:({children})=><div>{children}</div>,ResizablePanel:({children})=><div>{children}</div>,ResizeHandle:()=>null}));
vi.mock('ol/color.js',()=>({asArray:()=>[38,117,220,1]}));
vi.mock('ol/Map.js',()=>({default:class{constructor(){}getView(){return{fit(){}};}on(){}setTarget(){}updateSize(){}}}));
afterEach(cleanup);
it('the actual Microsoft asset workspace labels its CDLA URL correctly rather than substituting ODbL',async()=>{render(<VectorWorkspace id={official[0].id}/>);const link=await screen.findByRole('link',{name:'CDLA Permissive 2.0'});expect(link.getAttribute('href')).toBe(official[0].licenseUrl);expect(screen.queryByRole('link',{name:'ODbL'})).toBeNull();});
