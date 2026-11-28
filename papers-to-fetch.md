# Papers still to fetch

Rewritten 28 Nov 2026. The owner has supplied eleven batches in all (the last two on 27–28 Nov:
`papers-batch1`, `papers 2.zip`, the Komatitsch & Vilotte and Richards BSSA files, and the earlier
`graphics-refs`, `elastic solids vibration seismic` and `rigid bodies` zips). Every research note in
`docs/research/` has been updated from them, and everything they covered is removed from this list.
What is held was confirmed against the owner's local paper library and its index (28 Nov 2026).
What follows is only what is still missing, by note. DOIs were checked on 28 Nov 2026 against
`https://api.crossref.org/works/<doi>` unless marked otherwise. PDFs stay outside the repository,
which is public.

Resolved on 28 Nov and so removed: "Boy et al. on 3-D pressure corrections" is Boy & Chao 2005
(*JGR* 110, B08412, https://doi.org/10.1029/2002JB002333), which compares the 3-D atmosphere with the
thin layer (¶[1], ¶[29]); Boy, Gegout & Hinderer 2002 is the companion surface-pressure paper. Both
are held and read (`gradiometer-environment.md` §2.1). The "ELGAR companion" is arXiv:2007.04014,
held and read in part.

## Rigid bodies (`rigid-bodies.md`)
- [ ] van Zon & Schofield 2007, "Numerical implementation of the exact dynamics of free rigid bodies",
  *J. Comput. Phys.* 225, 145–164: https://doi.org/10.1016/j.jcp.2006.11.019 (the note's earlier
  "from memory" DOI, …11.020, is Yue et al.'s drop paper, which is why that one arrived instead)
- [ ] Tonge, Benevolenski & Sueda 2012, the supplementary material (convergence proofs) of "Mass
  splitting for jitter-free parallel rigid body simulation", *ACM TOG* 31(4), 105: main paper
  https://doi.org/10.1145/2185520.2185601; the supplement has no DOI of its own (ACM DL
  "supplemental material" tab). The file supplied as `tonge2012.pdf` is the main paper again.
- [ ] Silcowitz, Niebe & Erleben 2009, "Nonsmooth Newton method for Fischer function reformulation of
  contact force problems for interactive rigid body simulation", VRIPHYS 2009 (Eurographics
  Digital Library; **DOI not found in Crossref**, Eurographics DOIs are registered with DataCite)
- [ ] Celledoni & Säfström 2006, "Efficient time-symmetric simulation of torqued rigid bodies using
  Jacobi elliptic functions", *J. Phys. A* 39, 5463–5478: https://doi.org/10.1088/0305-4470/39/19/S08
  (the other source van Zon & Schofield's PRE cites for the free Euler solution)

## The gradiometer's environment (`gradiometer-environment.md`)
- [ ] MIGA's systematics paper. Not identified for certain; the likeliest is Canuel et al. 2018,
  "Exploring gravity with the MIGA large scale atom interferometer", *Sci. Rep.* 8, 14064:
  https://doi.org/10.1038/s41598-018-32165-z (Junca et al. 2019's ref. [19])

## Voxel engines (`voxel-engines.md`)
- [ ] Hoetzlein 2016, "GVDB: Raytracing sparse voxel database structures on the GPU", High-Performance
  Graphics 2016: https://doi.org/10.2312/hpg.20161197 (**not in Crossref**; it resolves through
  doi.org to the Eurographics Digital Library, checked 28 Nov 2026)

## World octree (`s5-world-octree.md`) and the f32 paper (`paper-boxes-and-precision.md`)
- [ ] Hida, Li & Bailey 2001, "Algorithms for quad-double precision floating point arithmetic",
  Proc. 15th IEEE Symposium on Computer Arithmetic, 155–162:
  https://doi.org/10.1109/ARITH.2001.930115 (to settle which double-word addition Thall 2006's "as in
  Hida" means: Joldes et al.'s Algorithm 5, unbounded, or 6)
- Not a paper: Dehnen 2014's figures were never seen (the PDF was read as text).

## The polyhedral-gravity paper (`paper-benchmarks.md`, `paper-benchmarks/literature-notes.md`)
- [ ] Holstein 2003, "Gravimagnetic anomaly formulas for polyhedra of spatially linear media",
  *Geophysics* 68(1), 157–167: https://doi.org/10.1190/1.1543203 (optional; Ren et al. 2018 take
  their κ estimates from it)
- [ ] Strakhov et al. 1986 and Strakhov & Lapina 1990, cited by Werner 2017 §13.3 for a gore
  half-angle form whose numerator is a height times an edge length; the likeliest earlier place for a
  rounding remark on the solid angle's numerator. Russian-language; **no DOI found**.
- [ ] Duff 1989, the printed version of "Polygon scan conversion by exact convolution" (*Raster
  Imaging and Digital Typography*, CUP, pp. 154–168 by a patent's citation; **no DOI**). The copy
  supplied is a 2024 retyping without venue or pages; only the citation needs checking.
- [ ] Hockney & Eastwood, *Computer Simulation Using Particles*, in full (low priority: cited only as
  the cloud-in-cell analogy). The copy held is 8 pages of the IOP paperback's front matter. The 1981
  McGraw-Hill original has no DOI; the CRC reissue is https://doi.org/10.1201/9780367806934.

## Fluids and elastic solids
Nothing missing. Johnson 1974 and Garvin 1956 (cited by Richards 1979 and Komatitsch & Vilotte
1998) would matter only if a dynamic elastic solver is built.

## The paper's moving bodies with the real phase (`paper-flight-phase.md`)
- [ ] Storey & Cohen-Tannoudji 1994, J. Phys. II France 4, 1999 (the first-order phase as the action along unperturbed paths; cited through Hogan et al. 2008 §5.2.1): https://doi.org/10.1051/jp2:1994103 (DOI from memory, not resolved)
- [ ] Cheinet et al. 2008, IEEE Trans. Instrum. Meas. 57, 1141, the published version (the arXiv preprint physics/0510197 was read): https://doi.org/10.1109/TIM.2007.915148
- [ ] Dimopoulos, Graham, Hogan & Kasevich 2008, Phys. Rev. D 78, 042003 (arXiv:0802.4098; the interferometer's suppressed response to short-wavelength gravity, Hogan et al.'s ref. [34])

## Surface representation (`surface-representation.md`)
DOIs below are from memory and not resolved (marked so); the PLIC side rests on Weymouth & Yue 2010,
which is held and cites these.
- [ ] Scardovelli & Zaleski 2000, "Analytical relations connecting linear interfaces and volume fractions in rectangular grids", *J. Comput. Phys.* 164, 228–237 (the closed-form volume ↔ plane offset of a cube cut by a plane): https://doi.org/10.1006/jcph.2000.6567 (not resolved)
- [ ] Dyadechko & Shashkov 2008, "Reconstruction of multi-material interfaces from moment data", *J. Comput. Phys.* 227, 5361–5384 (moment of fluid: the plane fitted to the fill and centroid): https://doi.org/10.1016/j.jcp.2007.12.029
- [ ] Pilliod & Puckett 2004, "Second-order accurate volume-of-fluid algorithms for tracking material interfaces", *J. Comput. Phys.* 199, 465–502 (LVIRA/ELVIRA normals): https://doi.org/10.1016/j.jcp.2003.12.023 (not resolved)
- [ ] Wald 2007, "On fast construction of SAH-based bounding volume hierarchies", IEEE/EG Symp. Interactive Ray Tracing, 33–40 (binned SAH): https://doi.org/10.1109/RT.2007.4342588 (not resolved)
- [ ] MacDonald & Booth 1990, "Heuristics for ray tracing using space subdivision", *The Visual Computer* 6, 153–166 (the surface-area heuristic): https://doi.org/10.1007/BF01911006 (not resolved)

## Worlds (`worlds.md`)
DOIs from the research agents' reading on 28 Nov 2026, not resolved against Crossref.
- [ ] Perlin 1985, "An Image Synthesizer", SIGGRAPH '85: https://doi.org/10.1145/325165.325247
- [ ] Cook & DeRose 2005, "Wavelet Noise", *ACM TOG* 24(3), 803–811 (read only through Lagae et al. 2010): https://doi.org/10.1145/1073204.1073264
- [ ] Musgrave, Kolb & Mace 1989, "The synthesis and rendering of eroded fractal terrains", SIGGRAPH '89: https://doi.org/10.1145/74334.74337
- [ ] Mei, Decaudin & Hu 2007, "Fast Hydraulic Erosion Simulation and Visualization on GPU", Pacific Graphics (HAL inria-00402079): https://doi.org/10.1109/PG.2007.15
- [ ] Cordonnier et al. 2016, "Large Scale Terrain Generation from Tectonic Uplift and Fluvial Erosion", *CGF* 35(2) (HAL hal-01262376): https://doi.org/10.1111/cgf.12820
- [ ] Smelik, Tutenel, Bidarra & Benes 2014, "A Survey on Procedural Modelling for Virtual Worlds", *CGF* 33(6), 31–50 (open PDFs exist, over 10 MB): https://doi.org/10.1111/cgf.12276
- [ ] Tobler 1979, "Smooth Pycnophylactic Interpolation for Geographical Regions", *JASA* 74: https://doi.org/10.1080/01621459.1979.10481647
- [ ] Boneva, Kendall & Stefanov 1971, "Spline Transformations", *JRSS B* 33(1), 1–71: https://doi.org/10.1111/j.2517-6161.1971.tb00855.x
- [ ] Ronchi, Iacono & Paolucci 1996, "The Cubed Sphere", *J. Comput. Phys.* 124, 93–114: https://doi.org/10.1006/jcph.1996.0047
- [ ] Chan & O'Neill 1975, quadrilateralised spherical cube (NTIS ADA010232), and O'Neill & Laubscher 1976 (DTIC ADA026294): no DOI
- [ ] Forsberg 1984, Ohio State Univ. Dept. Geodetic Science report 355, terrain reductions (DTIC ADA150788, refused automated fetch): no DOI
- [ ] Ebert, Musgrave, Peachey, Perlin & Worley, *Texturing and Modeling: A Procedural Approach*, 3rd ed., 2003, ISBN 1-55860-848-6 (Musgrave's chapters 14–20; read only through Blender's port)
