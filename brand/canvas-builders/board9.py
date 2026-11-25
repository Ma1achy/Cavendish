import json
exec(open('board8.py').read().split("d={}")[0])
URL.update({'c-cmyk-light':'/_blob/32d806d1bce94bdf596a1e0b11081709','c-cmyk-dark':'/_blob/9c0e71c54ae520dcd51a487931c6c3fe','c-cmy-overprint':'/_blob/8423fb2fdee1d98d97d9fd4d201522db',
 'c-secondary-dark':'/_blob/1fc5852fbe9998ac6b3d34bfe3520a52','c-tertiary-a':'/_blob/19fd4776a6e9d23143a5f2e6e2515768','c-tertiary-b':'/_blob/8d0f974a4f59f0db4fd6c21276f7235f',
 't-cmyk-light':'/_blob/ad72f452e17ad7061424afc93bace746','t-cmyk-dark':'/_blob/c49b71563da8bdaeb67af20eadc292fe','t-cmy-overprint':'/_blob/370073ac4a061021b048fa3702ec89c1',
 't-secondary-dark':'/_blob/1117f4d22d3a92962d5e9babb4cb54d0','t-tertiary-a':'/_blob/558471d51abc5a53156653089b7137f0','t-tertiary-b':'/_blob/93da9c3921273444fc4fcc6afb4c42c3'})
URL.update(json.load(open('/home/claude/cav/urls.json')))
PAL.update({'cmyk-light':('CMYK, paper','Magenta field, cyan smoke, yellow floor, black (K) voxels.'),
 'cmyk-dark':('CMYK, dark','The same inks glowing on near-black, with bone voxels.'),
 'cmy-overprint':('CMY overprint','Softer, lighter inks on white paper.'),
 'secondary-dark':('Secondary triad','Orange field, purple smoke, green floor.'),
 'tertiary-a':('Tertiary A','Red-orange field, blue-violet smoke, chartreuse floor.'),
 'tertiary-b':('Tertiary B','Amber field, teal smoke, red-violet floor.')})
PICK.update({'c-cmyk-light','t-cmyk-light'})
ks=['cmyk-light','cmyk-dark','cmy-overprint','secondary-dark','tertiary-a','tertiary-b']
keys=['c-'+k for k in ks]+['t-'+k for k in ks]
cols=6; w=64*2+cols*300+(cols-1)*32; h=64+44+16+48+40+2*(300+12+84)+32+40+24+16+96+64
sizes=''.join(f'<div style="display:flex;align-items:flex-end;gap:16px">'+''.join(f'<img src="{URL[k]}" alt="" style="width:{s}px;height:{s}px;display:block">' for s in (96,48,32,16))+'</div>' for k in ['c-cmyk-light','c-cmyk-dark','t-cmyk-light','c-tertiary-b'])
body=f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:40px">
<div style="display:flex;flex-direction:column;gap:12px;max-width:1000px">
<h1 style="margin:0;font-size:40px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">Inks: CMYK and other triads</h1>
<p style="margin:0;font-size:16px;line-height:1.5;color:#a8a194">The same two compositions, with the three fields mapped to three inks. In CMYK, magenta is the gravity field, cyan the smoke, yellow the quadtree floor, and black (the K) the voxels. Then the secondary triad (orange, purple, green) and two tertiary triads. Top row: the C alone in the room. Bottom row: the C on the top face.</p>
</div>
<div style="display:grid;grid-template-columns:repeat({cols}, minmax(0, 1fr));gap:32px">
{''.join(tile(k) for k in keys)}
</div>
<div style="display:flex;flex-direction:column;gap:16px">
<div style="font-size:15px;font-weight:700">At size: 96, 48, 32, 16</div>
<div style="display:flex;flex-wrap:wrap;gap:40px">{sizes}</div>
</div>
</div>
'''
open('/home/claude/cav/canvas2/project/Inks.dc.html','w').write(HEAD.format(title='Inks: CMYK and other triads')+body+TAIL.format(w=w,h=h))
cj=json.load(open('/home/claude/cav/canvas2/project/canvas.json'))
cj['boards']['Inks.dc.html']=dict(x=6504,y=0,w=w,h=h,title='Inks: CMYK and other triads')
if 'Inks.dc.html' not in cj['order']: cj['order'].append('Inks.dc.html')
cj['notes']['t']['maxW']=6504+w
json.dump(cj,open('/home/claude/cav/canvas2/project/canvas.json','w'),indent=1)
print(w,h)
