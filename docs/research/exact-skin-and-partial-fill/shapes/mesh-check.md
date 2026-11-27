| mesh | closed | pieces (expected) | max vertex → original or caps (m) | max original vertex → mesh (m) | self-crossings | verdict |
|---|---|---|---|---|---|---|
| teapot, repaired | True | 1 (1) | 1.5e-17 | 0.0e+00 | 0 | pass |
| teapot, control: marching cubes 1/48 m | True | 2 (1) | 7.6e-03 | 2.4e-02 | 0 | FAIL |
| suzanne, repaired | True | 1 (1) | 5.4e-17 | 0.0e+00 | 0 | pass |
| suzanne, control: marching cubes 1/48 m | True | 1 (1) | 8.6e-03 | 1.8e-02 | 0 | FAIL |
| bunny, repaired | True | 1 (1) | 0.0e+00 | 0.0e+00 | 0 | pass |
| bunny, control: marching cubes 1/48 m | True | 1 (1) | 7.7e-03 | 1.5e-02 | 0 | FAIL |
| spot, repaired | True | 1 (1) | 0.0e+00 | 0.0e+00 | 0 | pass |
| spot, control: marching cubes 1/48 m | True | 1 (1) | 8.8e-03 | 1.2e-02 | 0 | FAIL |
| dragon, repaired | True | 1 (1) | 0.0e+00 | 0.0e+00 | 0 | pass |
| dragon, control: marching cubes 1/48 m | True | 6 (1) | 8.2e-03 | 4.0e-02 | 0 | FAIL |
| armadillo, repaired | True | 1 (1) | 0.0e+00 | 0.0e+00 | 0 | pass |
| armadillo, control: marching cubes 1/48 m | True | 4 (1) | 1.1e-02 | 3.4e-02 | 0 | FAIL |
| buddha, repaired | True | 1 (1) | 0.0e+00 | 0.0e+00 | 0 | pass |
| buddha, control: marching cubes 1/48 m | True | 6 (1) | 8.6e-03 | 3.1e-02 | 0 | FAIL |
| bracket, repaired | True | 1 (1) | 0.0e+00 | 0.0e+00 | 0 | pass |
| bracket, control: marching cubes 1/48 m | True | 1 (1) | 9.0e-03 | 2.4e-02 | 0 | FAIL |

Decimation (the decimated mesh against the original download, same frame, vertex-sampled both ways):

| mesh | original triangles | on non-manifold edges, dropped | decimated to | cut out where it crossed itself (rounds) | after repair | max decimated vertex → original (m) | max original vertex → decimated (m) | RMS original → decimated (m) | original vertices beyond 1/256 m | volume change | verdict (< 1/256 m) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| dragon | 871414 | 0 | 50000 | 118 (1) | 49138 | 2.2e-03 | 2.2e-02 | 5.8e-04 | 691 | -7.1e-04 | FAIL |
| armadillo | 345944 | 0 | 60000 | 0 (0) | 59996 | 2.6e-03 | 3.3e-03 | 2.8e-04 | 0 | -5.7e-04 | pass |
| buddha | 1087716 | 78 | 49998 | 91 (1) | 47974 | 2.5e-03 | 1.0e-02 | 5.0e-04 | 1912 | -3.9e-04 | FAIL |
