import json
exec(open('board9.py').read().split("ks=['cmyk-light'")[0])
URL.update(json.load(open('/home/claude/cav/urls.json')))
groups=[
 ('The pair: dark and light',[
   ('c-magma-dark','Primary mark, dark','The C alone in the room, magma with emerald. Its shadow on the red field reads the letter twice; red field, blue smoke and green floor are the three things Cavendish models. For the dark README, the social card and the site.'),
   ('c-tertiary-b-light','Primary mark, light','The same scene on paper in the tertiary triad: amber field, teal smoke, red-violet floor, charcoal voxels. The amber keeps the magma gold, so the pair feels related while every ink changes.')]),
 ('The icon',[
   ('t-magma-dark','Favicon and app icon','C on the top face. A solid hexagon holds up best at 16 to 32 px, and the C stays readable on top. Same magma inks, so it belongs to the main mark.'),
   ('t-tertiary-b-light','Favicon, light','The top-face cube in the tertiary triad: violet top, charcoal C, amber field and teal smoke on the sides.')]),
 ('One colour',[
   ('c-mono-dark','Mono, dark','Charcoal and bone, for when colour isn’t available or would fight the context: papers, slides, footers.'),
   ('c-bw-light','Black and white, light','Pure neutral greys, for print and fax-grade use.')]),
]
w=64*2+3*(2*440+40)+2*80
cards=''
for gname,items in groups:
    tiles=''.join(f'''<div style="display:flex;flex-direction:column;gap:14px;width:440px">
<img src="{URL[k]}" alt="{t}" style="width:440px;height:440px;display:block;outline:1px solid #2c2a26">
<div style="display:flex;flex-direction:column;gap:6px">
<div style="font-size:17px;font-weight:700;color:#efe9dc">{t}</div>
<div style="font-size:14px;line-height:1.5;color:#a8a194">{d}</div>
</div>
</div>''' for k,t,d in items)
    cards+=f'''<div style="display:flex;flex-direction:column;gap:20px">
<div style="font-size:20px;font-weight:700;color:#efe9dc;border-top:1px solid #2c2a26;padding-top:16px">{gname}</div>
<div style="display:flex;gap:40px">{tiles}</div>
</div>'''
sizes=''.join(f'<div style="display:flex;align-items:flex-end;gap:16px">'+''.join(f'<img src="{URL[k]}" alt="" style="width:{s}px;height:{s}px;display:block">' for s in (128,64,32,16))+'</div>' for k in ['c-magma-dark','c-tertiary-b-light','t-magma-dark','t-tertiary-b-light'])
h=64+44+16+72+48+(36+440+14+130)+48+(24+16+128)+64
body=f'''<div style="width:{w}px;height:{h}px;box-sizing:border-box;padding:64px;background:#121110;color:#efe9dc;font-family:Inter,'Helvetica Neue',Helvetica,Arial,sans-serif;display:flex;flex-direction:column;gap:48px">
<div style="display:flex;flex-direction:column;gap:12px;max-width:1100px">
<h1 style="margin:0;font-size:44px;line-height:1.05;font-weight:700;letter-spacing:-0.01em">The strongest: a small system</h1>
<p style="margin:0;font-size:17px;line-height:1.5;color:#a8a194">Six picks that work together: one primary mark in dark and light, a cube icon for small sizes, and two one-colour fallbacks. The primary pair shares a structure and swaps inks, magma red, blue and green by night; amber, teal and violet by day.</p>
</div>
<div style="display:flex;gap:80px">{cards}</div>
<div style="display:flex;flex-direction:column;gap:16px">
<div style="font-size:15px;font-weight:700">At size: 128, 64, 32, 16</div>
<div style="display:flex;flex-wrap:wrap;gap:56px">{sizes}</div>
</div>
</div>
'''
open('/home/claude/cav/canvas2/project/Picks.dc.html','w').write(HEAD.format(title='The strongest')+body+TAIL.format(w=w,h=h))
cj=json.load(open('/home/claude/cav/canvas2/project/canvas.json'))
cj['pages']=[{'id':'all','name':'All variants'},{'id':'picks','name':'Strongest'}]
cj['boards']['Picks.dc.html']=dict(x=0,y=0,w=w,h=h,title='The strongest',page='picks')
if 'Picks.dc.html' not in cj['order']: cj['order'].append('Picks.dc.html')
cj['launch']={'view':'canvas','page':'picks'}
cj['notes']['p']={'x':0,'y':-300,'text':'Cavendish: the strongest marks','kind':'title1','maxW':w,'page':'picks'}
json.dump(cj,open('/home/claude/cav/canvas2/project/canvas.json','w'),indent=1)
print(w,h)
