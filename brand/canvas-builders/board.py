import re, glob, json, datetime
P='/home/claude/cav/canvas/project/'
def inl(f, size, uid=None):
    s=open(f).read()
    s=re.sub(r'width="512" height="512"', f'width="{size}" height="{size}" aria-hidden="true" style="display:block"', s, count=1)
    if uid is not None:
        for gid in set(re.findall(r'id="([A-Za-z0-9_-]+)"',s)):
            s=s.replace(f'id="{gid}"',f'id="{gid}_{uid}"').replace(f'url(#{gid})',f'url(#{gid}_{uid})')
    s=re.sub(r'<(path|rect|circle|stop)([^<>]*?)\s*/>', r'<\1\2></\1>', s)
    return s.strip()
CAP={
'iso-01-plain':('Plain','A 5×5 voxel C in isometric. The base form.'),
'iso-02-tower-mouth':('Tower in the mouth','The gradiometer column stands between the arms, with an atom cloud at each end.'),
'iso-03-tower-side':('Tower beside','The column stands clear of the C, full height. It reads as the C plus an instrument, never as $.'),
'iso-04-gradient-voxels':('Gradient voxels','Each voxel is lit by 1/r from the mouth, so the arm tips glow and the spine falls into shadow.'),
'iso-05-gradient-voxels-tower':('Gradient + tower','The same lighting with the column in the mouth as the source.'),
'iso-06-octree':('Octree','Voxels subdivide towards the mouth, the way the engine refines near mass.'),
'iso-07-octree-tower':('Octree + tower','Refinement with the instrument that causes it.'),
'iso-08-gradient-fill-side':('Gradient fill','Faces carry a continuous gradient, with a graded column beside them.'),
'iso-09-light-tower':('Light: tower','Charcoal voxels on bone.'),
'iso-10-light-octree-side':('Light: octree, tower beside','The busiest of the light set.'),
'iso-11-octree-gradient':('Octree + gradient + tower','All three ideas at once: size and tone both follow the field.'),
'flat-01-grid-tower':('Grid','A front orthographic voxel C, with the tower in the mouth.'),
'flat-02-gradient-tower':('Gradient','Voxel tone follows 1/r from the tower.'),
'flat-03-octree-tower':('Octree','Cells split towards the tower: coarse at the spine, fine at the tips.'),
'flat-04-solid-gradient-side':('Solid gradient','The C as one shape with a horizontal field gradient, and the tower beside it.'),
'flat-05-octree':('Octree, no tower','The refinement alone implies the mass.'),
'flat-06-gradient':('Gradient, no tower','The tonal field alone implies the mass.'),
'flat-07-light-octree-tower':('Light: octree','Charcoal on bone.'),
'flat-08-light-gradient-side':('Light: gradient, tower beside','A quiet light variant.'),
}
PICKS={'iso-03-tower-side','iso-07-octree-tower','flat-03-octree-tower','flat-05-octree','iso-11-octree-gradient'}
HEAD='''<!doctype html>
<html lang="en-GB">
<head>
<meta charset="utf-8">
<title>{title}</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;700&amp;display=swap">
<style>
body{{margin:0}}
a{{color:#c8c1b2}}a:hover{{color:#efe9dc}}
</style>
</helmet>
'''
TAIL='''</x-dc>
<script type="text/x-dc" data-dc-script data-props='{{"$preview":{{"width":{w},"height":{h}}}}}'>
class Component extends DCLogic {{
renderVals() {{ return {{}}; }}
}}
</script>
</body>
</html>
'''
def tile(f,i):
    k=f.split('/')[-1][:-4]; t,d=CAP[k]; pick=' · pick' if k in PICKS else ''
    return f'''<div style="display:flex;flex-direction:column;gap:12px">
<div style="width:300px;height:300px;outline:1px solid #2c2a26">{inl(f,300,i)}</div>
<div style="display:flex;flex-direction:column;gap:4px;width:300px">
<div style="font-size:15px;font-weight:700;color:#efe9dc">{t}<span style="font-weight:400;color:#a8a194">{pick}</span></div>
<div style="font-size:13px;line-height:1.45;color:#a8a194">{d}</div>
</div>
</div>'''
def family(name,title,lede,files,cols=4):
    rows=(len(files)+cols-1)//cols
    w=64*2+cols*300+(cols-1)*32
    h=64+44+16+48+40+rows*(300+12+80)+(rows-1)*32+64
    body=f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:40px">
<div style="display:flex;flex-direction:column;gap:12px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">{title}</h1>
<p style="margin:0;font-size:16px;line-height:1.5;color:#a8a194;max-width:760px">{lede}</p>
</div>
<div style="display:grid;grid-template-columns:repeat({cols}, minmax(0, 1fr));gap:32px">
{''.join(tile(f,i) for i,f in enumerate(files))}
</div>
</div>
'''
    open(P+name,'w').write(HEAD.format(title=title)+body+TAIL.format(w=w,h=h)); return w,h
iso=sorted(glob.glob('out/iso-*.svg')); flat=sorted(glob.glob('out/flat-*.svg'))
dims={}
dims['Main.dc.html']=family('Main.dc.html','Isometric voxel C','A 5×5 C in voxels, drawn isometric. A source mass sits in the mouth, and the gravity gradient shows up as voxel tone, voxel size (octree refinement) or a continuous gradient. The tower stays inside the mouth or stands beside the C, never across the arms, where it turns into a $.',iso)
dims['Flat.dc.html']=family('Flat.dc.html','Orthographic voxel C','The same ideas seen straight on. These are flatter and sharper, and the octree ones hold up best at favicon size.',flat)
# scale board
picks=['out/iso-03-tower-side.svg','out/iso-07-octree-tower.svg','out/flat-03-octree-tower.svg','out/flat-05-octree.svg']
names=['Isometric, tower beside','Isometric octree + tower','Flat octree + tower','Flat octree']
rows=''
for i,(f,n) in enumerate(zip(picks,names)):
    rows+=f'''<div style="display:flex;align-items:flex-end;gap:28px;padding:20px 0;border-top:1px solid #2c2a26">
<div style="width:220px;font-size:15px;font-weight:500;color:#efe9dc;align-self:center">{n}</div>
{inl(f,180,'s180'+str(i))}{inl(f,64,'s64'+str(i))}{inl(f,32,'s32'+str(i))}{inl(f,16,'s16'+str(i))}
<div style="display:flex;align-items:center;gap:8px;background:#2a2824;padding:8px 14px;font-size:13px;color:#c8c1b2;align-self:center">{inl(f,16,'tab'+str(i))}Cavendish</div>
</div>'''
w,h=1040,64*2+44+40+4*(180+40)+40
open(P+'Scale.dc.html','w').write(HEAD.format(title='Picks at size')+f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:24px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">Picks at size: 180, 64, 32, 16</h1>
<div style="display:flex;flex-direction:column">{rows}</div>
</div>
'''+TAIL.format(w=w,h=h)); dims['Scale.dc.html']=(w,h)
# lockup board
title=open('/home/claude/cav/out/readme/title-dark.svg').read()
title=re.sub(r'<(path)([^<>]*?)\s*/>', r'<\1\2></\1>', title)
tl=re.sub(r'width="1600" height="214"','width="760" height="102" aria-hidden="true" style="display:block"',title,count=1).strip()
titlel=open('/home/claude/cav/out/readme/title-light.svg').read()
titlel=re.sub(r'<(path)([^<>]*?)\s*/>', r'<\1\2></\1>', titlel)
tll=re.sub(r'width="1600" height="214"','width="760" height="102" aria-hidden="true" style="display:block"',titlel,count=1).strip()
def lock(f,bg,tt,uid,ink):
    return f'''<div style="background:{bg};padding:48px;display:flex;align-items:center;gap:36px">{inl(f,120,uid)}{tt}</div>'''
L=[lock('out/flat-03-octree-tower.svg','#1d1b19',tl,'L1',1),lock('out/iso-07-octree-tower.svg','#1d1b19',tl,'L2',1),lock('out/flat-07-light-octree-tower.svg','#efe9dc',tll,'L3',0),lock('out/iso-10-light-octree-side.svg','#efe9dc',tll,'L4',0)]
w,h=64*2+48*2+120+36+760, 64*2+44+24+4*216+3*16
open(P+'Lockup.dc.html','w').write(HEAD.format(title='Lockups')+f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:24px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">Lockups with the wordmark</h1>
<div style="display:flex;flex-direction:column;gap:16px">{''.join(L)}</div>
</div>
'''+TAIL.format(w=w,h=h)); dims['Lockup.dc.html']=(w,h)
# canvas.json
order=['Main.dc.html','Flat.dc.html','Scale.dc.html','Lockup.dc.html']
x=0; boards={}
mw,mh=dims['Main.dc.html']; boards['Main.dc.html']=dict(x=0,y=0,w=mw,h=mh,title='Isometric voxel C')
fw,fh=dims['Flat.dc.html']; boards['Flat.dc.html']=dict(x=mw+80,y=0,w=fw,h=fh,title='Orthographic voxel C')
sw,sh=dims['Scale.dc.html']; boards['Scale.dc.html']=dict(x=0,y=max(mh,fh)+120,w=sw,h=sh,title='Picks at size')
lw,lh=dims['Lockup.dc.html']; boards['Lockup.dc.html']=dict(x=sw+80,y=max(mh,fh)+120,w=lw,h=lh,title='Lockups')
cj={"v":3,"createdOnFiles":{"v":1,"at":datetime.datetime.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ')},"title":"Cavendish mark — voxel C exploration","launch":{"view":"canvas"},"pages":[],"boards":boards,"order":order,"notes":{"t":{"x":0,"y":-300,"text":"Cavendish: voxel C, tower and gravity gradient","kind":"title1","maxW":mw+80+fw}},"designSystems":[]}
json.dump(cj,open(P+'canvas.json','w'),indent=1)
print(dims)
import os; print({f:os.path.getsize(P+f) for f in os.listdir(P)})
