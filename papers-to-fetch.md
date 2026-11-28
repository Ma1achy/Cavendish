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
