# Papers still to fetch

Rewritten 28 Nov 2026 after the owner's third batch of the day (`papers 3.zip`, 21 files). Every research
note it touches has been updated: `s6-density-and-skinning.md` (new), `rigid-bodies.md`,
`voxel-engines.md` and `gradiometer-environment.md`. Everything it covered is removed from this list.
What is held was checked against the owner's local paper library and its index. Only what is still
missing follows, by note. DOIs were checked on 28 Nov 2026 against `https://api.crossref.org/works/<doi>`
unless marked otherwise. PDFs stay outside the repository, which is public.

The third batch resolved these, so they are removed: van Zon & Schofield 2007 (*J. Comput. Phys.* 225,
145); Tonge et al. 2012's supplement; Silcowitz, Niebe & Erleben 2009 (DataCite DOI
10.2312/PE/vriphys/vriphys09/105-114); Canuel et al. 2018, MIGA (*Sci. Rep.* 8, 14064, read as
arXiv:1703.02490v3). The MIGA paper turned out to have no systematics budget, so that item is closed.
Also resolved: Hoetzlein 2016, GVDB, and Hockney & Eastwood in full (a 540-page scan with a text layer).
Removed because it was already held: Dimopoulos, Graham, Hogan & Kasevich 2008, *Phys. Rev. D* 78, 042003,
https://doi.org/10.1103/PhysRevD.78.042003. **A misleading file name:** `10.1190@1.2122413.pdf` held
Martins et al. 2010 (*Geophysics* 75, I21), not the paper with that DOI. That DOI is García-Abdeslem
2005, now listed below. It is not Holstein 2003, as the batch's note guessed.

## S6: density as a function (`s6-density-and-skinning.md`)
- [ ] García-Abdeslem 2005, "The gravitational attraction of a right rectangular prism with density
  varying with depth following a cubic polynomial", *Geophysics* 70(6), J39–J42:
  https://doi.org/10.1190/1.2122413. This is the benchmark prism that D'Urso & Trotta 2017 (Table 1)
  and Ren et al. 2018 both use. D'Urso & Trotta report that its Fig. 3 used wrong limits, so read it
  for the formulas, not the figure.
- [ ] Holstein 2003, "Gravimagnetic anomaly formulas for polyhedra of spatially linear media",
  *Geophysics* 68(1), 157–167: https://doi.org/10.1190/1.1543203. It is the source of the far-field
  exponents κ ≈ 4 and 5 that Ren et al. 2018 confirm, and one of D'Urso 2014's comparisons. Also
  wanted by `paper-benchmarks.md`.
- [ ] Optional: a primary source for simplex noise (Perlin's 2001 "Noise hardware" course notes, or
  Gustavson 2005, "Simplex noise demystified"). Neither has a DOI that I found. The note's statement
  about simplex noise's spherical kernels is from general knowledge until one is read.

## Rigid bodies (`rigid-bodies.md`)
- [ ] Optional: Celledoni & Säfström 2006, "Efficient time-symmetric simulation of torqued rigid
  bodies using Jacobi elliptic functions", *J. Phys. A* 39, 5463–5478:
  https://doi.org/10.1088/0305-4470/39/19/S08. Possibly relevant to the near-separatrix accuracy loss
  that B10 measured in van Zon & Schofield's algorithm.

## The gradiometer's environment (`gradiometer-environment.md`)
- [ ] Neumeyer, Schmidt & Stoeber 2006, "Improved determination of the atmospheric attraction with 3D
  air density data and its reduction on ground gravity measurements", *Dynamic Planet*, IAG Symposia
  130, 541–548: https://doi.org/10.1007/978-3-540-49350-1_79. This is the 3-D attraction method that
  both atmospheric papers in the third batch build on.

## World octree (`s5-world-octree.md`) and the f32 paper (`paper-boxes-and-precision.md`)
- [ ] Hida, Li & Bailey 2001, "Algorithms for quad-double precision floating point arithmetic",
  Proc. 15th IEEE Symposium on Computer Arithmetic, 155–162:
  https://doi.org/10.1109/ARITH.2001.930115. It would settle which double-word addition Thall 2006's
  "as in Hida" means: Joldes et al.'s Algorithm 5 (unbounded) or 6.
- Not a paper: Dehnen 2014's figures were never seen (the PDF was read as text).

## The polyhedral-gravity paper (`paper-benchmarks.md`, `paper-benchmarks/literature-notes.md`)
- [ ] Holstein 2003: see S6 above.
- [ ] Strakhov et al. 1986 and Strakhov & Lapina 1990, cited by Werner 2017 §13.3 for a gore
  half-angle form whose numerator is a height times an edge length. They are the likeliest earlier
  place for a rounding remark on the solid angle's numerator. Russian-language; **no DOI found**.
- [ ] Duff 1989, the printed version of "Polygon scan conversion by exact convolution" (*Raster
  Imaging and Digital Typography*, CUP, pp. 154–168 according to a patent's citation; **no DOI**). The
  copy supplied is a 2024 retyping without venue or pages, so only the citation needs checking.

## The paper's moving bodies with the real phase (`paper-flight-phase.md`)
- [ ] Storey & Cohen-Tannoudji 1994, "The Feynman path integral approach to atomic interferometry.
  A tutorial", *J. Phys. II France* 4, 1999–2027: https://doi.org/10.1051/jp2:1994103. It gives the
  first-order phase as the action along unperturbed paths, cited through Hogan et al. 2008 §5.2.1.
- [ ] Cheinet et al. 2008, "Measurement of the sensitivity function in a time-domain atomic
  interferometer", *IEEE Trans. Instrum. Meas.* 57, 1141–1148, the published version (the arXiv
  preprint physics/0510197 was read): https://doi.org/10.1109/TIM.2007.915148

## Surface representation (`surface-representation.md`)
The PLIC side rests on Weymouth & Yue 2010, which is held and cites these. All five DOIs now resolve
in Crossref to the titles given.
- [ ] Scardovelli & Zaleski 2000, "Analytical relations connecting linear interfaces and volume
  fractions in rectangular grids", *J. Comput. Phys.* 164, 228–237:
  https://doi.org/10.1006/jcph.2000.6567. It gives the closed-form volume against plane offset of a
  cube cut by a plane.
- [ ] Dyadechko & Shashkov 2008, "Reconstruction of multi-material interfaces from moment data",
  *J. Comput. Phys.* 227, 5361–5384: https://doi.org/10.1016/j.jcp.2007.12.029. This is moment of
  fluid: the plane fitted to the fill and the centroid.
- [ ] Pilliod & Puckett 2004, "Second-order accurate volume-of-fluid algorithms for tracking material
  interfaces", *J. Comput. Phys.* 199, 465–502: https://doi.org/10.1016/j.jcp.2003.12.023 (LVIRA and
  ELVIRA normals)
- [ ] Wald 2007, "On fast construction of SAH-based bounding volume hierarchies", IEEE Symp.
  Interactive Ray Tracing, 33–40: https://doi.org/10.1109/RT.2007.4342588 (binned SAH)
- [ ] MacDonald & Booth 1990, "Heuristics for ray tracing using space subdivision", *The Visual
  Computer* 6, 153–166: https://doi.org/10.1007/BF01911006 (the surface-area heuristic)

## Fluids and elastic solids
Nothing missing. Johnson 1974 and Garvin 1956 (cited by Richards 1979 and Komatitsch & Vilotte 1998)
would matter only if a dynamic elastic solver is built.
