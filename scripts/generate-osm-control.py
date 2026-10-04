"""Author a small independent synthetic OSM control with Pyosmium 4.3.1.

Includes joined reversed outer ways, a hole, nested ordinary relations, UTF-8
tags, untagged nodes and editing metadata. Never overwrites frozen fixtures.
"""
from pathlib import Path
import argparse,xml.etree.ElementTree as ET,json,hashlib,importlib.metadata
import osmium
p=argparse.ArgumentParser();p.add_argument('output');a=p.parse_args();out=Path(a.output);out.mkdir(parents=True,exist_ok=True)
xml=out/'independent.osm';pbf=out/'independent.osm.pbf';assert not xml.exists() and not pbf.exists()
root=ET.Element('osm',{'version':'0.6','generator':'Independent Pyosmium QA; synthetic'})
ET.SubElement(root,'bounds',{'minlon':'-1','minlat':'-1','maxlon':'5','maxlat':'5'})
metadata={'version':'2','changeset':'123','uid':'9','user':'测试 café','timestamp':'2025-01-02T03:04:05Z'}
for id,(lon,lat) in enumerate([(0,0),(4,0),(4,4),(0,4),(1,1),(1,2),(2,2),(2,1)],1):ET.SubElement(root,'node',{'id':str(id),'lon':str(lon),'lat':str(lat),**metadata})
for id,refs in [(101,[1,2,3]),(102,[1,4,3]),(103,[5,6,7,8,5])]:
    e=ET.SubElement(root,'way',{'id':str(id),**metadata})
    for ref in refs:ET.SubElement(e,'nd',{'ref':str(ref)})
for id,members,tags in [(201,[('way',101,'outer'),('way',102,'outer'),('way',103,'inner')],{'type':'multipolygon','building':'yes','name':'中文 & café'}),(202,[('relation',201,'site'),('node',1,'label')],{'type':'site','name':'nested'})]:
    e=ET.SubElement(root,'relation',{'id':str(id),**metadata})
    for kind,ref,role in members:ET.SubElement(e,'member',{'type':kind,'ref':str(ref),'role':role})
    for k,v in tags.items():ET.SubElement(e,'tag',{'k':k,'v':v})
ET.ElementTree(root).write(xml,encoding='utf-8',xml_declaration=True)
header=osmium.io.Header();header.set('generator','Independent Pyosmium QA; synthetic');header.add_box(osmium.osm.Box(-1,-1,5,5))
with osmium.SimpleWriter(str(pbf),header=header) as writer:
    class Copy(osmium.SimpleHandler):
        def node(self,o):writer.add_node(o)
        def way(self,o):writer.add_way(o)
        def relation(self,o):writer.add_relation(o)
    Copy().apply_file(str(xml))
receipt={'kind':'own synthetic independent control; not public provider imagery','writer':'Pyosmium '+importlib.metadata.version('osmium'),'sourceScript':'scripts/generate-osm-control.py','files':[{'file':f.name,'bytes':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest()} for f in (xml,pbf)]}
(out/'INDEPENDENT-SOURCE.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8');print(json.dumps(receipt))
