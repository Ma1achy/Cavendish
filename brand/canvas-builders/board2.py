import sys, json, re; sys.path.insert(0,'.')
exec(open('board.py').read().split("iso=sorted(")[0])   # reuse inl, HEAD, TAIL, family helpers
CAP.update({
'p16-faces-octree-panel-h-noghost':('Faces, octree, gradient panel','The C on the left face and the tower on the right. The wash across the tower’s face is the field falling off from the C. No ghost cube needed.'),
'p15-faces-octree-panel-h':('Same, with ghost cube','Dashed edges spell out the cube the two faces belong to.'),
'p14-faces-panel-h':('Faces, solid C, panel','Plain voxels. The gradient alone carries the physics.'),
'p17-faces-gradient-panel-v':('Faces, gradient voxels, vertical panel','The C is lit by 1/r towards the corner, and the panel grades vertically along the tower’s baseline.'),
'p18-light-faces-octree-panel':('Light: faces, octree, panel','Charcoal on bone.'),
'p09-faces-octree-ghost':('Faces, octree, ghost','The C refines towards the shared edge, where the tower is.'),
'p08-faces-gradient-ghost':('Faces, gradient, ghost','Voxel tone follows the field, and the tower is graded too.'),
'p07-faces-ghost':('Faces, plain, ghost','The base layout.'),
'p10-faces-octree':('Faces, octree, no ghost','Without the cube the two planes merge. The panel versions fix this.'),
'p13-light-faces-octree-ghost':('Light: faces, octree, ghost','Charcoal on bone.'),
'p03-ground-octree-tower':('Ground C, tower rising','The C lies flat as terrain and the tower stands out of it. The C gets foreshortened.'),
'p06-ground-octree-shaft':('Ground C, shaft down','The tower as a vertical shaft below the surface, like AION.'),
'p11-top-octree-right-shaft':('Top face C, tower on right face','The C on the cube’s top and the tower down its side.'),
})
PICKS.update({'p16-faces-octree-panel-h-noghost','p15-faces-octree-panel-h','p14-faces-panel-h'})
order=['p16-faces-octree-panel-h-noghost','p15-faces-octree-panel-h','p14-faces-panel-h','p17-faces-gradient-panel-v','p18-light-faces-octree-panel',
       'p09-faces-octree-ghost','p08-faces-gradient-ghost','p07-faces-ghost','p10-faces-octree','p13-light-faces-octree-ghost',
       'p03-ground-octree-tower','p06-ground-octree-shaft','p11-top-octree-right-shaft']
files=['out2/'+k+'.svg' for k in order]
# make gradient ids unique per tile (pg… ids are per file already)
w,h=family('Planes.dc.html','Tower on another face','The C sits on the left face of an implied cube and the tower stands on the right face, in perpendicular planes. The gravity gradient becomes a wash across the tower’s face, brightest at the edge it shares with the C. Ground and top-face versions are at the end for comparison.',files,cols=5)
cj=json.load(open('/home/claude/cav/canvas/project/canvas.json'))
cj['boards']['Planes.dc.html']=dict(x=3008,y=0,w=w,h=h,title='Tower on another face')
cj['order'].append('Planes.dc.html')
cj['notes']['t']['maxW']=3008+w
json.dump(cj,open('/home/claude/cav/canvas/project/canvas.json','w'),indent=1)
print(w,h)
